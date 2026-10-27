use crate::{
    Expense, ExpenseCurrencyTotals, ExpenseFilter, ExpenseInput, ExpensePage, ExpenseReport,
    ExpenseReportFilter, ExpenseReportRow, ExpenseSummary, calculate,
};
use bastion_core::{
    CoreAdmin, CoreError, DashboardPeriod, DashboardPoint, DashboardWindow, Database,
};
use sha2::{Digest, Sha256};
use sqlx::{MySql, QueryBuilder, Row, Transaction};

#[derive(serde::Serialize, sqlx::FromRow)]
struct ExpenseDashboardState {
    label: String,
    count: i64,
}
#[derive(serde::Serialize, sqlx::FromRow)]
struct ExpenseDashboardCurrency {
    currency: String,
    amount: String,
}
#[derive(serde::Serialize, sqlx::FromRow)]
struct ExpenseDashboardRow {
    id: u64,
    invoice_number: String,
    expense_date: String,
    provider: String,
    currency: String,
    total: String,
}
fn expense_dashboard_query(
    select: &str,
    window: &DashboardWindow,
) -> QueryBuilder<MySql> {
    let mut query = QueryBuilder::new(select.to_owned());
    query.push(" FROM gastos_records WHERE deleted_at IS NULL");
    if let Some(from) = window.from {
        query.push(" AND expense_date>=").push_bind(from);
    }
    if let Some(until) = window.until {
        query.push(" AND expense_date<").push_bind(until);
    }
    query
}

#[derive(Clone)]
pub struct ExpenseService {
    database: Database,
    admin: CoreAdmin,
}

impl ExpenseService {
    pub fn new(database: Database, admin: CoreAdmin) -> Self {
        Self { database, admin }
    }

    pub async fn migrate(database: &Database) -> Result<(), CoreError> {
        let mut migrator = sqlx::migrate!("./migrations");
        migrator.dangerous_set_table_name("_gastos_migrations");
        migrator
            .run(database.pool())
            .await
            .map_err(CoreError::Migration)
    }

    pub async fn synchronize_release(&self, allowed: bool) -> Result<(), CoreError> {
        let exists: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM information_schema.tables WHERE table_schema=DATABASE() AND table_name='_gastos_migrations'")
            .fetch_one(self.database.pool()).await?;
        let ready = if exists > 0 {
            let applied = sqlx::query_as::<_, (i64, bool, Vec<u8>)>(
                "SELECT version,success,checksum FROM _gastos_migrations",
            )
            .fetch_all(self.database.pool())
            .await?;
            let expected = sqlx::migrate!("./migrations");
            applied.len() == expected.iter().count()
                && expected.iter().all(|migration| {
                    applied.iter().any(|(version, success, checksum)| {
                        *version == migration.version
                            && *success
                            && checksum.as_slice() == migration.checksum.as_ref()
                    })
                })
        } else {
            false
        };
        sqlx::query("INSERT INTO module_registry (module_id,display_name,included_in_release,environment_allowed,enabled) VALUES ('gastos','Gastos',1,?,0) ON DUPLICATE KEY UPDATE included_in_release=1,environment_allowed=VALUES(environment_allowed)")
            .bind(allowed && ready)
            .execute(self.database.pool())
            .await?;
        Ok(())
    }

    async fn begin(
        &self,
        actor: u64,
        capability: &str,
    ) -> Result<Transaction<'_, MySql>, CoreError> {
        self.admin
            .authorize(actor, "gastos", "registros", capability)
            .await?;
        let mut tx = self.database.pool().begin().await?;
        let enabled: Option<bool> = sqlx::query_scalar("SELECT (included_in_release AND environment_allowed AND enabled) FROM module_registry WHERE module_id='gastos' FOR UPDATE")
            .fetch_optional(&mut *tx).await?;
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
        sqlx::query("INSERT INTO gastos_actor_limits (actor_id,window_start,requests) VALUES (?,UTC_TIMESTAMP(6),1) ON DUPLICATE KEY UPDATE requests=IF(window_start<=UTC_TIMESTAMP(6)-INTERVAL 1 MINUTE,1,requests+1),window_start=IF(window_start<=UTC_TIMESTAMP(6)-INTERVAL 1 MINUTE,UTC_TIMESTAMP(6),window_start)")
            .bind(actor).execute(&mut *tx).await?;
        let count: u32 =
            sqlx::query_scalar("SELECT requests FROM gastos_actor_limits WHERE actor_id=?")
                .bind(actor)
                .fetch_one(&mut *tx)
                .await?;
        tx.commit().await?;
        if count > 120 {
            return Err(CoreError::Locked);
        }
        Ok(())
    }

    pub async fn list(&self, actor: u64, filter: &ExpenseFilter) -> Result<ExpensePage, CoreError> {
        let (page, page_size) = filter.bounds()?;
        let mut tx = self.begin(actor, "ver").await?;
        let mut summary_query = filtered_query(
            "SELECT CAST(COUNT(*) AS UNSIGNED) AS count,CAST(COALESCE(SUM(taxable_amount),0.00) AS CHAR) AS taxable_amount,CAST(COALESCE(SUM(exempt_amount),0.00) AS CHAR) AS exempt_amount,CAST(COALESCE(SUM(isv_15),0.00) AS CHAR) AS isv_15,CAST(COALESCE(SUM(total),0.00) AS CHAR) AS total FROM gastos_records WHERE deleted_at IS NULL",
            filter,
        );
        let summary = summary_query
            .build_query_as::<ExpenseSummary>()
            .fetch_one(&mut *tx)
            .await?;
        let mut query = filtered_query(
            "SELECT id,version,CAST(expense_date AS CHAR) AS expense_date,invoice_number,provider,currency,CAST(taxable_amount AS CHAR) AS taxable_amount,CAST(exempt_amount AS CHAR) AS exempt_amount,CAST(isv_15 AS CHAR) AS isv_15,CAST(total AS CHAR) AS total FROM gastos_records WHERE deleted_at IS NULL",
            filter,
        );
        query
            .push(" ORDER BY expense_date DESC,id DESC LIMIT ")
            .push_bind(page_size + 1)
            .push(" OFFSET ")
            .push_bind((page - 1) * page_size);
        let mut rows = query.build().fetch_all(&mut *tx).await?;
        let has_more = rows.len() > page_size as usize;
        rows.truncate(page_size as usize);
        let items = rows
            .iter()
            .map(expense_from_row)
            .collect::<Result<Vec<_>, _>>()?;
        tx.commit().await?;
        Ok(ExpensePage {
            items,
            page,
            page_size,
            has_more,
            summary,
        })
    }

    pub async fn preview(
        &self,
        actor: u64,
        input: &ExpenseInput,
    ) -> Result<crate::ExpenseCalculation, CoreError> {
        self.check_access(actor, "editar").await?;
        calculate(input)
    }

    pub async fn report(
        &self,
        actor: u64,
        filter: &ExpenseReportFilter,
    ) -> Result<ExpenseReport, CoreError> {
        let (page, page_size) = filter.bounds()?;
        let mut tx = self.begin(actor, "ver").await?;
        let mut totals_query = QueryBuilder::<MySql>::new(
            "SELECT currency,CAST(COUNT(*) AS UNSIGNED) AS records,CAST(COALESCE(SUM(taxable_amount),0) AS CHAR) AS taxable_amount,CAST(COALESCE(SUM(exempt_amount),0) AS CHAR) AS exempt_amount,CAST(COALESCE(SUM(isv_15),0) AS CHAR) AS isv_15,CAST(COALESCE(SUM(total),0) AS CHAR) AS total FROM gastos_records WHERE deleted_at IS NULL AND expense_date>=",
        );
        totals_query
            .push_bind(filter.date_from)
            .push(" AND expense_date<=")
            .push_bind(filter.date_to);
        if let Some(term) = filter.search() {
            totals_query
                .push(" AND (LOCATE(")
                .push_bind(term)
                .push(",invoice_number)>0 OR LOCATE(")
                .push_bind(term)
                .push(",provider)>0)");
        }
        let totals: Vec<ExpenseCurrencyTotals> = totals_query
            .push(" GROUP BY currency ORDER BY currency")
            .build_query_as()
            .fetch_all(&mut *tx)
            .await?;
        let mut items_query = QueryBuilder::<MySql>::new(
            "SELECT id,invoice_number,CAST(expense_date AS CHAR) AS expense_date,provider,currency,CAST(taxable_amount AS CHAR) AS taxable_amount,CAST(exempt_amount AS CHAR) AS exempt_amount,CAST(isv_15 AS CHAR) AS isv_15,CAST(total AS CHAR) AS total FROM gastos_records WHERE deleted_at IS NULL AND expense_date>=",
        );
        items_query
            .push_bind(filter.date_from)
            .push(" AND expense_date<=")
            .push_bind(filter.date_to);
        if let Some(term) = filter.search() {
            items_query
                .push(" AND (LOCATE(")
                .push_bind(term)
                .push(",invoice_number)>0 OR LOCATE(")
                .push_bind(term)
                .push(",provider)>0)");
        }
        let items: Vec<ExpenseReportRow> = items_query
            .push(" ORDER BY expense_date DESC,id DESC LIMIT ")
            .push_bind(page_size + 1)
            .push(" OFFSET ")
            .push_bind((page - 1) * page_size)
            .build_query_as()
            .fetch_all(&mut *tx)
            .await?;
        let mut items = items;
        let has_more = items.len() > page_size as usize;
        items.truncate(page_size as usize);
        tx.commit().await?;
        Ok(ExpenseReport {
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
        let metrics = expense_dashboard_query("SELECT COUNT(*) AS total", &window)
            .build()
            .fetch_one(&mut *tx)
            .await?;
        let total: i64 = metrics.try_get("total")?;
        let mut currencies = expense_dashboard_query(
            "SELECT currency,CAST(COALESCE(SUM(total),0) AS CHAR) AS amount",
            &window,
        );
        currencies.push(" GROUP BY currency ORDER BY currency");
        let currencies = currencies
            .build_query_as::<ExpenseDashboardCurrency>()
            .fetch_all(&mut *tx)
            .await?;
        let mut states =
            expense_dashboard_query("SELECT provider AS label,COUNT(*) AS count", &window);
        states.push(" GROUP BY provider ORDER BY count DESC,provider LIMIT 8");
        let states = states
            .build_query_as::<ExpenseDashboardState>()
            .fetch_all(&mut *tx)
            .await?;
        let mut chart = QueryBuilder::<MySql>::new("SELECT DATE_FORMAT(expense_date,");
        chart
            .push_bind(period.sql_format())
            .push(") AS bucket,COUNT(*) AS count FROM gastos_records WHERE deleted_at IS NULL");
        if let Some(from) = window.from {
            chart.push(" AND expense_date>=").push_bind(from);
        }
        if let Some(until) = window.until {
            chart.push(" AND expense_date<").push_bind(until);
        }
        chart.push(" GROUP BY bucket ORDER BY bucket LIMIT 121");
        let mut points = chart
            .build_query_as::<DashboardPoint>()
            .fetch_all(&mut *tx)
            .await?;
        let chart_truncated = points.len() > 120;
        points.truncate(120);
        let points = window.fill_series(points, false)?;
        let recent = expense_dashboard_query(
            "SELECT id,invoice_number,CAST(expense_date AS CHAR) AS expense_date,provider,currency,CAST(total AS CHAR) AS total",
            &window,
        )
            .push(" ORDER BY expense_date DESC,id DESC LIMIT 8")
            .build_query_as::<ExpenseDashboardRow>()
            .fetch_all(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(
            serde_json::json!({
                "window": window,
                "as_of": now.to_rfc3339(),
                "metrics": {"total": total, "currencies": currencies},
                "series": points,
                "chart_truncated": chart_truncated,
                "distribution": states,
                "recent": recent
            }),
        )
    }

    pub async fn detail(&self, actor: u64, id: u64) -> Result<Expense, CoreError> {
        let mut tx = self.begin(actor, "ver").await?;
        let expense = load(&mut tx, id).await?;
        tx.commit().await?;
        Ok(expense)
    }

    pub async fn create(
        &self,
        actor: u64,
        key: &str,
        input: ExpenseInput,
    ) -> Result<Expense, CoreError> {
        let calculation = calculate(&input)?;
        if !(16..=128).contains(&key.len())
            || !key
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
        {
            return Err(CoreError::Invalid);
        }
        let request_hash =
            Sha256::digest(serde_json::to_vec(&calculation).map_err(|_| CoreError::Invalid)?)
                .to_vec();
        let key_hash = Sha256::digest(key.as_bytes()).to_vec();
        let mut tx = self.begin(actor, "editar").await?;
        let previous: Option<(Vec<u8>, serde_json::Value)> = sqlx::query_as("SELECT request_hash,response_json FROM gastos_create_receipts WHERE actor_id=? AND key_hash=?")
            .bind(actor).bind(&key_hash).fetch_optional(&mut *tx).await?;
        if let Some((old_hash, response)) = previous {
            if old_hash != request_hash {
                return Err(CoreError::Conflict);
            }
            let expense = serde_json::from_value(response).map_err(|_| CoreError::Invalid)?;
            tx.commit().await?;
            return Ok(expense);
        }
        let id = sqlx::query("INSERT INTO gastos_records (expense_date,invoice_number,provider,currency,taxable_amount,exempt_amount,isv_15,total,created_by,updated_by,created_at,updated_at) VALUES (?,?,?,?,CAST(? AS DECIMAL(14,2)),CAST(? AS DECIMAL(14,2)),CAST(? AS DECIMAL(14,2)),CAST(? AS DECIMAL(14,2)),?,?,UTC_TIMESTAMP(6),UTC_TIMESTAMP(6))")
            .bind(calculation.data.expense_date).bind(&calculation.data.invoice_number).bind(&calculation.data.provider).bind(&calculation.data.currency)
            .bind(&calculation.data.taxable_amount).bind(&calculation.data.exempt_amount).bind(&calculation.isv_15).bind(&calculation.total)
            .bind(actor).bind(actor).execute(&mut *tx).await?.last_insert_id();
        let expense = Expense {
            id,
            version: 1,
            data: calculation.data,
            isv_15: calculation.isv_15,
            total: calculation.total,
        };
        sqlx::query("INSERT INTO gastos_create_receipts (actor_id,key_hash,request_hash,response_json) VALUES (?,?,?,?)")
            .bind(actor).bind(&key_hash).bind(&request_hash).bind(serde_json::to_value(&expense).map_err(|_| CoreError::Invalid)?)
            .execute(&mut *tx).await?;
        CoreAdmin::record_domain_event(
            &mut tx,
            actor,
            "gastos",
            "expense.create",
            &id.to_string(),
            serde_json::json!({"version": expense.version}),
        )
        .await?;
        tx.commit().await?;
        Ok(expense)
    }

    pub async fn replace(
        &self,
        actor: u64,
        id: u64,
        expected: u64,
        input: ExpenseInput,
    ) -> Result<Expense, CoreError> {
        let calculation = calculate(&input)?;
        let mut tx = self.begin(actor, "editar").await?;
        let current = load(&mut tx, id).await?;
        if current.version != expected {
            return Err(CoreError::PreconditionFailed);
        }
        let version = expected.checked_add(1).ok_or(CoreError::Invalid)?;
        sqlx::query("UPDATE gastos_records SET expense_date=?,invoice_number=?,provider=?,currency=?,taxable_amount=CAST(? AS DECIMAL(14,2)),exempt_amount=CAST(? AS DECIMAL(14,2)),isv_15=CAST(? AS DECIMAL(14,2)),total=CAST(? AS DECIMAL(14,2)),updated_by=?,updated_at=UTC_TIMESTAMP(6),version=? WHERE id=?")
            .bind(calculation.data.expense_date).bind(&calculation.data.invoice_number).bind(&calculation.data.provider).bind(&calculation.data.currency)
            .bind(&calculation.data.taxable_amount).bind(&calculation.data.exempt_amount).bind(&calculation.isv_15).bind(&calculation.total)
            .bind(actor).bind(version).bind(id).execute(&mut *tx).await?;
        CoreAdmin::record_domain_event(
            &mut tx,
            actor,
            "gastos",
            "expense.update",
            &id.to_string(),
            serde_json::json!({"version": version}),
        )
        .await?;
        tx.commit().await?;
        Ok(Expense {
            id,
            version,
            data: calculation.data,
            isv_15: calculation.isv_15,
            total: calculation.total,
        })
    }

    pub async fn delete(&self, actor: u64, id: u64) -> Result<(), CoreError> {
        let mut tx = self.begin(actor, "eliminar").await?;
        let result = sqlx::query("UPDATE gastos_records SET deleted_at=UTC_TIMESTAMP(6),updated_at=UTC_TIMESTAMP(6),updated_by=?,version=version+1 WHERE id=? AND deleted_at IS NULL")
            .bind(actor).bind(id)
            .execute(&mut *tx)
            .await?;
        if result.rows_affected() == 0 {
            return Err(CoreError::NotFound);
        }
        CoreAdmin::record_domain_event(
            &mut tx,
            actor,
            "gastos",
            "expense.delete",
            &id.to_string(),
            serde_json::json!({}),
        )
        .await?;
        tx.commit().await?;
        Ok(())
    }
}

fn expense_from_row(row: &sqlx::mysql::MySqlRow) -> Result<Expense, sqlx::Error> {
    let date: String = row.try_get("expense_date")?;
    Ok(Expense {
        id: row.try_get("id")?,
        version: row.try_get("version")?,
        data: ExpenseInput {
            expense_date: chrono::NaiveDate::parse_from_str(&date, "%Y-%m-%d").map_err(|_| {
                sqlx::Error::ColumnDecode {
                    index: "expense_date".into(),
                    source: Box::new(std::io::Error::other("invalid date in gastos_records")),
                }
            })?,
            invoice_number: row.try_get("invoice_number")?,
            provider: row.try_get("provider")?,
            currency: row.try_get("currency")?,
            taxable_amount: row.try_get("taxable_amount")?,
            exempt_amount: row.try_get("exempt_amount")?,
        },
        isv_15: row.try_get("isv_15")?,
        total: row.try_get("total")?,
    })
}

async fn load(tx: &mut Transaction<'_, MySql>, id: u64) -> Result<Expense, CoreError> {
    let row = sqlx::query("SELECT id,version,CAST(expense_date AS CHAR) AS expense_date,invoice_number,provider,currency,CAST(taxable_amount AS CHAR) AS taxable_amount,CAST(exempt_amount AS CHAR) AS exempt_amount,CAST(isv_15 AS CHAR) AS isv_15,CAST(total AS CHAR) AS total FROM gastos_records WHERE id=? AND deleted_at IS NULL FOR UPDATE")
        .bind(id).fetch_optional(&mut **tx).await?.ok_or(CoreError::NotFound)?;
    Ok(expense_from_row(&row)?)
}

fn filtered_query(prefix: &str, filter: &ExpenseFilter) -> QueryBuilder<MySql> {
    let mut query = QueryBuilder::<MySql>::new(prefix.to_owned());
    if let Some(term) = filter.search() {
        query
            .push(" AND (LOCATE(")
            .push_bind(term)
            .push(",invoice_number)>0 OR LOCATE(")
            .push_bind(term)
            .push(",provider)>0)");
    }
    if let Some(from) = filter.date_from {
        query.push(" AND expense_date>=").push_bind(from);
    }
    if let Some(to) = filter.date_to {
        query.push(" AND expense_date<=").push_bind(to);
    }
    query
}
