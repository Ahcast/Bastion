//! Calendar and bounded chart contract; business queries remain in their modules.
use crate::CoreError;
use chrono::{DateTime, Datelike, Days, FixedOffset, NaiveDate, NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Default, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DashboardPeriod {
    #[default]
    Month,
    Year,
    All,
}
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DashboardQuery {
    #[serde(default)]
    pub period: DashboardPeriod,
}
#[derive(Serialize)]
pub struct DashboardWindow {
    pub period: DashboardPeriod,
    pub from: Option<NaiveDate>,
    pub until: Option<NaiveDate>,
    pub today: NaiveDate,
    pub timezone: &'static str,
}
#[derive(Serialize, sqlx::FromRow)]
pub struct DashboardPoint {
    pub bucket: String,
    pub count: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount: Option<String>,
}
impl DashboardPeriod {
    pub fn window(self, now: DateTime<Utc>) -> Result<DashboardWindow, CoreError> {
        let today = now
            .with_timezone(&FixedOffset::west_opt(6 * 3600).ok_or(CoreError::Invalid)?)
            .date_naive();
        let date = |year, month| NaiveDate::from_ymd_opt(year, month, 1).ok_or(CoreError::Invalid);
        let (from, until) = match self {
            Self::Month => (
                Some(date(today.year(), today.month())?),
                Some(if today.month() == 12 {
                    date(today.year() + 1, 1)?
                } else {
                    date(today.year(), today.month() + 1)?
                }),
            ),
            Self::Year => (
                Some(date(today.year(), 1)?),
                Some(date(today.year() + 1, 1)?),
            ),
            Self::All => (None, None),
        };
        Ok(DashboardWindow {
            period: self,
            from,
            until,
            today,
            timezone: "America/Tegucigalpa",
        })
    }
    pub fn sql_format(self) -> &'static str {
        match self {
            Self::Month => "%Y-%m-%d",
            Self::Year | Self::All => "%Y-%m",
        }
    }
}
impl DashboardWindow {
    pub fn midnight_utc(date: NaiveDate) -> Result<NaiveDateTime, CoreError> {
        date.and_hms_opt(6, 0, 0).ok_or(CoreError::Invalid)
    }
    pub fn fill_series(
        &self,
        points: Vec<DashboardPoint>,
        money: bool,
    ) -> Result<Vec<DashboardPoint>, CoreError> {
        let Some(mut date) = self.from else {
            return Ok(points);
        };
        let end = self.until.ok_or(CoreError::Invalid)?;
        let mut points: BTreeMap<_, _> = points
            .into_iter()
            .map(|point| (point.bucket.clone(), point))
            .collect();
        let mut result = Vec::new();
        while date < end {
            let bucket = date.format(self.period.sql_format()).to_string();
            result.push(points.remove(&bucket).unwrap_or(DashboardPoint {
                bucket,
                count: 0,
                amount: money.then(|| "0.00".into()),
            }));
            date = match self.period {
                DashboardPeriod::Month => date
                    .checked_add_days(Days::new(1))
                    .ok_or(CoreError::Invalid)?,
                _ => if date.month() == 12 {
                    NaiveDate::from_ymd_opt(date.year() + 1, 1, 1)
                } else {
                    NaiveDate::from_ymd_opt(date.year(), date.month() + 1, 1)
                }
                .ok_or(CoreError::Invalid)?,
            };
        }
        Ok(result)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honduras_month_boundary_and_leap_year_are_explicit() {
        let now = DateTime::parse_from_rfc3339("2026-03-01T03:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let window = DashboardPeriod::Month.window(now).unwrap();
        assert_eq!(window.from.unwrap().to_string(), "2026-02-01");
        assert_eq!(window.until.unwrap().to_string(), "2026-03-01");
        assert_eq!(window.fill_series(vec![], true).unwrap().len(), 28);
        let leap = DashboardPeriod::Month
            .window(
                DateTime::parse_from_rfc3339("2024-02-15T12:00:00Z")
                    .unwrap()
                    .with_timezone(&Utc),
            )
            .unwrap();
        assert_eq!(leap.fill_series(vec![], false).unwrap().len(), 29);
        assert!(serde_json::from_str::<DashboardQuery>(r#"{"period":"week"}"#).is_err());
        assert!(serde_json::from_str::<DashboardQuery>(r#"{"period":"all","actor":1}"#).is_err());
    }
}
