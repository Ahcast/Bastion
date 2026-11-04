use crate::service::{bump, editable, event, load};
use crate::{Expedient, TenderService};
use bastion_core::CoreError;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LotItem {
    pub descripcion: String,
    pub pais_origen: String,
    pub fecha_entrega: String,
    pub cantidad: String,
    pub unidad: String,
    pub transporte: String,
    pub precio_unitario: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LotInput {
    pub items: Vec<LotItem>,
}
#[derive(Serialize)]
pub struct Lot {
    pub numero: u32,
    pub items: Vec<LotItem>,
    pub tamano_tabla: String,
    pub total: String,
    pub version: u64,
}
#[derive(Serialize)]
pub struct LotSummary {
    pub numero: u32,
    pub items: u64,
    pub tamano_tabla: String,
}

// Return an exact nonnegative integer at a fixed scale. Never round user input.
fn scaled(value: &str, scale: u32) -> Result<i128, CoreError> {
    let (whole, fraction) = value.split_once('.').unwrap_or((value, ""));
    if whole.is_empty()
        || whole.len() > 12
        || !whole.bytes().all(|b| b.is_ascii_digit())
        || fraction.len() > scale as usize
        || !fraction.bytes().all(|b| b.is_ascii_digit())
        || (value.contains('.') && fraction.is_empty())
    {
        return Err(CoreError::Invalid);
    }
    let whole: i128 = whole.parse().map_err(|_| CoreError::Invalid)?;
    let fraction: i128 = if fraction.is_empty() {
        0
    } else {
        fraction.parse::<i128>().map_err(|_| CoreError::Invalid)?
            * 10_i128.pow(scale - fraction.len() as u32)
    };
    whole
        .checked_mul(10_i128.pow(scale))
        .and_then(|v| v.checked_add(fraction))
        .ok_or(CoreError::Invalid)
}
pub(crate) fn validate_table_font_size(value: &str) -> Result<(), CoreError> {
    if value.len() > 8 || !(100..=10_000).contains(&scaled(value, 2)?) {
        return Err(CoreError::Invalid);
    }
    Ok(())
}
fn nonempty(value: &str, max: usize) -> bool {
    !value.trim().is_empty()
        && value.chars().count() <= max
        && !value
            .chars()
            .any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t'))
}
impl LotInput {
    pub(crate) fn total_for_style(&self, style: &str) -> Result<String, CoreError> {
        self.calculate_for_style(style, true)
    }
    fn calculate_for_style(&self, style: &str, require_origin: bool) -> Result<String, CoreError> {
        match style {
            "bid" => self.calculate(true, require_origin),
            "std" => self.calculate(false, require_origin),
            "jica" => self.calculate_ddp(require_origin),
            _ => Err(CoreError::Invalid),
        }
    }
    fn calculate_ddp(&self, require_origin: bool) -> Result<String, CoreError> {
        if self.items.len() > 1000 {
            return Err(CoreError::Invalid);
        }
        let scale = 10_i128.pow(12);
        let mut total = 0_i128;
        for item in &self.items {
            if !nonempty(&item.descripcion, 4000)
                || !nonempty(&item.fecha_entrega, 255)
                || !nonempty(&item.unidad, 255)
                || (require_origin && !nonempty(&item.pais_origen, 255))
                || item.transporte != "0.00"
            {
                return Err(CoreError::Invalid);
            }
            let product = scaled(&item.cantidad, 12)?
                .checked_mul(scaled(&item.precio_unitario, 2)?)
                .ok_or(CoreError::Invalid)?;
            let cents = product.checked_add(scale / 2).ok_or(CoreError::Invalid)? / scale;
            total = total.checked_add(cents).ok_or(CoreError::Invalid)?;
        }
        Ok(format!("{}.{:02}", total / 100, total % 100))
    }
    pub fn total(&self, bid: bool) -> Result<String, CoreError> {
        self.calculate(bid, true)
    }
    fn calculate(&self, bid: bool, require_origin: bool) -> Result<String, CoreError> {
        if self.items.len() > 1000 {
            return Err(CoreError::Invalid);
        }
        // Quantities have up to 12 decimals; numerator is cents * 10^14.
        let denominator = 100_i128 * 10_i128.pow(12);
        let mut total = 0_i128;
        for item in &self.items {
            if !nonempty(&item.descripcion, 4000)
                || !nonempty(&item.fecha_entrega, 255)
                || !nonempty(&item.unidad, 255)
                || (bid && require_origin && !nonempty(&item.pais_origen, 255))
                || (!bid && (!item.pais_origen.is_empty() || item.transporte != "0.00"))
            {
                return Err(CoreError::Invalid);
            }
            let qty = scaled(&item.cantidad, 12)?;
            let price = scaled(&item.precio_unitario, 2)?;
            let transport = scaled(&item.transporte, 2)?;
            let line = qty
                .checked_mul(price)
                .and_then(|v| v.checked_mul(115))
                .and_then(|v| {
                    transport
                        .checked_mul(denominator)
                        .and_then(|t| v.checked_add(t))
                })
                .ok_or(CoreError::Invalid)?;
            total = total.checked_add(line).ok_or(CoreError::Invalid)?;
        }
        let cents = total
            .checked_add(denominator / 2)
            .ok_or(CoreError::Invalid)?
            / denominator;
        Ok(format!("{}.{:02}", cents / 100, cents % 100))
    }
}
fn config(exp: &Expedient, number: u32) -> Result<&str, CoreError> {
    let get = |key: &str| {
        exp.variables
            .iter()
            .find(|v| v.key == key)
            .and_then(|v| v.value.as_ref())
            .and_then(|v| v.as_str())
    };
    let count = get("lotes")
        .ok_or(CoreError::Invalid)?
        .parse::<u32>()
        .map_err(|_| CoreError::Invalid)?;
    if number == 0 || number > count || count > 1000 {
        return Err(CoreError::Invalid);
    }
    match get("stylo") {
        Some(style @ ("bid" | "std" | "jica")) => Ok(style),
        _ => Err(CoreError::Invalid),
    }
}
pub(crate) async fn reconcile(
    tx: &mut sqlx::Transaction<'_, sqlx::MySql>,
    id: u64,
    old: &[crate::GlobalVariable],
    new: &[crate::GlobalVariable],
    confirm: bool,
) -> Result<(), CoreError> {
    let get = |values: &[crate::GlobalVariable], key: &str| {
        values
            .iter()
            .find(|v| v.key == key)
            .and_then(|v| v.value.as_ref())
            .and_then(|v| v.as_str())
            .map(str::to_owned)
    };
    let old_style = get(old, "stylo");
    let new_style = get(new, "stylo");
    let count = get(new, "lotes")
        .and_then(|s| s.parse::<u32>().ok())
        .filter(|n| *n <= 1000)
        .unwrap_or(0);
    let rows: Vec<(u32, serde_json::Value, String)> = sqlx::query_as("SELECT numero,items_json,tamano_tabla FROM licitaciones_lotes WHERE expediente_id=? ORDER BY numero FOR UPDATE").bind(id).fetch_all(&mut **tx).await?;
    let mut deletions = Vec::new();
    let mut changes = Vec::new();
    for (number, raw, size) in rows {
        let mut items: Vec<LotItem> =
            serde_json::from_value(raw).map_err(|_| CoreError::Invalid)?;
        if number > count || new_style.is_none() {
            if (!items.is_empty() || scaled(&size, 2)? != 700) && !confirm {
                return Err(CoreError::Conflict);
            }
            deletions.push(number);
        } else if old_style != new_style
            && (old_style.as_deref() == Some("jica") || new_style.as_deref() == Some("jica"))
        {
            if !items.is_empty() && !confirm {
                return Err(CoreError::Conflict);
            }
            if !items.is_empty() {
                changes.push((number, Vec::new()));
            }
        } else if old_style.as_deref() == Some("bid") && new_style.as_deref() == Some("std") {
            let loses = items.iter().any(|item| {
                !item.pais_origen.is_empty() || scaled(&item.transporte, 2).is_ok_and(|n| n != 0)
            });
            if loses && !confirm {
                return Err(CoreError::Conflict);
            }
            for item in &mut items {
                item.pais_origen.clear();
                item.transporte = "0.00".into();
            }
            changes.push((number, items));
        }
    }
    for number in deletions {
        sqlx::query("DELETE FROM licitaciones_lotes WHERE expediente_id=? AND numero=?")
            .bind(id)
            .bind(number)
            .execute(&mut **tx)
            .await?;
    }
    for (number, items) in changes {
        sqlx::query(
            "UPDATE licitaciones_lotes SET items_json=? WHERE expediente_id=? AND numero=?",
        )
        .bind(serde_json::to_value(items).map_err(|_| CoreError::Invalid)?)
        .bind(id)
        .bind(number)
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}
impl TenderService {
    pub async fn lot_summaries(&self, actor: u64, id: u64) -> Result<Vec<LotSummary>, CoreError> {
        let mut tx = self.begin(actor, "ver").await?;
        load(&mut tx, id).await?;
        let rows: Vec<(u32, serde_json::Value, String)> = sqlx::query_as("SELECT numero,items_json,tamano_tabla FROM licitaciones_lotes WHERE expediente_id=? ORDER BY numero").bind(id).fetch_all(&mut *tx).await?;
        let result = rows
            .into_iter()
            .map(|(numero, value, tamano_tabla)| {
                value
                    .as_array()
                    .map(|items| LotSummary {
                        numero,
                        items: items.len() as u64,
                        tamano_tabla,
                    })
                    .ok_or(CoreError::Invalid)
            })
            .collect();
        tx.commit().await?;
        result
    }
    pub async fn lot(&self, actor: u64, id: u64, numero: u32) -> Result<Lot, CoreError> {
        let mut tx = self.begin(actor, "ver").await?;
        let exp = load(&mut tx, id).await?;
        let style = config(&exp, numero)?;
        let row: Option<(serde_json::Value, String)> = sqlx::query_as(
            "SELECT items_json,tamano_tabla FROM licitaciones_lotes WHERE expediente_id=? AND numero=?",
        )
        .bind(id)
        .bind(numero)
        .fetch_optional(&mut *tx)
        .await?;
        let size = row
            .as_ref()
            .map(|(_, size)| size.clone())
            .unwrap_or_else(|| "7".into());
        let items: Vec<LotItem> = row
            .map(|(value, _)| serde_json::from_value(value))
            .transpose()
            .map_err(|_| CoreError::Invalid)?
            .unwrap_or_default();
        let total = LotInput {
            items: items.clone(),
        }
        .calculate_for_style(style, false)?;
        tx.commit().await?;
        Ok(Lot {
            numero,
            items,
            tamano_tabla: size,
            total,
            version: exp.version,
        })
    }
    pub async fn save_lot(
        &self,
        actor: u64,
        id: u64,
        numero: u32,
        version: u64,
        input: LotInput,
    ) -> Result<Expedient, CoreError> {
        self.save_lot_with_font_size(actor, id, numero, version, input, None)
            .await
    }
    pub async fn save_lot_with_font_size(
        &self,
        actor: u64,
        id: u64,
        numero: u32,
        version: u64,
        input: LotInput,
        tamano_tabla: Option<String>,
    ) -> Result<Expedient, CoreError> {
        if let Some(value) = &tamano_tabla {
            validate_table_font_size(value)?;
        }
        let mut tx = self.begin(actor, "editar").await?;
        let exp = load(&mut tx, id).await?;
        editable(&exp, version)?;
        input.total_for_style(config(&exp, numero)?)?;
        sqlx::query("INSERT INTO licitaciones_lotes(expediente_id,numero,items_json,tamano_tabla) VALUES (?,?,?,COALESCE(?, '7')) ON DUPLICATE KEY UPDATE items_json=VALUES(items_json),tamano_tabla=COALESCE(?,tamano_tabla)")
            .bind(id).bind(numero).bind(serde_json::to_value(input.items).map_err(|_|CoreError::Invalid)?).bind(&tamano_tabla).bind(&tamano_tabla).execute(&mut *tx).await?;
        bump(&mut tx, actor, id, version).await?;
        let exp = load(&mut tx, id).await?;
        event(&mut tx, actor, "lot.items", id, exp.version).await?;
        tx.commit().await?;
        Ok(exp)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn item() -> LotItem {
        LotItem {
            descripcion: "Equipo".into(),
            pais_origen: "Honduras".into(),
            fecha_entrega: "30 días".into(),
            cantidad: "2.5".into(),
            unidad: "und".into(),
            transporte: "10.00".into(),
            precio_unitario: "100.00".into(),
        }
    }
    #[test]
    fn decimals_and_isv_follow_the_template_without_binary_floats() {
        let input = LotInput {
            items: vec![item()],
        };
        assert_eq!(input.total(true).unwrap(), "297.50");
        let mut invalid = item();
        invalid.precio_unitario = "1.001".into();
        assert!(
            LotInput {
                items: vec![invalid]
            }
            .total(true)
            .is_err()
        );
        assert_eq!(LotInput { items: vec![] }.total(false).unwrap(), "0.00");
    }
    #[test]
    fn jica_ddp_includes_tax_and_transport_and_rounds_each_item() {
        let mut row = item();
        row.transporte = "0.00".into();
        assert_eq!(
            LotInput {
                items: vec![row.clone()]
            }
            .total_for_style("jica")
            .unwrap(),
            "250.00"
        );
        row.cantidad = "0.005".into();
        row.precio_unitario = "1.00".into();
        assert_eq!(
            LotInput {
                items: vec![row.clone(), row.clone()]
            }
            .total_for_style("jica")
            .unwrap(),
            "0.02"
        );
        row.transporte = "0.01".into();
        assert!(
            LotInput {
                items: vec![row.clone()]
            }
            .total_for_style("jica")
            .is_err()
        );
        row.transporte = "0.00".into();
        row.pais_origen.clear();
        assert!(
            LotInput { items: vec![row] }
                .total_for_style("jica")
                .is_err()
        );
    }
    #[test]
    fn table_font_size_accepts_only_bounded_numeric_points() {
        for size in ["1", "7", "8.5", "100"] {
            assert!(validate_table_font_size(size).is_ok());
        }
        for size in ["0", "101", "7pt", "#panic()", "7.123", "-1", ""] {
            assert!(validate_table_font_size(size).is_err());
        }
    }
}
