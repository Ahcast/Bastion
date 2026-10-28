use crate::{GlobalVariable, GlobalVariablesInput, LotInput, LotItem};
use bastion_core::CoreError;
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;

/// Prepares data files only. The caller must supply an approved template and run
/// the compiler in a separately isolated environment; this function executes nothing.
pub struct TypstLotSources {
    pub files: BTreeMap<String, Vec<u8>>,
    pub offered_lots: Vec<u32>,
}

pub fn prepare_typst_lot_sources(
    variables: &[GlobalVariable],
    lots: &BTreeMap<u32, Vec<LotItem>>,
    total_pages: u32,
    lot_template: &str,
) -> Result<TypstLotSources, CoreError> {
    if total_pages == 0 || lot_template.len() > 100_000 {
        return Err(CoreError::Invalid);
    }
    let input = GlobalVariablesInput {
        variables: variables.to_vec(),
    };
    input.validate()?;
    input.validate_form_config()?;
    let get = |key: &str| -> Result<&str, CoreError> {
        variables
            .iter()
            .find(|v| v.key == key)
            .and_then(|v| v.value.as_ref())
            .and_then(Value::as_str)
            .filter(|v| !v.trim().is_empty())
            .ok_or(CoreError::Invalid)
    };
    let count = get("lotes")?
        .parse::<u32>()
        .map_err(|_| CoreError::Invalid)?;
    let style = get("stylo")?;
    if !(1..=1000).contains(&count) || !matches!(style, "bid" | "std") {
        return Err(CoreError::Invalid);
    }
    let marker = format!("items1_{style}.typ");
    if lot_template.matches(&marker).count() != 1 || lots.keys().any(|n| *n == 0 || *n > count) {
        return Err(CoreError::Invalid);
    }
    let mut data = Map::new();
    for key in ["lpn", "inst", "dia", "mes", "year", "stylo", "descripcion"] {
        data.insert(key.into(), Value::String(get(key)?.to_owned()));
    }
    data.insert("lotes".into(), json!(count));
    data.insert("total_paginas".into(), json!(total_pages));
    let mut files = BTreeMap::new();
    files.insert(
        "config.json".into(),
        serde_json::to_vec(&data).map_err(|_| CoreError::Invalid)?,
    );
    files.insert(
        "config.typ".into(),
        b"#let datos_base = json(\"config.json\")\n".to_vec(),
    );
    let mut offered_lots = Vec::new();
    for number in 1..=count {
        let items = lots.get(&number).cloned().unwrap_or_default();
        LotInput {
            items: items.clone(),
        }
        .total(style == "bid")?;
        let json_name = format!("items{number}_{style}.json");
        let typ_name = format!("items{number}_{style}.typ");
        files.insert(
            json_name.clone(),
            serde_json::to_vec(&items).map_err(|_| CoreError::Invalid)?,
        );
        // All user content is read as data; no value is interpolated into code.
        let country = if style == "bid" {
            "pais_origen: p.pais_origen, transporte: decimal(p.transporte),"
        } else {
            ""
        };
        let source = format!(
            "#let items = json(\"{json_name}\").enumerate().map(((index, p)) => (numero: index + 1, descripcion: p.descripcion, {country} fecha_entrega: p.fecha_entrega, cantidad: decimal(p.cantidad), unidad: p.unidad, precio_unitario: decimal(p.precio_unitario)))\n"
        );
        files.insert(typ_name, source.into_bytes());
        let lot_name = format!("lote{number}-{style}.typ");
        if items.is_empty() {
            files.insert(lot_name, b"#let total_lote = decimal(\"0.00\")\n".to_vec());
        } else {
            offered_lots.push(number);
            files.insert(
                lot_name,
                lot_template
                    .replace(&marker, &format!("items{number}_{style}.typ"))
                    .into_bytes(),
            );
        }
    }
    let mut totals = String::from("#import \"config.typ\": datos_base\n");
    for number in &offered_lots {
        totals.push_str(&format!(
            "#import \"lote{number}-{style}.typ\" as lote{number}\n"
        ));
    }
    totals.push_str("#let totales_lotes = (");
    for number in &offered_lots {
        totals.push_str(&format!(
            "\"total_lote{number}\": lote{number}.total_lote, "
        ));
    }
    if offered_lots.is_empty() {
        totals.push_str(":");
    }
    totals.push_str(")\n#let total_general = totales_lotes.values().sum(default: decimal(\"0.00\"))\n#let datos = datos_base + (totales_lotes: totales_lotes, lpn_total: total_general)\n");
    files.insert("variables.typ".into(), totals.into_bytes());
    Ok(TypstLotSources {
        files,
        offered_lots,
    })
}

/// A versioned database snapshot for the future isolated renderer. Attachment
/// bytes and template approvals are intentionally not captured by this method;
/// the renderer must authorize and verify those independently before use.
pub struct TenderTypstSnapshot {
    pub expediente: crate::Expedient,
    pub order: Vec<crate::PdfOrderItem>,
    pub sources: TypstLotSources,
}
impl crate::TenderService {
    pub async fn prepare_typst_snapshot(&self, actor: u64, id: u64, expected: u64, total_pages: u32, lot_template: &str) -> Result<TenderTypstSnapshot, CoreError> {
        let mut tx = self.begin(actor, "ver").await?;
        // load locks the expediente row: its variables, order and lots are a
        // consistent version while this transaction is reading them.
        let expediente = crate::service::load(&mut tx, id).await?;
        if expediente.version != expected { return Err(CoreError::PreconditionFailed); }
        if expediente.forms.iter().any(|form| !form.missing_required.is_empty()) { return Err(CoreError::Invalid); }
        let rows: Vec<(u32, serde_json::Value)> = sqlx::query_as("SELECT numero,items_json FROM licitaciones_lotes WHERE expediente_id=? ORDER BY numero")
            .bind(id).fetch_all(&mut *tx).await?;
        let mut lots = BTreeMap::new();
        for (number, raw) in rows { lots.insert(number, serde_json::from_value(raw).map_err(|_| CoreError::Invalid)?); }
        let sources = prepare_typst_lot_sources(&expediente.variables, &lots, total_pages, lot_template)?;
        let mut candidates = crate::order::candidates(self, &mut tx, actor, &expediente).await?;
        let stored: Option<serde_json::Value> = sqlx::query_scalar("SELECT pdf_order_json FROM licitaciones_expedientes WHERE id=?")
            .bind(id).fetch_one(&mut *tx).await?;
        let order = crate::order::resolve_order(stored, &mut candidates)?;
        tx.commit().await?;
        Ok(TenderTypstSnapshot { expediente, order, sources })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use bastion_colaboracion::VariableKind;
    fn variable(key: &str, kind: VariableKind, value: &str) -> GlobalVariable {
        GlobalVariable {
            key: key.into(),
            label: key.into(),
            kind,
            value: Some(json!(value)),
        }
    }
    fn variables() -> Vec<GlobalVariable> {
        vec![
            variable("lpn", VariableKind::Text, "#panic(\"input\")"),
            variable("inst", VariableKind::Text, "Entidad"),
            variable("dia", VariableKind::Integer, "1"),
            variable("mes", VariableKind::Text, "enero"),
            variable("year", VariableKind::Integer, "2026"),
            variable("lotes", VariableKind::Integer, "2"),
            variable("stylo", VariableKind::Text, "bid"),
            variable("descripcion", VariableKind::Text, "Prueba"),
        ]
    }
    fn item() -> LotItem {
        LotItem {
            descripcion: "#panic(\"item\")".into(),
            pais_origen: "HN".into(),
            fecha_entrega: "30 días".into(),
            cantidad: "2.5".into(),
            unidad: "und".into(),
            transporte: "10.00".into(),
            precio_unitario: "100.00".into(),
        }
    }
    #[test]
    fn prepares_only_offered_lots_and_keeps_user_input_as_data() {
        let source = prepare_typst_lot_sources(
            &variables(),
            &BTreeMap::from([(2, vec![item()])]),
            7,
            "#import \"items1_bid.typ\": items",
        )
        .unwrap();
        assert_eq!(source.offered_lots, vec![2]);
        let config: Value = serde_json::from_slice(&source.files["config.json"]).unwrap();
        assert_eq!(config["total_paginas"], 7);
        assert_eq!(config["lpn"], "#panic(\"input\")");
        let code = String::from_utf8(source.files["items2_bid.typ"].clone()).unwrap();
        assert!(!code.contains("#panic"));
        let totals = String::from_utf8(source.files["variables.typ"].clone()).unwrap();
        assert!(totals.contains("total_lote2"));
        assert!(!totals.contains("total_lote1"));
        assert!(
            String::from_utf8(source.files["lote2-bid.typ"].clone())
                .unwrap()
                .contains("items2_bid.typ")
        );
    }
    #[test]
    fn rejects_missing_or_invalid_data_and_unexpected_lots() {
        let mut data = variables();
        data.retain(|v| v.key != "inst");
        assert!(prepare_typst_lot_sources(&data, &BTreeMap::new(), 1, "items1_bid.typ").is_err());
        assert!(
            prepare_typst_lot_sources(
                &variables(),
                &BTreeMap::from([(3, vec![])]),
                1,
                "items1_bid.typ"
            )
            .is_err()
        );
        assert!(
            prepare_typst_lot_sources(&variables(), &BTreeMap::new(), 0, "items1_bid.typ").is_err()
        );
        assert!(
            prepare_typst_lot_sources(
                &variables(),
                &BTreeMap::new(),
                1,
                "items1_bid.typ items1_bid.typ"
            )
            .is_err()
        );
    }
    #[test]
    fn generated_sources_compile_with_typst_when_available() {
        let Some(bin) = std::env::var_os("BASTION_TYPST_BIN") else {
            return;
        };
        let root =
            std::env::temp_dir().join(format!("bastion-tender-typst-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let template = "#import \"items1_bid.typ\": items\n#let total_lote = items.map(p => p.cantidad * p.precio_unitario * decimal(\"1.15\") + p.transporte).sum(default: decimal(\"0.00\"))\n";
        let built = prepare_typst_lot_sources(
            &variables(),
            &BTreeMap::from([(2, vec![item()])]),
            7,
            template,
        )
        .unwrap();
        for (name, content) in built.files {
            std::fs::write(root.join(name), content).unwrap();
        }
        std::fs::write(root.join("main.typ"), "#import \"variables.typ\": datos\n#import \"items2_bid.typ\": items\n#assert.eq(datos.lpn_total, decimal(\"297.50\"))\n#assert.eq(items.first().numero, 1)\n#datos.lpn\n").unwrap();
        let output = std::process::Command::new(bin)
            .arg("compile")
            .arg("--root")
            .arg(&root)
            .arg(root.join("main.typ"))
            .arg(root.join("result.pdf"))
            .output()
            .unwrap();
        let _ = std::fs::remove_dir_all(&root);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
