use crate::{InvoiceReport, InvoiceReportFilter, InvoiceReportRow, InvoiceReportTotals};
use bastion_core::{
    CoreAdmin, CoreError, DashboardPeriod, DashboardPoint, DashboardWindow, Database,
};
use sha2::{Digest, Sha256};
use sqlx::{MySql, QueryBuilder, Row, Transaction};

use crate::{
    InvoiceCalculation, InvoiceDraft, InvoiceFilter, InvoiceInput, InvoicePage, InvoiceSummary,
    calculate,
};

#[derive(serde::Serialize, sqlx::FromRow)]
struct InvoiceDashboardState {
    label: String,
    count: i64,
}
#[derive(serde::Serialize, sqlx::FromRow)]
struct InvoiceDashboardRow {
    id: u64,
    order_number: String,
    client_name: String,
    invoice_date: String,
    status: String,
    total: Option<String>,
    balance: Option<String>,
}
fn invoice_dashboard_query(select: &str, window: &DashboardWindow) -> QueryBuilder<MySql> {
    let mut query = QueryBuilder::new(select.to_owned());
    query.push(" FROM facturas_drafts WHERE deleted_at IS NULL");
    if let Some(from) = window.from {
        query.push(" AND invoice_date>=").push_bind(from);
    }
    if let Some(until) = window.until {
        query.push(" AND invoice_date<").push_bind(until);
    }
    query
}

#[path = "actions.rs"]
mod actions;
#[path = "conversion.rs"]
mod conversion;
#[path = "documents.rs"]
mod documents;

#[derive(Clone)]
pub struct InvoiceService {
    database: Database,
    admin: CoreAdmin,
}

impl InvoiceService {
    pub fn new(database: Database, admin: CoreAdmin) -> Self {
        Self { database, admin }
    }

    pub async fn migrate(database: &Database) -> Result<(), CoreError> {
        let mut migrator = sqlx::migrate!("./migrations");
        migrator.dangerous_set_table_name("_facturas_migrations");
        migrator
            .run(database.pool())
            .await
            .map_err(CoreError::Migration)
    }

    pub async fn synchronize_release(&self, allowed: bool) -> Result<(), CoreError> {
        let exists: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM information_schema.tables WHERE table_schema=DATABASE() AND table_name='_facturas_migrations'").fetch_one(self.database.pool()).await?;
        let ready = if exists > 0 {
            let applied = sqlx::query_as::<_, (i64, bool, Vec<u8>)>(
                "SELECT version,success,checksum FROM _facturas_migrations",
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
        sqlx::query("INSERT INTO module_registry (module_id,display_name,included_in_release,environment_allowed,enabled) VALUES ('facturas','Facturas',1,?,0) ON DUPLICATE KEY UPDATE included_in_release=1,environment_allowed=VALUES(environment_allowed)")
            .bind(allowed && ready).execute(self.database.pool()).await?;
        Ok(())
    }

    async fn begin(
        &self,
        actor: u64,
        capability: &str,
    ) -> Result<Transaction<'_, MySql>, CoreError> {
        self.admin
            .authorize(actor, "facturas", "documentos", capability)
            .await?;
        let mut tx = self.database.pool().begin().await?;
        let enabled: Option<bool> = sqlx::query_scalar("SELECT (included_in_release AND environment_allowed AND enabled) FROM module_registry WHERE module_id='facturas' FOR UPDATE").fetch_optional(&mut *tx).await?;
        if enabled != Some(true) {
            return Err(CoreError::NotFound);
        }
        Ok(tx)
    }

    pub async fn check_access(&self, actor: u64, capability: &str) -> Result<(), CoreError> {
        self.begin(actor, capability).await?.commit().await?;
        Ok(())
    }

    pub async fn consume_request(&self, actor: u64) -> Result<(), CoreError> {
        let mut tx = self.database.pool().begin().await?;
        sqlx::query("INSERT INTO facturas_actor_limits (actor_id,window_start,requests) VALUES (?,UTC_TIMESTAMP(6),1) ON DUPLICATE KEY UPDATE requests=IF(window_start<=UTC_TIMESTAMP(6)-INTERVAL 1 MINUTE,1,requests+1),window_start=IF(window_start<=UTC_TIMESTAMP(6)-INTERVAL 1 MINUTE,UTC_TIMESTAMP(6),window_start)")
            .bind(actor).execute(&mut *tx).await?;
        let count: u32 =
            sqlx::query_scalar("SELECT requests FROM facturas_actor_limits WHERE actor_id=?")
                .bind(actor)
                .fetch_one(&mut *tx)
                .await?;
        tx.commit().await?;
        if count > 120 {
            return Err(CoreError::Locked);
        }
        Ok(())
    }

    pub async fn list(&self, actor: u64, filter: &InvoiceFilter) -> Result<InvoicePage, CoreError> {
        let (page, page_size) = filter.bounds()?;
        let mut tx = self.begin(actor, "ver").await?;
        let mut query = QueryBuilder::<MySql>::new(
            "SELECT id,version,invoice_number,JSON_UNQUOTE(JSON_EXTRACT(calculation_json,'$.paid_total')) AS paid_total,client_name,order_number,CAST(invoice_date AS CHAR) AS invoice_date,CAST(JSON_LENGTH(data_json,'$.items') AS UNSIGNED) AS item_count,status,JSON_UNQUOTE(JSON_EXTRACT(calculation_json,'$.totals.total')) AS total,JSON_UNQUOTE(JSON_EXTRACT(calculation_json,'$.balance')) AS balance FROM facturas_drafts WHERE deleted_at IS NULL",
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
                .push(",order_number)>0 OR LOCATE(")
                .push_bind(term)
                .push(",invoice_number)>0)");
        }
        query
            .push(" ORDER BY created_at DESC,id DESC LIMIT ")
            .push_bind(page_size + 1)
            .push(" OFFSET ")
            .push_bind((page - 1) * page_size);
        let mut items: Vec<InvoiceSummary> = query.build_query_as().fetch_all(&mut *tx).await?;
        let has_more = items.len() > page_size as usize;
        items.truncate(page_size as usize);
        tx.commit().await?;
        Ok(InvoicePage {
            items,
            page,
            page_size,
            has_more,
        })
    }

    pub async fn report(
        &self,
        actor: u64,
        filter: &InvoiceReportFilter,
    ) -> Result<InvoiceReport, CoreError> {
        let (page, page_size) = filter.bounds()?;
        let mut tx = self.begin(actor, "ver").await?;
        let mut totals_query = QueryBuilder::<MySql>::new(
            "SELECT CAST(COUNT(*) AS UNSIGNED) AS records,CAST(COALESCE(SUM(CAST(JSON_UNQUOTE(JSON_EXTRACT(calculation_json,'$.totals.total')) AS DECIMAL(30,2))),0) AS CHAR) AS total,CAST(COALESCE(SUM(CAST(JSON_UNQUOTE(JSON_EXTRACT(calculation_json,'$.paid_total')) AS DECIMAL(30,2))),0) AS CHAR) AS paid_total,CAST(COALESCE(SUM(CAST(JSON_UNQUOTE(JSON_EXTRACT(calculation_json,'$.balance')) AS DECIMAL(30,2))),0) AS CHAR) AS balance FROM facturas_drafts WHERE deleted_at IS NULL AND invoice_date>=",
        );
        totals_query
            .push_bind(filter.date_from)
            .push(" AND invoice_date<=")
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
                .push(",client_name)>0 OR LOCATE(")
                .push_bind(term)
                .push(",order_number)>0 OR LOCATE(")
                .push_bind(term)
                .push(",invoice_number)>0)");
        }
        if let Some(status) = &filter.status {
            totals_query.push(" AND status=").push_bind(status);
        }
        let totals: InvoiceReportTotals = totals_query.build_query_as().fetch_one(&mut *tx).await?;

        let mut rows_query = QueryBuilder::<MySql>::new(
            "SELECT id,invoice_number,client_name,order_number,CAST(invoice_date AS CHAR) AS invoice_date,status,JSON_UNQUOTE(JSON_EXTRACT(calculation_json,'$.totals.total')) AS total,JSON_UNQUOTE(JSON_EXTRACT(calculation_json,'$.paid_total')) AS paid_total,JSON_UNQUOTE(JSON_EXTRACT(calculation_json,'$.balance')) AS balance FROM facturas_drafts WHERE deleted_at IS NULL AND invoice_date>=",
        );
        rows_query
            .push_bind(filter.date_from)
            .push(" AND invoice_date<=")
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
                .push(",client_name)>0 OR LOCATE(")
                .push_bind(term)
                .push(",order_number)>0 OR LOCATE(")
                .push_bind(term)
                .push(",invoice_number)>0)");
        }
        if let Some(status) = &filter.status {
            rows_query.push(" AND status=").push_bind(status);
        }
        let mut items: Vec<InvoiceReportRow> = rows_query
            .push(" ORDER BY invoice_date DESC,id DESC LIMIT ")
            .push_bind(page_size + 1)
            .push(" OFFSET ")
            .push_bind((page - 1) * page_size)
            .build_query_as()
            .fetch_all(&mut *tx)
            .await?;
        let has_more = items.len() > page_size as usize;
        items.truncate(page_size as usize);
        tx.commit().await?;
        Ok(InvoiceReport {
            items,
            totals,
            page,
            page_size,
            has_more,
        })
    }

    pub async fn dashboard(
        &self,
        actor: u64,
        period: DashboardPeriod,
    ) -> Result<serde_json::Value, CoreError> {
        let now = chrono::Utc::now();
        let window = period.window(now)?;
        let mut tx = self.begin(actor, "ver").await?;
        let total_sql = "JSON_UNQUOTE(JSON_EXTRACT(calculation_json,'$.totals.total'))";
        let metrics = invoice_dashboard_query(
            &format!("SELECT COUNT(*) AS total,COUNT({total_sql}) AS valued,CAST(COALESCE(SUM(CAST({total_sql} AS DECIMAL(50,2))),0) AS CHAR) AS amount"),
            &window,
        )
        .build()
        .fetch_one(&mut *tx)
        .await?;
        let total: i64 = metrics.try_get("total")?;
        let valued: i64 = metrics.try_get("valued")?;
        let amount: String = metrics.try_get("amount")?;
        let mut states =
            invoice_dashboard_query("SELECT status AS label,COUNT(*) AS count", &window);
        states.push(" GROUP BY status ORDER BY status LIMIT 20");
        let states = states
            .build_query_as::<InvoiceDashboardState>()
            .fetch_all(&mut *tx)
            .await?;
        let mut chart = QueryBuilder::<MySql>::new("SELECT DATE_FORMAT(invoice_date,");
        chart.push_bind(period.sql_format()).push(format!(
            ") AS bucket,COUNT(*) AS count,CAST(COALESCE(SUM(CAST({total_sql} AS DECIMAL(50,2))),0) AS CHAR) AS amount FROM facturas_drafts WHERE deleted_at IS NULL"
        ));
        if let Some(from) = window.from {
            chart.push(" AND invoice_date>=").push_bind(from);
        }
        if let Some(until) = window.until {
            chart.push(" AND invoice_date<").push_bind(until);
        }
        chart.push(" GROUP BY bucket ORDER BY bucket LIMIT 121");
        let mut points = chart
            .build_query_as::<DashboardPoint>()
            .fetch_all(&mut *tx)
            .await?;
        let chart_truncated = points.len() > 120;
        points.truncate(120);
        let points = window.fill_series(points, true)?;
        let recent = invoice_dashboard_query("SELECT id,order_number,client_name,CAST(invoice_date AS CHAR) AS invoice_date,status,JSON_UNQUOTE(JSON_EXTRACT(calculation_json,'$.totals.total')) AS total,JSON_UNQUOTE(JSON_EXTRACT(calculation_json,'$.balance')) AS balance", &window)
            .push(" ORDER BY invoice_date DESC,id DESC LIMIT 8")
            .build_query_as::<InvoiceDashboardRow>()
            .fetch_all(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(serde_json::json!({
            "window": window,
            "as_of": now.to_rfc3339(),
            "metrics": {"total": total, "amount": amount, "without_total": total - valued},
            "series": points,
            "chart_truncated": chart_truncated,
            "distribution": states,
            "recent": recent
        }))
    }

    pub async fn detail(&self, actor: u64, id: u64) -> Result<InvoiceDraft, CoreError> {
        let mut tx = self.begin(actor, "ver").await?;
        let draft = load(&mut tx, id).await?;
        tx.commit().await?;
        Ok(draft)
    }

    pub async fn preview(
        &self,
        actor: u64,
        input: &InvoiceInput,
    ) -> Result<InvoiceCalculation, CoreError> {
        let tx = self.begin(actor, "editar").await?;
        let mut result = calculate(input)?;
        if let Some(payment) = input.initial_payment.clone() {
            let payment = actions::normalize_payment(payment)?;
            actions::settle(&mut result, crate::calculation::cents(&payment.amount)?)?;
        }
        tx.commit().await?;
        Ok(result)
    }

    pub async fn create(
        &self,
        actor: u64,
        key: &str,
        mut input: InvoiceInput,
    ) -> Result<InvoiceDraft, CoreError> {
        if !(16..=128).contains(&key.len())
            || !key
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        {
            return Err(CoreError::Invalid);
        }
        let mut tx = self.begin(actor, "editar").await?;
        let payment = input
            .initial_payment
            .take()
            .map(actions::normalize_payment)
            .transpose()?;
        let mut calculation = calculate(&input)?;
        if let Some(payment) = &payment {
            actions::settle(
                &mut calculation,
                crate::calculation::cents(&payment.amount)?,
            )?;
        }
        calculation.ensure_saveable()?;
        let data = serde_json::to_value(&calculation.data).map_err(|_| CoreError::Invalid)?;
        let hash_data = if payment.is_some() {
            serde_json::json!({"data":data,"initial_payment":payment})
        } else {
            data.clone()
        };
        let request_hash =
            Sha256::digest(serde_json::to_vec(&hash_data).map_err(|_| CoreError::Invalid)?)
                .to_vec();
        let key_hash = Sha256::digest(key.as_bytes()).to_vec();
        let previous: Option<(Vec<u8>, serde_json::Value)> = sqlx::query_as("SELECT request_hash,response_json FROM facturas_create_receipts WHERE actor_id=? AND key_hash=?").bind(actor).bind(&key_hash).fetch_optional(&mut *tx).await?;
        if let Some((hash, response)) = previous {
            if hash != request_hash {
                return Err(CoreError::Conflict);
            }
            let result: InvoiceDraft =
                serde_json::from_value(response).map_err(|_| CoreError::Invalid)?;
            load(&mut tx, result.id).await?;
            tx.commit().await?;
            return Ok(result);
        }
        self.resolve_client(&mut tx, actor, &mut input, None)
            .await?;
        calculation = calculate(&input)?;
        if let Some(payment) = &payment {
            actions::settle(
                &mut calculation,
                crate::calculation::cents(&payment.amount)?,
            )?;
        }
        calculation.ensure_saveable()?;
        let data = serde_json::to_value(&calculation.data).map_err(|_| CoreError::Invalid)?;
        let id = sqlx::query("INSERT INTO facturas_drafts (client_name,order_number,invoice_date,data_json,calculation_json,created_by,updated_by,created_at,updated_at) VALUES (?,?,?,?,?,?,?,UTC_TIMESTAMP(6),UTC_TIMESTAMP(6))")
            .bind(&calculation.data.client_name).bind(&calculation.data.order_number).bind(calculation.data.invoice_date).bind(data)
            .bind(serde_json::to_value(&calculation).map_err(|_| CoreError::Invalid)?).bind(actor).bind(actor).execute(&mut *tx).await?.last_insert_id();
        let payments = if let Some(payment) = payment {
            vec![actions::insert_payment(&mut tx, actor, id, &payment).await?]
        } else {
            Vec::new()
        };
        let draft = InvoiceDraft {
            id,
            version: 1,
            status: "Borrador".into(),
            currency: "HNL".into(),
            invoice_number: None,
            payments,
            sar_snapshot: None,
            company_snapshot: None,
            source_quote: None,
            source_order: None,
            data: calculation.data.clone(),
            calculation,
        };
        sqlx::query("INSERT INTO facturas_create_receipts (actor_id,key_hash,request_hash,response_json) VALUES (?,?,?,?)")
            .bind(actor).bind(key_hash).bind(request_hash).bind(serde_json::to_value(&draft).map_err(|_| CoreError::Invalid)?).execute(&mut *tx).await?;
        CoreAdmin::record_domain_event(
            &mut tx,
            actor,
            "facturas",
            "invoice.draft.create",
            &id.to_string(),
            serde_json::json!({"version":1}),
        )
        .await?;
        tx.commit().await?;
        Ok(draft)
    }

    pub async fn replace(
        &self,
        actor: u64,
        id: u64,
        expected: u64,
        mut input: InvoiceInput,
    ) -> Result<InvoiceDraft, CoreError> {
        let mut tx = self.begin(actor, "editar").await?;
        let current = load(&mut tx, id).await?;
        if current.version != expected {
            return Err(CoreError::PreconditionFailed);
        }
        if current.status != "Borrador" {
            return Err(CoreError::Conflict);
        }
        self.resolve_client(&mut tx, actor, &mut input, Some(&current.data))
            .await?;
        let payment = input
            .initial_payment
            .take()
            .map(actions::normalize_payment)
            .transpose()?;
        if payment.is_some() && !current.payments.is_empty() {
            return Err(CoreError::Conflict);
        }
        let mut calculation = calculate(&input)?;
        let paid = crate::calculation::cents(&current.calculation.paid_total)?
            + payment
                .as_ref()
                .map(|p| crate::calculation::cents(&p.amount))
                .transpose()?
                .unwrap_or(0);
        actions::settle(&mut calculation, paid)?;
        calculation.ensure_saveable()?;
        let version = expected.checked_add(1).ok_or(CoreError::Invalid)?;
        let mut payments = current.payments;
        if let Some(payment) = payment {
            payments.push(actions::insert_payment(&mut tx, actor, id, &payment).await?);
        }
        sqlx::query("UPDATE facturas_drafts SET version=?,client_name=?,order_number=?,invoice_date=?,data_json=?,calculation_json=?,updated_by=?,updated_at=UTC_TIMESTAMP(6) WHERE id=?")
            .bind(version).bind(&calculation.data.client_name).bind(&calculation.data.order_number).bind(calculation.data.invoice_date)
            .bind(serde_json::to_value(&calculation.data).map_err(|_| CoreError::Invalid)?).bind(serde_json::to_value(&calculation).map_err(|_| CoreError::Invalid)?).bind(actor).bind(id).execute(&mut *tx).await?;
        CoreAdmin::record_domain_event(
            &mut tx,
            actor,
            "facturas",
            "invoice.draft.update",
            &id.to_string(),
            serde_json::json!({"version":version}),
        )
        .await?;
        tx.commit().await?;
        Ok(InvoiceDraft {
            id,
            version,
            status: "Borrador".into(),
            currency: "HNL".into(),
            invoice_number: None,
            payments,
            sar_snapshot: None,
            company_snapshot: None,
            source_quote: current.source_quote,
            source_order: current.source_order,
            data: calculation.data.clone(),
            calculation,
        })
    }

    async fn resolve_client(
        &self,
        tx: &mut Transaction<'_, MySql>,
        actor: u64,
        input: &mut InvoiceInput,
        current: Option<&InvoiceInput>,
    ) -> Result<(), CoreError> {
        if let Some(id) = input.id_client {
            if let Some(previous) = current.filter(|previous| previous.id_client == Some(id)) {
                input.client_name.clone_from(&previous.client_name);
                input.client_rtn.clone_from(&previous.client_rtn);
                input.client_email.clone_from(&previous.client_email);
            } else {
                let client =
                    bastion_clientes::ClientService::new(self.database.clone(), self.admin.clone())
                        .document_snapshot(actor, id, tx)
                        .await?;
                input.client_name = client.cliente_name;
                input.client_rtn = client.rtn;
                input.client_email = client.email;
            }
        } else {
            input.client_rtn.clear();
            input.client_email.clear();
        }
        Ok(())
    }
}

async fn load(tx: &mut Transaction<'_, MySql>, id: u64) -> Result<InvoiceDraft, CoreError> {
    let row = sqlx::query("SELECT id,version,status,data_json,calculation_json,source_quote_json,source_order_json,invoice_number,sar_snapshot_json,company_snapshot_json FROM facturas_drafts WHERE id=? AND deleted_at IS NULL FOR UPDATE").bind(id).fetch_optional(&mut **tx).await?.ok_or(CoreError::NotFound)?;
    let data: serde_json::Value = row.try_get("data_json")?;
    let calculation: serde_json::Value = row.try_get("calculation_json")?;
    let origin: Option<serde_json::Value> = row.try_get("source_quote_json")?;
    let order_origin: Option<serde_json::Value> = row.try_get("source_order_json")?;
    let payments=sqlx::query_as::<_,crate::InvoicePayment>("SELECT id,CAST(amount AS CHAR) AS amount,payment_date AS date,method,reference FROM facturas_payments WHERE invoice_id=? ORDER BY id").bind(id).fetch_all(&mut **tx).await?;
    let sar: Option<serde_json::Value> = row.try_get("sar_snapshot_json")?;
    let company: Option<serde_json::Value> = row.try_get("company_snapshot_json")?;
    Ok(InvoiceDraft {
        id: row.try_get("id")?,
        version: row.try_get("version")?,
        status: row.try_get("status")?,
        currency: "HNL".into(),
        invoice_number: row.try_get("invoice_number")?,
        payments,
        sar_snapshot: sar
            .filter(|v| !v.is_null())
            .map(serde_json::from_value)
            .transpose()
            .map_err(|_| CoreError::Invalid)?,
        company_snapshot: company
            .filter(|v| !v.is_null())
            .map(serde_json::from_value)
            .transpose()
            .map_err(|_| CoreError::Invalid)?,
        source_order: order_origin
            .map(serde_json::from_value)
            .transpose()
            .map_err(|_| CoreError::Invalid)?,
        source_quote: origin
            .map(serde_json::from_value)
            .transpose()
            .map_err(|_| CoreError::Invalid)?,
        data: serde_json::from_value(data).map_err(|_| CoreError::Invalid)?,
        calculation: serde_json::from_value(calculation).map_err(|_| CoreError::Invalid)?,
    })
}
