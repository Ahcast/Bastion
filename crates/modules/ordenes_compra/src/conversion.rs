use crate::{OrderInput, OrderService, PurchaseOrder, SourceQuote};
use bastion_core::{CoreAdmin, CoreError};
use sqlx::{MySql, Transaction};

impl OrderService {
    /// Internal port: caller owns the transaction and locks Cotizaciones first.
    pub async fn enter_conversion(
        &self,
        tx: &mut Transaction<'_, MySql>,
        actor: u64,
    ) -> Result<(), CoreError> {
        self.admin
            .authorize(actor, "ordenes_compra", "documentos", "editar")
            .await?;
        let enabled: Option<bool> = sqlx::query_scalar("SELECT (included_in_release AND environment_allowed AND enabled) FROM module_registry WHERE module_id='ordenes_compra' FOR UPDATE").fetch_optional(&mut **tx).await?;
        if enabled != Some(true) {
            return Err(CoreError::NotFound);
        }
        Ok(())
    }
    pub async fn conversion_receipt(
        &self,
        tx: &mut Transaction<'_, MySql>,
        actor: u64,
        key: &[u8],
        request: &[u8],
    ) -> Result<Option<PurchaseOrder>, CoreError> {
        let previous: Option<(Vec<u8>, serde_json::Value)> = sqlx::query_as("SELECT request_hash,response_json FROM ordenes_conversion_receipts WHERE actor_id=? AND key_hash=?").bind(actor).bind(key).fetch_optional(&mut **tx).await?;
        match previous {
            Some((hash, response)) if hash == request => Ok(Some(
                serde_json::from_value(response).map_err(|_| CoreError::Invalid)?,
            )),
            Some(_) => Err(CoreError::Conflict),
            None => Ok(None),
        }
    }
    pub async fn persist_conversion(
        &self,
        tx: &mut Transaction<'_, MySql>,
        actor: u64,
        key: &[u8],
        request: &[u8],
        input: OrderInput,
        source: SourceQuote,
    ) -> Result<PurchaseOrder, CoreError> {
        let exists: Option<u64> =
            sqlx::query_scalar("SELECT id FROM ordenes_orders WHERE source_quote_id=?")
                .bind(source.id)
                .fetch_optional(&mut **tx)
                .await?;
        if exists.is_some() {
            return Err(CoreError::Conflict);
        }
        let calculation = crate::calculate(&input)?;
        calculation.ensure_saveable()?;
        super::validate_destination(tx, calculation.data.id_destination).await?;
        let id = sqlx::query("INSERT INTO ordenes_orders (status,n_orders,n_bill,client_name,order_date,id_destination,total,data_json,calculation_json,created_by,updated_by,created_at,updated_at,source_quote_id,source_quote_version,source_quote_number) VALUES (?,?,?,?,?,?,?,?,?,?,?,UTC_TIMESTAMP(6),UTC_TIMESTAMP(6),?,?,?)")
            .bind(&calculation.status).bind(&calculation.data.n_orders).bind(&calculation.data.n_bill).bind(&calculation.data.client_name).bind(calculation.data.order_date).bind(calculation.data.id_destination).bind(&calculation.totals.total).bind(serde_json::to_value(&calculation.data).map_err(|_| CoreError::Invalid)?).bind(serde_json::to_value(&calculation).map_err(|_| CoreError::Invalid)?).bind(actor).bind(actor).bind(source.id).bind(source.version).bind(&source.quote_number).execute(&mut **tx).await?.last_insert_id();
        let mut order = super::record(id, 1, calculation);
        order.source_quote = Some(source);
        sqlx::query("INSERT INTO ordenes_conversion_receipts (actor_id,key_hash,request_hash,response_json) VALUES (?,?,?,?)").bind(actor).bind(key).bind(request).bind(serde_json::to_value(&order).map_err(|_| CoreError::Invalid)?).execute(&mut **tx).await?;
        CoreAdmin::record_domain_event(
            tx,
            actor,
            "ordenes_compra",
            "order.from_quote",
            &id.to_string(),
            serde_json::json!({"version":1,"source_quote":order.source_quote}),
        )
        .await?;
        Ok(order)
    }
}
