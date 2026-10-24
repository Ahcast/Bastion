#![forbid(unsafe_code)]
use bastion_core::{CoreError, Database};
use bastion_cotizaciones::QuoteService;
use bastion_facturas::InvoiceService;
use bastion_ordenes_compra::OrderService;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Clone)]
pub struct ConversionService {
    database: Database,
    quotes: QuoteService,
    invoices: InvoiceService,
    orders: OrderService,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConversionResult {
    pub target_module: String,
    pub target_id: u64,
    pub quote_id: u64,
    pub quote_version: u64,
    pub reused: bool,
}
#[derive(Serialize)]
pub struct OrderPreview {
    pub quote_version: u64,
    pub reason: Option<&'static str>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OrderConversionInput {
    pub n_orders: String,
    pub id_destination: u64,
}

impl ConversionService {
    pub fn new(core: &bastion_core::CoreService) -> Self {
        Self {
            database: core.database(),
            quotes: QuoteService::new(core.database(), core.admin()),
            invoices: InvoiceService::new(core.database(), core.admin()),
            orders: OrderService::new(core.database(), core.admin()),
        }
    }
    pub async fn migrate(database: &Database) -> Result<(), CoreError> {
        let mut migrator = sqlx::migrate!("./migrations");
        migrator.dangerous_set_table_name("_document_flows_migrations");
        migrator
            .run(database.pool())
            .await
            .map_err(CoreError::Migration)
    }
    pub async fn order_preview(
        &self,
        actor: u64,
        id: u64,
        expected: u64,
    ) -> Result<OrderPreview, CoreError> {
        let mut tx = self.database.pool().begin().await?;
        let quote = self.quotes.conversion_source(actor, id, &mut tx).await?;
        self.orders.lock_conversion(actor, &mut tx).await?;
        if quote.version != expected {
            return Err(CoreError::PreconditionFailed);
        }
        let reason = bastion_ordenes_compra::quote_order_input(&quote).err();
        tx.commit().await?;
        Ok(OrderPreview {
            quote_version: quote.version,
            reason,
        })
    }
    pub async fn invoice(
        &self,
        actor: u64,
        id: u64,
        expected: u64,
        key: &str,
    ) -> Result<ConversionResult, CoreError> {
        self.convert(actor, id, expected, key, None).await
    }
    pub async fn order(
        &self,
        actor: u64,
        id: u64,
        expected: u64,
        key: &str,
        input: OrderConversionInput,
    ) -> Result<ConversionResult, CoreError> {
        if input.n_orders.trim().is_empty()
            || input.n_orders.chars().count() > 50
            || input.id_destination == 0
        {
            return Err(CoreError::Invalid);
        }
        self.convert(actor, id, expected, key, Some(input)).await
    }
    async fn convert(
        &self,
        actor: u64,
        id: u64,
        expected: u64,
        key: &str,
        input: Option<OrderConversionInput>,
    ) -> Result<ConversionResult, CoreError> {
        if !(16..=128).contains(&key.len())
            || !key
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
        {
            return Err(CoreError::Invalid);
        }
        let module = if input.is_some() {
            "ordenes_compra"
        } else {
            "facturas"
        };
        // Domain quotas remain in force for the destination, too.
        if input.is_some() {
            self.orders.consume_request(actor).await?;
        } else {
            self.invoices.consume_request(actor).await?;
        }
        let hash = Sha256::digest(
            serde_json::to_vec(&(module, id, expected, &input)).map_err(|_| CoreError::Invalid)?,
        )
        .to_vec();
        let key_hash = Sha256::digest(key.as_bytes()).to_vec();
        let mut tx = self.database.pool().begin().await?;
        let quote = self.quotes.conversion_source(actor, id, &mut tx).await?;
        if input.is_some() {
            self.orders.lock_conversion(actor, &mut tx).await?;
        } else {
            self.invoices.lock_conversion(actor, &mut tx).await?;
        }
        let previous:Option<(Vec<u8>,serde_json::Value)>=sqlx::query_as("SELECT request_hash,result_json FROM document_conversion_receipts WHERE actor_id=? AND key_hash=?")
            .bind(actor).bind(&key_hash).fetch_optional(&mut *tx).await?;
        if let Some((previous, response)) = previous {
            if previous != hash {
                return Err(CoreError::Conflict);
            }
            let result = serde_json::from_value(response).map_err(|_| CoreError::Invalid)?;
            tx.commit().await?;
            return Ok(result);
        }
        // Legacy: a quote has one invoice across actors and keys. Orders can be created explicitly again.
        let existing: Option<(u64, serde_json::Value)> = if input.is_none() {
            sqlx::query_as(
                "SELECT id,result_json FROM document_quote_conversions WHERE invoice_source_id=?",
            )
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?
        } else {
            None
        };
        if let Some((conversion, response)) = existing {
            let mut result: ConversionResult =
                serde_json::from_value(response).map_err(|_| CoreError::Invalid)?;
            result.reused = true;
            result.quote_version = quote.version;
            save_receipt(&mut tx, actor, &key_hash, &hash, conversion, &result).await?;
            tx.commit().await?;
            return Ok(result);
        }
        if quote.version != expected {
            return Err(CoreError::PreconditionFailed);
        }
        let target_id = if let Some(input) = input {
            self.orders
                .create_from_quote(
                    actor,
                    &quote,
                    &input.n_orders,
                    input.id_destination,
                    &mut tx,
                )
                .await?
                .id
        } else {
            self.invoices
                .create_from_quote(actor, &quote, &mut tx)
                .await?
                .id
        };
        let version = self
            .quotes
            .finish_conversion(actor, id, expected, &mut tx)
            .await?;
        let result = ConversionResult {
            target_module: module.into(),
            target_id,
            quote_id: id,
            quote_version: version,
            reused: false,
        };
        let is_invoice = module == "facturas";
        let conversion=sqlx::query("INSERT INTO document_quote_conversions (quote_id,source_id,invoice_source_id,invoice_id,order_id,snapshot_json,result_json,created_by,created_at) VALUES (?,?,?,?,?,?,?,?,UTC_TIMESTAMP(6))")
            .bind(id).bind(id).bind(is_invoice.then_some(id)).bind(is_invoice.then_some(target_id)).bind((!is_invoice).then_some(target_id))
            .bind(serde_json::to_value(&quote).map_err(|_|CoreError::Invalid)?).bind(serde_json::to_value(&result).map_err(|_|CoreError::Invalid)?).bind(actor).execute(&mut *tx).await?.last_insert_id();
        save_receipt(&mut tx, actor, &key_hash, &hash, conversion, &result).await?;
        tx.commit().await?;
        Ok(result)
    }
}
async fn save_receipt(
    tx: &mut sqlx::Transaction<'_, sqlx::MySql>,
    actor: u64,
    key: &[u8],
    hash: &[u8],
    conversion: u64,
    result: &ConversionResult,
) -> Result<(), CoreError> {
    sqlx::query("INSERT INTO document_conversion_receipts (actor_id,key_hash,request_hash,conversion_id,result_json) VALUES (?,?,?,?,?)")
        .bind(actor).bind(key).bind(hash).bind(conversion).bind(serde_json::to_value(result).map_err(|_|CoreError::Invalid)?).execute(&mut **tx).await?;
    Ok(())
}
