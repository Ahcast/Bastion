use super::*;
use base64::{Engine, engine::general_purpose::STANDARD};
use bastion_core::DocumentService;

const TEMPLATE: &str = include_str!("../templates/factura-v1.typ");

#[derive(serde::Serialize)]
pub struct InvoiceDocument {
    pub filename: String,
    pub mime: String,
    pub content_base64: String,
}

impl InvoiceService {
    pub async fn document_readiness(&self, actor: u64) -> Result<serde_json::Value, CoreError> {
        self.check_access(actor, "ver").await?;
        Ok(
            serde_json::json!({"pdf":DocumentService::from_environment().configured() && self.admin.document_company().await.is_ok()}),
        )
    }
    pub async fn document(
        &self,
        actor: u64,
        id: u64,
        expected: u64,
        kind: &str,
    ) -> Result<InvoiceDocument, CoreError> {
        if !matches!(kind, "invoice" | "delivery" | "receipt") {
            return Err(CoreError::Invalid);
        }
        let current = self.detail(actor, id).await?;
        if current.version != expected {
            return Err(CoreError::PreconditionFailed);
        }
        let storage = DocumentService::from_environment();
        if !storage.configured() {
            return Err(CoreError::Overloaded);
        }
        let company = if let Some(company) = &current.company_snapshot {
            company.clone()
        } else {
            self.admin.document_company().await?
        };
        let items=current.data.items.iter().zip(&current.calculation.items).map(|(row,c)|serde_json::json!({"description":row.description,"quantity":row.quantity,"unit_price":row.unit_price,"discount":c.discount_amount,"total":c.total})).collect::<Vec<_>>();
        let updated:Option<chrono::NaiveDateTime>=sqlx::query_scalar("SELECT updated_at FROM facturas_drafts WHERE id=? AND version=? AND deleted_at IS NULL").bind(id).bind(expected).fetch_optional(self.database.pool()).await?;
        let payload = serde_json::json!({"kind":kind,"number":current.invoice_number.clone().unwrap_or_else(||format!("Borrador {id}")),"issued":current.invoice_number.is_some(),"status":current.status,"date":current.data.invoice_date.to_string(),"payment_term_type":current.data.payment_term_type,"credit_days":current.data.credit_days,"discount_type":current.data.discount_type,"discount_value":current.data.discount_value,"purchase_order_exempt_number":current.data.purchase_order_exempt_number,"registration_certificate_number":current.data.registration_certificate_number,"sag_registration_number":current.data.sag_registration_number,"company":company,"client_name":current.data.client_name,"client_rtn":current.data.client_rtn,"order_number":current.data.order_number,"public_notes":current.data.public_notes,"totals":current.calculation.totals,"paid_total":current.calculation.paid_total,"balance":current.calculation.balance,"items":items,"payments":current.payments,"sar":current.sar_snapshot,"logo":storage.branding().0,"creation_timestamp":updated.ok_or(CoreError::PreconditionFailed)?.and_utc().timestamp(),"branding_fingerprint":storage.branding_fingerprint()?});
        let mut identity = serde_json::to_vec(&payload).map_err(|_| CoreError::Invalid)?;
        identity.extend_from_slice(TEMPLATE.as_bytes());
        let prefix = format!(
            "facturas/{id}/{expected}/{kind}-v1-{}-",
            hex::encode(Sha256::digest(identity))
        );
        let previous:Option<(String,Vec<u8>)>=sqlx::query_as("SELECT object_key,sha256 FROM facturas_documents WHERE invoice_id=? AND invoice_version=? AND kind=? AND object_key LIKE ? ORDER BY object_key LIMIT 1").bind(id).bind(expected).bind(kind).bind(format!("{prefix}%")).fetch_optional(self.database.pool()).await?;
        let key = previous
            .as_ref()
            .map(|(k, _)| k.clone())
            .unwrap_or_else(|| format!("{prefix}{}.pdf", DocumentService::object_suffix()));
        let filename = format!(
            "{kind}-{}.pdf",
            current
                .invoice_number
                .as_deref()
                .map(str::to_owned)
                .unwrap_or_else(|| format!("borrador-{id}"))
        );
        let bytes = if let Some((_, hash)) = previous {
            let bytes = storage.get(key.clone()).await?;
            if Sha256::digest(&bytes).as_slice() != hash.as_slice() {
                return Err(CoreError::Overloaded);
            }
            bytes
        } else {
            let bytes = storage.render(TEMPLATE, payload).await?;
            storage.put(key.clone(), bytes.clone()).await?;
            let persist=async {
                let mut tx=self.begin(actor,"ver").await?;let latest=load(&mut tx,id).await?;
                if latest.version!=expected {return Err(CoreError::PreconditionFailed);}
                sqlx::query("INSERT INTO facturas_documents(object_key,invoice_id,invoice_version,kind,filename,sha256,byte_size,created_at) VALUES (?,?,?,?,?,?,?,UTC_TIMESTAMP(6))").bind(&key).bind(id).bind(expected).bind(kind).bind(&filename).bind(Sha256::digest(&bytes).to_vec()).bind(bytes.len() as u32).execute(&mut *tx).await?;
                CoreAdmin::record_domain_event(&mut tx,actor,"facturas","invoice.document",&id.to_string(),serde_json::json!({"version":expected,"kind":kind,"template":"factura-v1"})).await?;tx.commit().await?;Ok::<(),CoreError>(())
            }.await;
            if let Err(error) = persist {
                sqlx::query("INSERT IGNORE INTO facturas_document_deletions(object_key,available_at) VALUES (?,UTC_TIMESTAMP(6)+INTERVAL 5 MINUTE)").bind(&key).execute(self.database.pool()).await?;
                return Err(error);
            }
            bytes
        };
        let latest = self.detail(actor, id).await?;
        if latest.version != expected {
            return Err(CoreError::PreconditionFailed);
        }
        Ok(InvoiceDocument {
            filename,
            mime: "application/pdf".into(),
            content_base64: STANDARD.encode(bytes),
        })
    }

    pub async fn clean_one_document(&self) -> Result<bool, CoreError> {
        let mut tx = self.database.pool().begin().await?;
        let key:Option<String>=sqlx::query_scalar("SELECT object_key FROM facturas_document_deletions WHERE available_at<=UTC_TIMESTAMP(6) ORDER BY available_at LIMIT 1 FOR UPDATE").fetch_optional(&mut *tx).await?;
        let Some(key) = key else {
            tx.commit().await?;
            return Ok(false);
        };
        sqlx::query("UPDATE facturas_document_deletions SET attempts=attempts+1,available_at=UTC_TIMESTAMP(6)+INTERVAL 5 MINUTE WHERE object_key=?").bind(&key).execute(&mut *tx).await?;
        tx.commit().await?;
        let live: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM facturas_documents WHERE object_key=?")
                .bind(&key)
                .fetch_one(self.database.pool())
                .await?;
        if live == 0 {
            DocumentService::from_environment()
                .remove(key.clone())
                .await?;
        }
        sqlx::query("DELETE FROM facturas_document_deletions WHERE object_key=?")
            .bind(key)
            .execute(self.database.pool())
            .await?;
        Ok(true)
    }
}
