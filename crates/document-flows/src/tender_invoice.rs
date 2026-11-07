use super::*;
use bastion_facturas::{InvoiceInput, TenderOrigin};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TenderInvoiceResult {
    pub target_module: String,
    pub target_id: u64,
    pub award_id: u64,
    pub reused: bool,
}

impl ConversionService {
    pub async fn tender_invoice_link(
        &self,
        actor: u64,
        expedient: u64,
        award_id: u64,
    ) -> Result<Option<TenderInvoiceResult>, CoreError> {
        let mut tx = self.database.pool().begin().await?;
        self.tenders
            .award_source_in_transaction(&mut tx, actor, expedient, award_id, "ver")
            .await?;
        let existing: Option<serde_json::Value> = sqlx::query_scalar(
            "SELECT result_json FROM document_tender_invoices WHERE award_id=? AND expedient_id=?",
        )
        .bind(award_id)
        .bind(expedient)
        .fetch_optional(&mut *tx)
        .await?;
        let result = if let Some(value) = existing {
            let result: TenderInvoiceResult =
                serde_json::from_value(value).map_err(|_| CoreError::Invalid)?;
            self.invoices
                .conversion_target(actor, result.target_id, &mut tx)
                .await?;
            Some(result)
        } else {
            self.invoices.lock_conversion_read(actor, &mut tx).await?;
            None
        };
        tx.commit().await?;
        Ok(result)
    }

    pub async fn tender_invoice(
        &self,
        actor: u64,
        expedient: u64,
        award_id: u64,
        expected: u64,
        key: &str,
        input: InvoiceInput,
    ) -> Result<TenderInvoiceResult, CoreError> {
        if !(16..=128).contains(&key.len())
            || !key
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_'))
        {
            return Err(CoreError::Invalid);
        }
        self.invoices.consume_request(actor).await?;
        let key_hash = Sha256::digest(key.as_bytes()).to_vec();
        let request_hash = Sha256::digest(
            serde_json::to_vec(&(expedient, award_id, expected, &input))
                .map_err(|_| CoreError::Invalid)?,
        )
        .to_vec();
        let mut tx = self.database.pool().begin().await?;
        let award = self
            .tenders
            .award_source_in_transaction(&mut tx, actor, expedient, award_id, "editar")
            .await?;
        self.invoices.lock_conversion(actor, &mut tx).await?;
        let previous: Option<(Vec<u8>, serde_json::Value)> = sqlx::query_as(
            "SELECT request_hash,result_json FROM document_tender_invoice_receipts WHERE actor_id=? AND key_hash=?",
        ).bind(actor).bind(&key_hash).fetch_optional(&mut *tx).await?;
        if let Some((hash, value)) = previous {
            if hash != request_hash {
                return Err(CoreError::Conflict);
            }
            let result: TenderInvoiceResult =
                serde_json::from_value(value).map_err(|_| CoreError::Invalid)?;
            self.invoices
                .conversion_target(actor, result.target_id, &mut tx)
                .await?;
            tx.commit().await?;
            return Ok(result);
        }
        if award.version != expected {
            return Err(CoreError::PreconditionFailed);
        }
        let existing: Option<serde_json::Value> = sqlx::query_scalar(
            "SELECT result_json FROM document_tender_invoices WHERE award_id=? AND expedient_id=?",
        )
        .bind(award_id)
        .bind(expedient)
        .fetch_optional(&mut *tx)
        .await?;
        let result = if let Some(value) = existing {
            let mut result: TenderInvoiceResult =
                serde_json::from_value(value).map_err(|_| CoreError::Invalid)?;
            self.invoices
                .conversion_target(actor, result.target_id, &mut tx)
                .await?;
            result.reused = true;
            result
        } else {
            let calculated = bastion_facturas::calculate(&input)?;
            if calculated.data.items.len() != award.snapshot.items.len() {
                return Err(CoreError::Invalid);
            }
            for (item, selected) in calculated.data.items.iter().zip(&award.snapshot.items) {
                if !same_quantity(&item.quantity, &selected.quantity)? {
                    return Err(CoreError::Invalid);
                }
            }
            let origin = TenderOrigin {
                id: expedient,
                version: award.snapshot.expedient_version,
                name: award.snapshot.expedient_name.clone(),
                award_id,
            };
            let invoice = self
                .invoices
                .create_from_tender(actor, &mut tx, input, origin)
                .await?;
            let result = TenderInvoiceResult {
                target_module: "facturas".into(),
                target_id: invoice.id,
                award_id,
                reused: false,
            };
            sqlx::query("INSERT INTO document_tender_invoices(award_id,expedient_id,invoice_id,snapshot_json,result_json,created_by,created_at) VALUES (?,?,?,?,?,?,UTC_TIMESTAMP(6))")
                .bind(award_id).bind(expedient).bind(invoice.id)
                .bind(serde_json::to_value(&award).map_err(|_| CoreError::Invalid)?)
                .bind(serde_json::to_value(&result).map_err(|_| CoreError::Invalid)?)
                .bind(actor).execute(&mut *tx).await?;
            result
        };
        sqlx::query("INSERT INTO document_tender_invoice_receipts(actor_id,key_hash,request_hash,result_json) VALUES (?,?,?,?)")
            .bind(actor).bind(key_hash).bind(request_hash)
            .bind(serde_json::to_value(&result).map_err(|_| CoreError::Invalid)?)
            .execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(result)
    }
}

fn same_quantity(left: &str, right: &str) -> Result<bool, CoreError> {
    fn canonical(value: &str) -> Result<String, CoreError> {
        let (whole, decimals) = value.split_once('.').unwrap_or((value, ""));
        if whole.is_empty()
            || whole.len() > 12
            || decimals.len() > 12
            || !whole.bytes().all(|c| c.is_ascii_digit())
            || !decimals.bytes().all(|c| c.is_ascii_digit())
        {
            return Err(CoreError::Invalid);
        }
        let whole = whole.trim_start_matches('0');
        Ok(format!(
            "{}.{}",
            if whole.is_empty() { "0" } else { whole },
            decimals.trim_end_matches('0')
        ))
    }
    Ok(canonical(left)? == canonical(right)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn comparison_preserves_awarded_quantity_without_rounding() {
        assert!(same_quantity("1.20", "01.200000000000").unwrap());
        assert!(!same_quantity("1.23", "1.234").unwrap());
        assert!(same_quantity("-1", "1").is_err());
    }
}
