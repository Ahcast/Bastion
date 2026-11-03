use crate::service::{bump, editable, event, load};
use crate::{Expedient, TenderService};
use bastion_core::CoreError;
use serde::{Deserialize, Serialize};
use sqlx::{MySql, Transaction};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Serialize)]
pub struct PdfOrderItem {
    pub key: String,
    pub label: String,
    pub title_editable: bool,
}
#[derive(Serialize)]
pub struct PdfOrder {
    pub items: Vec<PdfOrderItem>,
    pub version: u64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PdfOrderInput {
    pub items: Vec<String>,
    #[serde(default)]
    pub titles: Option<BTreeMap<String, String>>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum StoredEntry {
    Key(String),
    Titled { key: String, label: String },
}

fn title(value: &str) -> Result<String, CoreError> {
    let text = value.trim();
    if text.is_empty() || text.chars().count() > 1000 || text.chars().any(char::is_control) {
        return Err(CoreError::Invalid);
    }
    Ok(text.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn item(key: &str, label: &str, editable: bool) -> PdfOrderItem {
        PdfOrderItem {
            key: key.into(),
            label: label.into(),
            title_editable: editable,
        }
    }
    #[test]
    fn legacy_orders_use_document_titles_and_new_orders_keep_custom_titles() {
        let mut available = vec![
            item("oferta", "Carta", true),
            item("archivo:1", "Título de Colaboración", true),
        ];
        let legacy = resolve_order(
            Some(serde_json::json!(["archivo:1", "oferta"])),
            &mut available,
        )
        .unwrap();
        assert_eq!(legacy[0].label, "Título de Colaboración");
        let mut available = vec![
            item("oferta", "Carta", true),
            item("archivo:1", "Título de Colaboración", true),
        ];
        let custom = resolve_order(
            Some(serde_json::json!([{"key":"archivo:1","label":"  Título editado  "},"oferta"])),
            &mut available,
        )
        .unwrap();
        assert_eq!(custom[0].label, "Título editado");
        let mut unavailable = vec![item("archivo:1", "No disponible", false)];
        let redacted = resolve_order(
            Some(serde_json::json!([{"key":"archivo:1","label":"Título anterior"}])),
            &mut unavailable,
        )
        .unwrap();
        assert_eq!(redacted[0].label, "No disponible");
        assert!(title(" ").is_err());
        assert!(title("línea\nsiguiente").is_err());
        assert!(title(&"a".repeat(1001)).is_err());
    }
}

pub(crate) async fn candidates(
    service: &TenderService,
    tx: &mut Transaction<'_, MySql>,
    actor: u64,
    exp: &Expedient,
) -> Result<Vec<PdfOrderItem>, CoreError> {
    let mut items = Vec::new();
    let style = exp
        .variables
        .iter()
        .find(|v| v.key == "stylo")
        .and_then(|v| v.value.as_ref())
        .and_then(|v| v.as_str());
    if matches!(style, Some("bid" | "std")) {
        items.push(PdfOrderItem {
            title_editable: true,
            key: "oferta".into(),
            label: if style == Some("bid") {
                "Carta de oferta"
            } else {
                "Presentación de oferta"
            }
            .into(),
        });
    }
    for key in &exp.initial_forms {
        let label = match key.as_str() {
            "integridad" => "Declaración de integridad",
            "oferente" => "Información sobre el oferente",
            _ => return Err(CoreError::Invalid),
        };
        items.push(PdfOrderItem {
            title_editable: true,
            key: format!("inicial:{key}"),
            label: label.into(),
        });
    }
    for form in &exp.forms {
        items.push(PdfOrderItem {
            title_editable: true,
            key: format!("formulario:{}", form.id),
            label: form.source.definition.title.clone(),
        });
    }
    let files: Vec<(String,String,Option<u64>,Option<String>)> = sqlx::query_as("SELECT id,filename,source_id,object_key FROM licitaciones_archivos WHERE expediente_id=? ORDER BY created_at,id").bind(exp.id).fetch_all(&mut **tx).await?;
    for (id, name, source, object_key) in files {
        let mut title_editable = true;
        let label = if object_key.is_none() {
            let source = source.ok_or(CoreError::Invalid)?;
            match service
                .library
                .file_metadata_in_transaction(tx, actor, source)
                .await
            {
                Ok((source, _)) => source.title,
                Err(CoreError::NotFound | CoreError::Forbidden) => {
                    title_editable = false;
                    "Archivo de Colaboración no disponible".into()
                }
                Err(e) => return Err(e),
            }
        } else {
            name
        };
        items.push(PdfOrderItem {
            title_editable,
            key: format!("archivo:{id}"),
            label,
        });
    }
    let lots: Vec<(u32, serde_json::Value)> = sqlx::query_as(
        "SELECT numero,items_json FROM licitaciones_lotes WHERE expediente_id=? ORDER BY numero",
    )
    .bind(exp.id)
    .fetch_all(&mut **tx)
    .await?;
    for (number, value) in lots {
        if !value.as_array().ok_or(CoreError::Invalid)?.is_empty() {
            items.push(PdfOrderItem {
                title_editable: true,
                key: format!("lote:{number}"),
                label: format!("Oferta económica · lote {number}"),
            });
        }
    }
    Ok(items)
}
pub(crate) fn resolve_order(
    stored: Option<serde_json::Value>,
    available: &mut Vec<PdfOrderItem>,
) -> Result<Vec<PdfOrderItem>, CoreError> {
    let entries: Vec<StoredEntry> = stored
        .map(serde_json::from_value)
        .transpose()
        .map_err(|_| CoreError::Invalid)?
        .unwrap_or_default();
    let mut result = Vec::new();
    for entry in entries {
        let (key, label) = match entry {
            StoredEntry::Key(key) => (key, None),
            StoredEntry::Titled { key, label } => (key, Some(title(&label)?)),
        };
        if let Some(index) = available.iter().position(|item| item.key == key) {
            let mut item = available.remove(index);
            if item.title_editable {
                if let Some(label) = label {
                    item.label = label;
                }
            }
            result.push(item);
        }
    }
    result.append(available);
    Ok(result)
}
impl TenderService {
    pub async fn pdf_order(&self, actor: u64, id: u64) -> Result<PdfOrder, CoreError> {
        let mut tx = self.begin(actor, "ver").await?;
        let exp = load(&mut tx, id).await?;
        let mut available = candidates(self, &mut tx, actor, &exp).await?;
        let stored: Option<serde_json::Value> =
            sqlx::query_scalar("SELECT pdf_order_json FROM licitaciones_expedientes WHERE id=?")
                .bind(id)
                .fetch_one(&mut *tx)
                .await?;
        let result = resolve_order(stored, &mut available)?;
        tx.commit().await?;
        Ok(PdfOrder {
            items: result,
            version: exp.version,
        })
    }
    pub async fn save_pdf_order(
        &self,
        actor: u64,
        id: u64,
        expected: u64,
        input: PdfOrderInput,
    ) -> Result<Expedient, CoreError> {
        let mut tx = self.begin(actor, "editar").await?;
        let exp = load(&mut tx, id).await?;
        editable(&exp, expected)?;
        let mut allowed = candidates(self, &mut tx, actor, &exp).await?;
        let expected_keys: BTreeSet<_> = allowed.iter().map(|item| item.key.as_str()).collect();
        let received: BTreeSet<_> = input.items.iter().map(String::as_str).collect();
        if input.items.len() != allowed.len()
            || received.len() != input.items.len()
            || received != expected_keys
        {
            return Err(CoreError::Invalid);
        }
        let stored: Option<serde_json::Value> =
            sqlx::query_scalar("SELECT pdf_order_json FROM licitaciones_expedientes WHERE id=?")
                .bind(id)
                .fetch_one(&mut *tx)
                .await?;
        let mut ordered = resolve_order(stored, &mut allowed)?;
        if let Some(titles) = input.titles {
            for (key, value) in titles {
                let item = ordered
                    .iter_mut()
                    .find(|item| item.key == key)
                    .ok_or(CoreError::Invalid)?;
                let value = title(&value)?;
                if item.title_editable {
                    item.label = value;
                }
            }
        }
        let entries = input
            .items
            .iter()
            .map(|key| {
                let item = ordered
                    .iter()
                    .find(|item| &item.key == key)
                    .ok_or(CoreError::Invalid)?;
                Ok(if item.title_editable {
                    serde_json::json!({"key":item.key,"label":item.label})
                } else {
                    serde_json::json!(item.key)
                })
            })
            .collect::<Result<Vec<_>, CoreError>>()?;
        sqlx::query("UPDATE licitaciones_expedientes SET pdf_order_json=? WHERE id=?")
            .bind(serde_json::to_value(entries).map_err(|_| CoreError::Invalid)?)
            .bind(id)
            .execute(&mut *tx)
            .await?;
        bump(&mut tx, actor, id, expected).await?;
        let result = load(&mut tx, id).await?;
        event(&mut tx, actor, "pdf.order", id, result.version).await?;
        tx.commit().await?;
        Ok(result)
    }
}
