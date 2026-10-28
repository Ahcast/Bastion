use crate::service::{bump, editable, event, load};
use crate::{Expedient, TenderService};
use bastion_core::CoreError;
use serde::{Deserialize, Serialize};
use sqlx::{MySql, Transaction};
use std::collections::BTreeSet;

#[derive(Serialize)]
pub struct PdfOrderItem { pub key: String, pub label: String }
#[derive(Serialize)]
pub struct PdfOrder { pub items: Vec<PdfOrderItem>, pub version: u64 }
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PdfOrderInput { pub items: Vec<String> }

async fn candidates(service: &TenderService, tx: &mut Transaction<'_, MySql>, actor: u64, exp: &Expedient) -> Result<Vec<PdfOrderItem>, CoreError> {
    let mut items = Vec::new();
    let style = exp.variables.iter().find(|v| v.key == "stylo").and_then(|v| v.value.as_ref()).and_then(|v| v.as_str());
    if matches!(style, Some("bid" | "std")) { items.push(PdfOrderItem { key:"oferta".into(), label:if style == Some("bid") { "Carta de oferta" } else { "Presentación de oferta" }.into() }); }
    for form in &exp.forms { items.push(PdfOrderItem { key:format!("formulario:{}",form.id), label:form.source.definition.title.clone() }); }
    let files: Vec<(String,String,Option<u64>,Option<String>)> = sqlx::query_as("SELECT id,filename,source_id,object_key FROM licitaciones_archivos WHERE expediente_id=? ORDER BY created_at,id").bind(exp.id).fetch_all(&mut **tx).await?;
    for (id, name, source, object_key) in files {
        let label = if object_key.is_none() {
            let source = source.ok_or(CoreError::Invalid)?;
            match service.library.file_metadata_in_transaction(tx, actor, source).await {
                Ok(_) => name,
                Err(CoreError::NotFound | CoreError::Forbidden) => "Archivo de Colaboración no disponible".into(),
                Err(e) => return Err(e),
            }
        } else { name };
        items.push(PdfOrderItem { key:format!("archivo:{id}"), label });
    }
    let lots: Vec<(u32,serde_json::Value)> = sqlx::query_as("SELECT numero,items_json FROM licitaciones_lotes WHERE expediente_id=? ORDER BY numero").bind(exp.id).fetch_all(&mut **tx).await?;
    for (number, value) in lots {
        if !value.as_array().ok_or(CoreError::Invalid)?.is_empty() { items.push(PdfOrderItem { key:format!("lote:{number}"), label:format!("Oferta económica · lote {number}") }); }
    }
    Ok(items)
}
impl TenderService {
    pub async fn pdf_order(&self, actor: u64, id: u64) -> Result<PdfOrder, CoreError> {
        let mut tx = self.begin(actor, "ver").await?;
        let exp = load(&mut tx,id).await?;
        let mut available = candidates(self, &mut tx, actor, &exp).await?;
        let stored: Option<serde_json::Value> = sqlx::query_scalar("SELECT pdf_order_json FROM licitaciones_expedientes WHERE id=?").bind(id).fetch_one(&mut *tx).await?;
        let keys: Vec<String> = stored.map(serde_json::from_value).transpose().map_err(|_|CoreError::Invalid)?.unwrap_or_default();
        let mut result = Vec::new();
        for key in keys { if let Some(index) = available.iter().position(|item| item.key == key) { result.push(available.remove(index)); } }
        result.extend(available);
        tx.commit().await?;
        Ok(PdfOrder { items: result, version: exp.version })
    }
    pub async fn save_pdf_order(&self, actor: u64, id: u64, expected: u64, input: PdfOrderInput) -> Result<Expedient, CoreError> {
        let mut tx = self.begin(actor, "editar").await?;
        let exp = load(&mut tx,id).await?;
        editable(&exp, expected)?;
        let allowed = candidates(self, &mut tx, actor, &exp).await?;
        let expected_keys: BTreeSet<_> = allowed.iter().map(|item| item.key.as_str()).collect();
        let received: BTreeSet<_> = input.items.iter().map(String::as_str).collect();
        if input.items.len() != allowed.len() || received.len() != input.items.len() || received != expected_keys { return Err(CoreError::Invalid); }
        sqlx::query("UPDATE licitaciones_expedientes SET pdf_order_json=? WHERE id=?").bind(serde_json::to_value(input.items).map_err(|_|CoreError::Invalid)?).bind(id).execute(&mut *tx).await?;
        bump(&mut tx, actor, id, expected).await?;
        let result=load(&mut tx,id).await?;
        event(&mut tx,actor,"pdf.order",id,result.version).await?;
        tx.commit().await?;
        Ok(result)
    }
}
