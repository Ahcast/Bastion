use super::*;
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OrderInvoiceResult {
    pub target_module: String,
    pub target_id: u64,
    pub order_id: u64,
    pub order_version: u64,
    pub reused: bool,
}
impl ConversionService {
    pub async fn order_invoice(
        &self,
        actor: u64,
        id: u64,
        expected: u64,
        key: &str,
    ) -> Result<OrderInvoiceResult, CoreError> {
        if !(16..=128).contains(&key.len())
            || !key
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
        {
            return Err(CoreError::Invalid);
        }
        self.invoices.consume_request(actor).await?;
        let key_hash = Sha256::digest(key.as_bytes()).to_vec();
        let request_hash =
            Sha256::digest(serde_json::to_vec(&(id, expected)).map_err(|_| CoreError::Invalid)?)
                .to_vec();
        let mut tx = self.database.pool().begin().await?;
        // Same lock order as quote -> order -> invoice conversions.
        let order = self.orders.conversion_source(actor, id, &mut tx).await?;
        self.invoices.lock_conversion(actor, &mut tx).await?;
        let previous:Option<(Vec<u8>,serde_json::Value)>=sqlx::query_as("SELECT request_hash,result_json FROM document_order_invoice_receipts WHERE actor_id=? AND key_hash=?").bind(actor).bind(&key_hash).fetch_optional(&mut *tx).await?;
        if let Some((hash, response)) = previous {
            if hash != request_hash {
                return Err(CoreError::Conflict);
            }
            let result = serde_json::from_value(response).map_err(|_| CoreError::Invalid)?;
            tx.commit().await?;
            return Ok(result);
        }
        let existing: Option<serde_json::Value> =
            sqlx::query_scalar("SELECT result_json FROM document_order_invoices WHERE order_id=?")
                .bind(id)
                .fetch_optional(&mut *tx)
                .await?;
        let result = if let Some(value) = existing {
            let mut result: OrderInvoiceResult =
                serde_json::from_value(value).map_err(|_| CoreError::Invalid)?;
            result.reused = true;
            result
        } else {
            if order.version != expected {
                return Err(CoreError::PreconditionFailed);
            }
            let invoice = self
                .invoices
                .create_from_order(actor, &order, &mut tx)
                .await?;
            let result = OrderInvoiceResult {
                target_module: "facturas".into(),
                target_id: invoice.id,
                order_id: id,
                order_version: expected,
                reused: false,
            };
            sqlx::query("INSERT INTO document_order_invoices(order_id,invoice_id,snapshot_json,result_json,created_by,created_at) VALUES (?,?,?,?,?,UTC_TIMESTAMP(6))")
                .bind(id).bind(invoice.id).bind(serde_json::to_value(&order).map_err(|_|CoreError::Invalid)?).bind(serde_json::to_value(&result).map_err(|_|CoreError::Invalid)?).bind(actor).execute(&mut *tx).await?;
            result
        };
        sqlx::query("INSERT INTO document_order_invoice_receipts(actor_id,key_hash,request_hash,result_json) VALUES (?,?,?,?)").bind(actor).bind(key_hash).bind(request_hash).bind(serde_json::to_value(&result).map_err(|_|CoreError::Invalid)?).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(result)
    }
}
