use super::*;
use bastion_core::{DashboardPeriod, DashboardPoint, DashboardWindow};
use sqlx::Row;

const DATE: &str = "JSON_UNQUOTE(JSON_EXTRACT(data_json,'$.quote_date'))";
const STATUS: &str = "COALESCE(JSON_UNQUOTE(JSON_EXTRACT(data_json,'$.status')),'Borrador')";
const TOTAL: &str = "JSON_UNQUOTE(JSON_EXTRACT(calculation_json,'$.totals.total'))";
fn query(select: &str, window: &DashboardWindow) -> QueryBuilder<MySql> {
    let mut query = QueryBuilder::new(select.to_owned());
    query.push(" FROM cotizaciones_drafts WHERE 1=1");
    if let Some(from) = window.from {
        query
            .push(format!(" AND {DATE}>="))
            .push_bind(from.to_string());
    }
    if let Some(until) = window.until {
        query
            .push(format!(" AND {DATE}<"))
            .push_bind(until.to_string());
    }
    query
}
#[derive(serde::Serialize, sqlx::FromRow)]
struct Recent {
    id: u64,
    quote_number: Option<String>,
    client_name: String,
    quote_date: String,
    status: String,
    total: Option<String>,
}
#[derive(serde::Serialize, sqlx::FromRow)]
struct StateCount {
    label: String,
    count: i64,
}
impl QuoteService {
    pub async fn dashboard(
        &self,
        actor: u64,
        period: DashboardPeriod,
    ) -> Result<serde_json::Value, CoreError> {
        let now = chrono::Utc::now();
        let window = period.window(now)?;
        let mut tx = self.begin(actor, "ver").await?;
        let amount = format!("CAST(COALESCE(SUM(CAST({TOTAL} AS DECIMAL(50,2))),0) AS CHAR)");
        let metrics = query(
            &format!("SELECT COUNT(*) AS total,COUNT({TOTAL}) AS valued,{amount} AS amount"),
            &window,
        )
        .build()
        .fetch_one(&mut *tx)
        .await?;
        let total: i64 = metrics.try_get("total")?;
        let valued: i64 = metrics.try_get("valued")?;
        let mut states = query(
            &format!("SELECT {STATUS} AS label,COUNT(*) AS count"),
            &window,
        );
        states.push(" GROUP BY label ORDER BY label LIMIT 20");
        let states: Vec<StateCount> = states.build_query_as().fetch_all(&mut *tx).await?;
        let mut chart = query(
            &format!(
                "SELECT DATE_FORMAT({DATE},'{}') AS bucket,COUNT(*) AS count,{amount} AS amount",
                period.sql_format()
            ),
            &window,
        );
        chart.push(" GROUP BY bucket ORDER BY bucket LIMIT 121");
        let mut points: Vec<DashboardPoint> = chart.build_query_as().fetch_all(&mut *tx).await?;
        let chart_truncated = points.len() > 120;
        points.truncate(120);
        let points = window.fill_series(points, true)?;
        let mut recent = query(
            &format!(
                "SELECT id,quote_number,client_name,{DATE} AS quote_date,{STATUS} AS status,{TOTAL} AS total"
            ),
            &window,
        );
        recent.push(format!(" ORDER BY {DATE} DESC,id DESC LIMIT 8"));
        let recent: Vec<Recent> = recent.build_query_as().fetch_all(&mut *tx).await?;
        tx.commit().await?;
        Ok(
            serde_json::json!({"window":window,"as_of":now.to_rfc3339(),"metrics":{"total":total,"amount":metrics.try_get::<String,_>("amount")?,"without_total":total-valued},"series":points,"chart_truncated":chart_truncated,"distribution":states,"recent":recent}),
        )
    }
}
