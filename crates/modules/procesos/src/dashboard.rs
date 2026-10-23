use super::*;
use bastion_core::{DashboardPeriod, DashboardPoint, DashboardWindow};

fn query(
    select: &str,
    window: &DashboardWindow,
    now: NaiveDateTime,
) -> Result<QueryBuilder<MySql>, CoreError> {
    filter(QueryBuilder::new(select.to_owned()), window, now)
}
fn filter(
    mut query: QueryBuilder<MySql>,
    window: &DashboardWindow,
    now: NaiveDateTime,
) -> Result<QueryBuilder<MySql>, CoreError> {
    query
        .push(PROCESS_FROM)
        .push(" AND r.fecha_recepcion>")
        .push_bind(now);
    if let Some(from) = window.from {
        query
            .push(" AND r.fecha_recepcion>=")
            .push_bind(DashboardWindow::midnight_utc(from)?);
    }
    if let Some(until) = window.until {
        query
            .push(" AND r.fecha_recepcion<")
            .push_bind(DashboardWindow::midnight_utc(until)?);
    }
    Ok(query)
}
#[derive(Serialize, FromRow)]
struct Recent {
    proceso_id: String,
    expediente: String,
    entidad: String,
    resumen_corto: String,
    #[serde(serialize_with = "utc_date")]
    fecha_recepcion: NaiveDateTime,
    marked: bool,
}
impl ProcessService {
    pub async fn dashboard(
        &self,
        actor: u64,
        period: DashboardPeriod,
    ) -> Result<serde_json::Value, CoreError> {
        let now = Utc::now();
        let window = period.window(now)?;
        let tomorrow = DashboardWindow::midnight_utc(
            window
                .today
                .checked_add_days(Days::new(1))
                .ok_or(CoreError::Invalid)?,
        )?;
        let day_after = DashboardWindow::midnight_utc(
            window
                .today
                .checked_add_days(Days::new(2))
                .ok_or(CoreError::Invalid)?,
        )?;
        let mut tx = self.begin(actor, "consulta", "ver").await?;
        let mut metrics = QueryBuilder::new(
            "SELECT COUNT(*) AS active,COUNT(DISTINCT r.entidad) AS entities,CAST(COALESCE(SUM(COALESCE(m.marked,0)=1),0) AS SIGNED) AS marked,CAST(COALESCE(SUM(r.fecha_recepcion<",
        );
        metrics
            .push_bind(tomorrow)
            .push("),0) AS SIGNED) AS today,CAST(COALESCE(SUM(r.fecha_recepcion>=")
            .push_bind(tomorrow)
            .push(" AND r.fecha_recepcion<")
            .push_bind(day_after)
            .push("),0) AS SIGNED) AS tomorrow");
        let mut metrics = filter(metrics, &window, now.naive_utc())?;
        let metrics = metrics.build().fetch_one(&mut *tx).await?;
        let active: i64 = metrics.try_get("active")?;
        let today: i64 = metrics.try_get("today")?;
        let next_day: i64 = metrics.try_get("tomorrow")?;
        let mut chart = query(
            &format!(
                "SELECT DATE_FORMAT(DATE_SUB(r.fecha_recepcion,INTERVAL 6 HOUR),'{}') AS bucket,COUNT(*) AS count,NULL AS amount",
                period.sql_format()
            ),
            &window,
            now.naive_utc(),
        )?;
        chart.push(" GROUP BY bucket ORDER BY bucket LIMIT 121");
        let mut points: Vec<DashboardPoint> = chart.build_query_as().fetch_all(&mut *tx).await?;
        let chart_truncated = points.len() > 120;
        points.truncate(120);
        let points = window.fill_series(points, false)?;
        let mut recent = query(
            "SELECT r.proceso_id,r.expediente,r.entidad,r.resumen_corto,r.fecha_recepcion,COALESCE(m.marked,0) AS marked",
            &window,
            now.naive_utc(),
        )?;
        recent.push(" ORDER BY r.fecha_recepcion ASC,r.proceso_id ASC LIMIT 8");
        let recent: Vec<Recent> = recent.build_query_as().fetch_all(&mut *tx).await?;
        tx.commit().await?;
        Ok(
            serde_json::json!({"window":window,"as_of":now.to_rfc3339(),"metrics":{"active":active,"marked":metrics.try_get::<i64,_>("marked")?,"entities":metrics.try_get::<i64,_>("entities")?,"today":today},"series":points,"chart_truncated":chart_truncated,"distribution":[{"label":"Hoy","count":today},{"label":"Mañana","count":next_day},{"label":"Después","count":active-today-next_day}],"recent":recent}),
        )
    }
}
