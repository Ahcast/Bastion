use crate::{
    Destination, DestinationInput, OrderCalculation, OrderFilter, OrderInput, OrderPage,
    OrderSummary, PurchaseOrder, calculate,
};
use crate::{OrderReport, OrderReportFilter, OrderReportRow, OrderReportTotals};
use bastion_core::{CoreAdmin, CoreError, Database};
use sha2::{Digest, Sha256};
use sqlx::{MySql, QueryBuilder, Row, Transaction};

#[path = "conversion.rs"]
mod conversion;
pub use conversion::quote_order_input;
#[path = "actions.rs"]
mod actions;
#[path = "documents.rs"]
mod documents;
pub use documents::*;

#[derive(Clone)]
pub struct OrderService {
    database: Database,
    admin: CoreAdmin,
}
impl OrderService {
    pub fn new(database: Database, admin: CoreAdmin) -> Self {
        Self { database, admin }
    }
    pub async fn migrate(database: &Database) -> Result<(), CoreError> {
        let mut migrator = sqlx::migrate!("./migrations");
        migrator.dangerous_set_table_name("_ordenes_compra_migrations");
        migrator
            .run(database.pool())
            .await
            .map_err(CoreError::Migration)
    }
    pub async fn synchronize_release(&self, allowed: bool) -> Result<(), CoreError> {
        let exists: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM information_schema.tables WHERE table_schema=DATABASE() AND table_name='_ordenes_compra_migrations'").fetch_one(self.database.pool()).await?;
        let ready = if exists > 0 {
            let applied = sqlx::query_as::<_, (i64, bool, Vec<u8>)>(
                "SELECT version,success,checksum FROM _ordenes_compra_migrations",
            )
            .fetch_all(self.database.pool())
            .await?;
            let expected = sqlx::migrate!("./migrations");
            applied.len() == expected.iter().count()
                && expected.iter().all(|m| {
                    applied.iter().any(|(v, ok, hash)| {
                        *v == m.version && *ok && hash.as_slice() == m.checksum.as_ref()
                    })
                })
        } else {
            false
        };
        sqlx::query("INSERT INTO module_registry (module_id,display_name,included_in_release,environment_allowed,enabled) VALUES ('ordenes_compra','Órdenes de compra',1,?,0) ON DUPLICATE KEY UPDATE included_in_release=1,environment_allowed=VALUES(environment_allowed)").bind(allowed && ready).execute(self.database.pool()).await?;
        Ok(())
    }
    async fn begin(
        &self,
        actor: u64,
        scope: &str,
        capability: &str,
    ) -> Result<Transaction<'_, MySql>, CoreError> {
        self.admin
            .authorize(actor, "ordenes_compra", scope, capability)
            .await?;
        let mut tx = self.database.pool().begin().await?;
        let enabled: Option<bool> = sqlx::query_scalar("SELECT (included_in_release AND environment_allowed AND enabled) FROM module_registry WHERE module_id='ordenes_compra' FOR UPDATE").fetch_optional(&mut *tx).await?;
        if enabled != Some(true) {
            return Err(CoreError::NotFound);
        }
        Ok(tx)
    }
    pub async fn check_access(
        &self,
        actor: u64,
        scope: &str,
        capability: &str,
    ) -> Result<(), CoreError> {
        self.begin(actor, scope, capability).await?.commit().await?;
        Ok(())
    }
    async fn begin_destinations(&self, actor: u64) -> Result<Transaction<'_, MySql>, CoreError> {
        match self.begin(actor, "destinos", "ver").await {
            Err(CoreError::Forbidden) => self.begin(actor, "documentos", "ver").await,
            result => result,
        }
    }
    pub async fn check_destinations(&self, actor: u64) -> Result<(), CoreError> {
        self.begin_destinations(actor).await?.commit().await?;
        Ok(())
    }
    pub async fn consume_request(&self, actor: u64) -> Result<(), CoreError> {
        let mut tx = self.database.pool().begin().await?;
        sqlx::query("INSERT INTO ordenes_actor_limits (actor_id,window_start,requests) VALUES (?,UTC_TIMESTAMP(6),1) ON DUPLICATE KEY UPDATE requests=IF(window_start<=UTC_TIMESTAMP(6)-INTERVAL 1 MINUTE,1,requests+1),window_start=IF(window_start<=UTC_TIMESTAMP(6)-INTERVAL 1 MINUTE,UTC_TIMESTAMP(6),window_start)").bind(actor).execute(&mut *tx).await?;
        let count: u32 =
            sqlx::query_scalar("SELECT requests FROM ordenes_actor_limits WHERE actor_id=?")
                .bind(actor)
                .fetch_one(&mut *tx)
                .await?;
        tx.commit().await?;
        if count > 120 {
            return Err(CoreError::Locked);
        }
        Ok(())
    }
    pub async fn list(&self, actor: u64, filter: &OrderFilter) -> Result<OrderPage, CoreError> {
        let (page, page_size) = filter.bounds()?;
        let mut tx = self.begin(actor, "documentos", "ver").await?;
        let mut query = QueryBuilder::<MySql>::new(
            "SELECT id,version,n_orders,n_bill,client_name,CAST(order_date AS CHAR) AS order_date,status,CAST(total AS CHAR) AS total FROM ordenes_orders WHERE 1=1",
        );
        if let Some(term) = filter
            .search
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            query
                .push(" AND (LOCATE(")
                .push_bind(term)
                .push(",client_name)>0 OR LOCATE(")
                .push_bind(term)
                .push(",n_orders)>0 OR LOCATE(")
                .push_bind(term)
                .push(",n_bill)>0)");
        }
        if let Some(status) = &filter.status {
            query.push(" AND status=").push_bind(status);
        }
        if let Some(min) = &filter.min_total {
            query
                .push(" AND total>=CAST(")
                .push_bind(min)
                .push(" AS DECIMAL(16,4))");
        }
        if let Some(max) = &filter.max_total {
            query
                .push(" AND total<=CAST(")
                .push_bind(max)
                .push(" AS DECIMAL(16,4))");
        }
        query
            .push(" ORDER BY created_at DESC,id DESC LIMIT ")
            .push_bind(page_size + 1)
            .push(" OFFSET ")
            .push_bind((page - 1) * page_size);
        let mut items: Vec<OrderSummary> = query.build_query_as().fetch_all(&mut *tx).await?;
        let has_more = items.len() > page_size as usize;
        items.truncate(page_size as usize);
        tx.commit().await?;
        Ok(OrderPage {
            items,
            page,
            page_size,
            has_more,
        })
    }
    pub async fn report(
        &self,
        actor: u64,
        filter: &OrderReportFilter,
    ) -> Result<OrderReport, CoreError> {
        let (page, page_size) = filter.bounds()?;
        let mut tx = self.begin(actor, "documentos", "ver").await?;
        let mut totals_query = QueryBuilder::<MySql>::new(
            "SELECT CAST(COUNT(*) AS UNSIGNED) AS records,CAST(COALESCE(SUM(CAST(JSON_UNQUOTE(JSON_EXTRACT(calculation_json,'$.totals.subtotal')) AS DECIMAL(30,2))),0) AS CHAR) AS subtotal,CAST(COALESCE(SUM(CAST(JSON_UNQUOTE(JSON_EXTRACT(calculation_json,'$.totals.tax')) AS DECIMAL(30,2))),0) AS CHAR) AS isv,CAST(COALESCE(SUM(CAST(JSON_UNQUOTE(JSON_EXTRACT(calculation_json,'$.totals.total')) AS DECIMAL(30,2))),0) AS CHAR) AS total,CAST(COALESCE(SUM(CAST(JSON_UNQUOTE(JSON_EXTRACT(calculation_json,'$.totals.products_investment')) AS DECIMAL(30,2))),0) AS CHAR) AS products_investment,CAST(COALESCE(SUM(CAST(JSON_UNQUOTE(JSON_EXTRACT(calculation_json,'$.totals.other_expenses')) AS DECIMAL(30,2))),0) AS CHAR) AS other_expenses,CAST(COALESCE(SUM(CAST(JSON_UNQUOTE(JSON_EXTRACT(calculation_json,'$.totals.investment_total')) AS DECIMAL(30,2))),0) AS CHAR) AS investment_total,CAST(COALESCE(SUM(CAST(JSON_UNQUOTE(JSON_EXTRACT(calculation_json,'$.totals.profit')) AS DECIMAL(30,2))),0) AS CHAR) AS profit FROM ordenes_orders WHERE order_date>=",
        );
        totals_query
            .push_bind(filter.date_from)
            .push(" AND order_date<=")
            .push_bind(filter.date_to);
        if let Some(term) = filter
            .search
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            totals_query
                .push(" AND (LOCATE(")
                .push_bind(term)
                .push(",n_orders)>0 OR LOCATE(")
                .push_bind(term)
                .push(",n_bill)>0 OR LOCATE(")
                .push_bind(term)
                .push(",client_name)>0)");
        }
        if let Some(status) = &filter.status {
            totals_query.push(" AND status=").push_bind(status);
        }
        let totals: OrderReportTotals = totals_query.build_query_as().fetch_one(&mut *tx).await?;

        let mut rows_query = QueryBuilder::<MySql>::new(
            "SELECT id,n_orders,n_bill,client_name,CAST(order_date AS CHAR) AS order_date,status,JSON_UNQUOTE(JSON_EXTRACT(calculation_json,'$.totals.subtotal')) AS subtotal,JSON_UNQUOTE(JSON_EXTRACT(calculation_json,'$.totals.tax')) AS isv,JSON_UNQUOTE(JSON_EXTRACT(calculation_json,'$.totals.total')) AS total,JSON_UNQUOTE(JSON_EXTRACT(calculation_json,'$.totals.products_investment')) AS products_investment,JSON_UNQUOTE(JSON_EXTRACT(calculation_json,'$.totals.other_expenses')) AS other_expenses,JSON_UNQUOTE(JSON_EXTRACT(calculation_json,'$.totals.investment_total')) AS investment_total,JSON_UNQUOTE(JSON_EXTRACT(calculation_json,'$.totals.profit')) AS profit FROM ordenes_orders WHERE order_date>=",
        );
        rows_query
            .push_bind(filter.date_from)
            .push(" AND order_date<=")
            .push_bind(filter.date_to);
        if let Some(term) = filter
            .search
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            rows_query
                .push(" AND (LOCATE(")
                .push_bind(term)
                .push(",n_orders)>0 OR LOCATE(")
                .push_bind(term)
                .push(",n_bill)>0 OR LOCATE(")
                .push_bind(term)
                .push(",client_name)>0)");
        }
        if let Some(status) = &filter.status {
            rows_query.push(" AND status=").push_bind(status);
        }
        let mut items: Vec<OrderReportRow> = rows_query
            .push(" ORDER BY order_date DESC,id DESC LIMIT ")
            .push_bind(page_size + 1)
            .push(" OFFSET ")
            .push_bind((page - 1) * page_size)
            .build_query_as()
            .fetch_all(&mut *tx)
            .await?;
        let has_more = items.len() > page_size as usize;
        items.truncate(page_size as usize);
        tx.commit().await?;
        Ok(OrderReport {
            items,
            totals,
            page,
            page_size,
            has_more,
        })
    }

    pub async fn detail(&self, actor: u64, id: u64) -> Result<PurchaseOrder, CoreError> {
        let mut tx = self.begin(actor, "documentos", "ver").await?;
        let order = load(&mut tx, id).await?;
        tx.commit().await?;
        Ok(order)
    }
    pub async fn preview(
        &self,
        actor: u64,
        input: &OrderInput,
    ) -> Result<OrderCalculation, CoreError> {
        let tx = self.begin(actor, "documentos", "editar").await?;
        let result = calculate(input)?;
        tx.commit().await?;
        Ok(result)
    }
    pub async fn create(
        &self,
        actor: u64,
        key: &str,
        input: OrderInput,
    ) -> Result<PurchaseOrder, CoreError> {
        if !(16..=128).contains(&key.len())
            || !key
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        {
            return Err(CoreError::Invalid);
        }
        let mut tx = self.begin(actor, "documentos", "editar").await?;
        let mut calculation = calculate(&input)?;
        calculation.ensure_saveable()?;
        let data = serde_json::to_value(&calculation.data).map_err(|_| CoreError::Invalid)?;
        let request_hash =
            Sha256::digest(serde_json::to_vec(&data).map_err(|_| CoreError::Invalid)?).to_vec();
        let key_hash = Sha256::digest(key.as_bytes()).to_vec();
        let previous:Option<(Vec<u8>,serde_json::Value)>=sqlx::query_as("SELECT request_hash,response_json FROM ordenes_create_receipts WHERE actor_id=? AND key_hash=?").bind(actor).bind(&key_hash).fetch_optional(&mut *tx).await?;
        if let Some((hash, response)) = previous {
            if hash != request_hash {
                return Err(CoreError::Conflict);
            }
            let result = serde_json::from_value(response).map_err(|_| CoreError::Invalid)?;
            tx.commit().await?;
            return Ok(result);
        }
        actions::fresh_lines(&mut calculation.data);
        let data = serde_json::to_value(&calculation.data).map_err(|_| CoreError::Invalid)?;
        validate_destination(&mut tx, calculation.data.id_destination).await?;
        let id=sqlx::query("INSERT INTO ordenes_orders (status,n_orders,n_bill,client_name,order_date,id_destination,total,data_json,calculation_json,created_by,updated_by,created_at,updated_at) VALUES (?,?,?,?,?,?,?,?,?,?,?,UTC_TIMESTAMP(6),UTC_TIMESTAMP(6))")
            .bind(&calculation.status).bind(&calculation.data.n_orders).bind(&calculation.data.n_bill).bind(&calculation.data.client_name).bind(calculation.data.order_date).bind(calculation.data.id_destination).bind(&calculation.totals.total).bind(data).bind(serde_json::to_value(&calculation).map_err(|_| CoreError::Invalid)?).bind(actor).bind(actor).execute(&mut *tx).await?.last_insert_id();
        let order = record(id, 1, calculation);
        sqlx::query("INSERT INTO ordenes_create_receipts (actor_id,key_hash,request_hash,response_json) VALUES (?,?,?,?)").bind(actor).bind(key_hash).bind(request_hash).bind(serde_json::to_value(&order).map_err(|_| CoreError::Invalid)?).execute(&mut *tx).await?;
        CoreAdmin::record_domain_event(
            &mut tx,
            actor,
            "ordenes_compra",
            "order.create",
            &id.to_string(),
            serde_json::json!({"version":1}),
        )
        .await?;
        tx.commit().await?;
        Ok(order)
    }
    pub async fn replace(
        &self,
        actor: u64,
        id: u64,
        expected: u64,
        input: OrderInput,
    ) -> Result<PurchaseOrder, CoreError> {
        let mut tx = self.begin(actor, "documentos", "editar").await?;
        let current = load(&mut tx, id).await?;
        if current.version != expected {
            return Err(CoreError::PreconditionFailed);
        }
        let mut input = input;
        actions::prepare_lines(&mut tx, &current, &mut input).await?;
        let calculation = calculate(&input)?;
        calculation.ensure_saveable()?;
        validate_destination(&mut tx, calculation.data.id_destination).await?;
        let version = expected.checked_add(1).ok_or(CoreError::Invalid)?;
        sqlx::query("UPDATE ordenes_orders SET version=?,status=?,n_orders=?,n_bill=?,client_name=?,order_date=?,id_destination=?,total=?,data_json=?,calculation_json=?,updated_by=?,updated_at=UTC_TIMESTAMP(6) WHERE id=?")
            .bind(version).bind(&calculation.status).bind(&calculation.data.n_orders).bind(&calculation.data.n_bill).bind(&calculation.data.client_name).bind(calculation.data.order_date).bind(calculation.data.id_destination).bind(&calculation.totals.total).bind(serde_json::to_value(&calculation.data).map_err(|_| CoreError::Invalid)?).bind(serde_json::to_value(&calculation).map_err(|_| CoreError::Invalid)?).bind(actor).bind(id).execute(&mut *tx).await?;
        CoreAdmin::record_domain_event(
            &mut tx,
            actor,
            "ordenes_compra",
            "order.update",
            &id.to_string(),
            serde_json::json!({"version":version}),
        )
        .await?;
        tx.commit().await?;
        let mut result = record(id, version, calculation);
        result.source_quote = current.source_quote;
        Ok(result)
    }
    pub async fn destinations(&self, actor: u64) -> Result<Vec<Destination>, CoreError> {
        let mut tx = self.begin_destinations(actor).await?;
        let rows=sqlx::query_as("SELECT id,version,destination FROM ordenes_destinations ORDER BY destination,id LIMIT 1000").fetch_all(&mut *tx).await?;
        tx.commit().await?;
        Ok(rows)
    }
    pub async fn save_destination(
        &self,
        actor: u64,
        id: Option<u64>,
        expected: Option<u64>,
        input: DestinationInput,
    ) -> Result<Destination, CoreError> {
        let name = input
            .destination
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        if name.is_empty()
            || input.destination.chars().count() > 255
            || input.destination.chars().any(char::is_control)
        {
            return Err(CoreError::Invalid);
        }
        let mut tx = self.begin(actor, "destinos", "editar").await?;
        let key = name.to_lowercase();
        if key.chars().count() > 255 {
            return Err(CoreError::Invalid);
        }
        let duplicate: Option<u64> =
            sqlx::query_scalar("SELECT id FROM ordenes_destinations WHERE name_key=?")
                .bind(&key)
                .fetch_optional(&mut *tx)
                .await?;
        let (id, version) = if let Some(id) = id {
            let version: u64 = sqlx::query_scalar(
                "SELECT version FROM ordenes_destinations WHERE id=? FOR UPDATE",
            )
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(CoreError::NotFound)?;
            if Some(version) != expected {
                return Err(CoreError::PreconditionFailed);
            }
            if duplicate.is_some_and(|other| other != id) {
                return Err(CoreError::Conflict);
            }
            let version = version.checked_add(1).ok_or(CoreError::Invalid)?;
            sqlx::query(
                "UPDATE ordenes_destinations SET destination=?,name_key=?,version=? WHERE id=?",
            )
            .bind(&name)
            .bind(&key)
            .bind(version)
            .bind(id)
            .execute(&mut *tx)
            .await?;
            (id, version)
        } else if let Some(id) = duplicate {
            let row = sqlx::query_as(
                "SELECT id,version,destination FROM ordenes_destinations WHERE id=?",
            )
            .bind(id)
            .fetch_one(&mut *tx)
            .await?;
            tx.commit().await?;
            return Ok(row);
        } else {
            let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM ordenes_destinations")
                .fetch_one(&mut *tx)
                .await?;
            if count >= 1000 {
                return Err(CoreError::Conflict);
            }
            let id =
                sqlx::query("INSERT INTO ordenes_destinations (destination,name_key) VALUES (?,?)")
                    .bind(&name)
                    .bind(&key)
                    .execute(&mut *tx)
                    .await?
                    .last_insert_id();
            (id, 1)
        };
        CoreAdmin::record_domain_event(
            &mut tx,
            actor,
            "ordenes_compra",
            if expected.is_some() {
                "destination.update"
            } else {
                "destination.create"
            },
            &id.to_string(),
            serde_json::json!({"version":version}),
        )
        .await?;
        tx.commit().await?;
        Ok(Destination {
            id,
            version,
            destination: name,
        })
    }
}
fn record(id: u64, version: u64, calculation: OrderCalculation) -> PurchaseOrder {
    PurchaseOrder {
        source_quote: None,
        id,
        version,
        status: calculation.status.clone(),
        currency: "HNL".into(),
        data: calculation.data.clone(),
        calculation,
    }
}
async fn validate_destination(
    tx: &mut Transaction<'_, MySql>,
    id: Option<u64>,
) -> Result<(), CoreError> {
    let found: Option<u64> = sqlx::query_scalar("SELECT id FROM ordenes_destinations WHERE id=?")
        .bind(id)
        .fetch_optional(&mut **tx)
        .await?;
    if found.is_none() {
        return Err(CoreError::Invalid);
    }
    Ok(())
}
async fn load(tx: &mut Transaction<'_, MySql>, id: u64) -> Result<PurchaseOrder, CoreError> {
    let row =
        sqlx::query("SELECT id,version,calculation_json,source_quote_json FROM ordenes_orders WHERE id=? FOR UPDATE")
            .bind(id)
            .fetch_optional(&mut **tx)
            .await?
            .ok_or(CoreError::NotFound)?;
    let value: serde_json::Value = row.try_get("calculation_json")?;
    let mut order = record(
        row.try_get("id")?,
        row.try_get("version")?,
        serde_json::from_value(value).map_err(|_| CoreError::Invalid)?,
    );
    let origin: Option<serde_json::Value> = row.try_get("source_quote_json")?;
    order.source_quote = origin
        .map(serde_json::from_value)
        .transpose()
        .map_err(|_| CoreError::Invalid)?;
    actions::hydrate_lines(&mut order);
    Ok(order)
}
