use super::*;
use crate::ProductStatus;
use bastion_core::DocumentService;

pub(super) fn fresh_lines(data: &mut OrderInput) {
    for row in &mut data.products {
        row.line_id = DocumentService::object_suffix();
    }
}
pub(super) fn hydrate_lines(order: &mut PurchaseOrder) {
    for (index, row) in order.calculation.data.products.iter_mut().enumerate() {
        if row.line_id.is_empty() {
            let hash = Sha256::digest(format!("order:{}:product:{index}", order.id));
            row.line_id = hash[..24].iter().map(|b| format!("{b:02x}")).collect();
        }
    }
    order.data = order.calculation.data.clone();
}
pub(super) async fn prepare_lines(
    tx: &mut Transaction<'_, MySql>,
    current: &PurchaseOrder,
    input: &mut OrderInput,
) -> Result<(), CoreError> {
    let attached: Vec<String> =
        sqlx::query_scalar("SELECT line_id FROM ordenes_documents WHERE order_id=?")
            .bind(current.id)
            .fetch_all(&mut **tx)
            .await?;
    let mut used = std::collections::HashSet::new();
    for (index, row) in input.products.iter_mut().enumerate() {
        if row.line_id.is_empty() {
            row.line_id = if attached.is_empty() {
                current
                    .data
                    .products
                    .get(index)
                    .map(|p| p.line_id.clone())
                    .unwrap_or_else(DocumentService::object_suffix)
            } else {
                DocumentService::object_suffix()
            };
        } else if !current
            .data
            .products
            .iter()
            .any(|p| p.line_id == row.line_id)
        {
            return Err(CoreError::Invalid);
        }
        if !used.insert(row.line_id.clone()) {
            return Err(CoreError::Invalid);
        }
    }
    if attached.iter().any(|id| !used.contains(id)) {
        return Err(CoreError::Conflict);
    }
    Ok(())
}
pub(super) fn validate_key(key: &str) -> Result<(), CoreError> {
    if !(16..=128).contains(&key.len())
        || !key
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        return Err(CoreError::Invalid);
    }
    Ok(())
}
impl OrderService {
    pub async fn conversion_source(
        &self,
        actor: u64,
        id: u64,
        tx: &mut Transaction<'_, MySql>,
    ) -> Result<PurchaseOrder, CoreError> {
        self.lock_conversion(actor, tx).await?;
        load(tx, id).await
    }
    pub async fn clone_order(
        &self,
        actor: u64,
        id: u64,
        expected: u64,
        key: &str,
    ) -> Result<PurchaseOrder, CoreError> {
        validate_key(key)?;
        self.check_access(actor, "documentos", "ver").await?;
        let mut tx = self.begin(actor, "documentos", "editar").await?;
        let source = load(&mut tx, id).await?;
        let key_hash = Sha256::digest(key.as_bytes()).to_vec();
        let request_hash =
            Sha256::digest(serde_json::to_vec(&(id, expected)).map_err(|_| CoreError::Invalid)?)
                .to_vec();
        let previous:Option<(Vec<u8>,serde_json::Value)>=sqlx::query_as("SELECT request_hash,response_json FROM ordenes_clone_receipts WHERE actor_id=? AND key_hash=?").bind(actor).bind(&key_hash).fetch_optional(&mut *tx).await?;
        if let Some((hash, response)) = previous {
            if hash != request_hash {
                return Err(CoreError::Conflict);
            }
            let order = serde_json::from_value(response).map_err(|_| CoreError::Invalid)?;
            tx.commit().await?;
            return Ok(order);
        }
        if source.version != expected {
            return Err(CoreError::PreconditionFailed);
        }
        let mut input = source.data;
        let suffix = format!(
            "-COPIA-{}",
            chrono::Utc::now()
                .with_timezone(&chrono::FixedOffset::west_opt(21600).ok_or(CoreError::Invalid)?)
                .format("%H%M%S")
        );
        input.n_orders = format!(
            "{}{}",
            input
                .n_orders
                .chars()
                .take(50 - suffix.chars().count())
                .collect::<String>(),
            suffix
        );
        input.n_bill.clear();
        input.received_date = None;
        for row in &mut input.products {
            row.status = ProductStatus::Pendiente;
        }
        fresh_lines(&mut input);
        let c = calculate(&input)?;
        c.ensure_saveable()?;
        validate_destination(&mut tx, c.data.id_destination).await?;
        let new_id=sqlx::query("INSERT INTO ordenes_orders (status,n_orders,n_bill,client_name,order_date,id_destination,total,data_json,calculation_json,created_by,updated_by,created_at,updated_at) VALUES (?,?,?,?,?,?,?,?,?,?,?,UTC_TIMESTAMP(6),UTC_TIMESTAMP(6))")
            .bind(&c.status).bind(&c.data.n_orders).bind(&c.data.n_bill).bind(&c.data.client_name).bind(c.data.order_date).bind(c.data.id_destination).bind(&c.totals.total).bind(serde_json::to_value(&c.data).map_err(|_|CoreError::Invalid)?).bind(serde_json::to_value(&c).map_err(|_|CoreError::Invalid)?).bind(actor).bind(actor).execute(&mut *tx).await?.last_insert_id();
        let result = record(new_id, 1, c);
        sqlx::query("INSERT INTO ordenes_clone_receipts(actor_id,key_hash,request_hash,response_json) VALUES (?,?,?,?)").bind(actor).bind(key_hash).bind(request_hash).bind(serde_json::to_value(&result).map_err(|_|CoreError::Invalid)?).execute(&mut *tx).await?;
        CoreAdmin::record_domain_event(
            &mut tx,
            actor,
            "ordenes_compra",
            "order.clone",
            &new_id.to_string(),
            serde_json::json!({"source_order_id":id,"source_version":expected}),
        )
        .await?;
        tx.commit().await?;
        Ok(result)
    }
}
