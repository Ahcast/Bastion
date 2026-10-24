use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::CoreError;

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SarSettings {
    pub limit_range: String,
    pub limit_date: Option<NaiveDate>,
    pub current_invoice: String,
    pub sar_text: String,
}

#[derive(Debug, Serialize)]
pub struct SarStatus {
    pub days_remaining: Option<i64>,
    pub invoices_remaining: Option<u32>,
    pub range_expired: bool,
    pub date_expired: bool,
}

#[derive(Debug, Serialize)]
pub struct SarSettingsRecord {
    pub settings: SarSettings,
    pub version: String,
    pub status: SarStatus,
}

impl SarSettings {
    pub fn normalize(mut self) -> Result<Self, CoreError> {
        if self.limit_range.chars().count() > 255
            || self.current_invoice.chars().count() > 80
            || self.sar_text.chars().count() > 3000
            || self
                .limit_date
                .is_some_and(|date| !(1000..=9999).contains(&chrono::Datelike::year(&date)))
        {
            return Err(CoreError::Invalid);
        }
        self.limit_range = self.limit_range.trim().into();
        self.current_invoice = self.current_invoice.trim().into();
        self.sar_text = self.sar_text.trim().into();
        Ok(self)
    }

    // Parity with CompanySettingsController::sarStatus/lastEightNumber.
    // These are informational Legacy calculations, not fiscal validation.
    pub fn status(&self, today: NaiveDate) -> SarStatus {
        let limit = last_eight(&self.limit_range, true);
        let current = last_eight(&self.current_invoice, false);
        let invoices_remaining = limit.zip(current).map(|(end, now)| end.saturating_sub(now));
        let days_remaining = self.limit_date.map(|date| (date - today).num_days());
        SarStatus {
            days_remaining,
            invoices_remaining,
            range_expired: invoices_remaining == Some(0),
            date_expired: days_remaining.is_some_and(|days| days < 0),
        }
    }
}

fn last_eight(value: &str, last_group: bool) -> Option<u32> {
    let groups: Vec<&str> = value
        .split(|character: char| !character.is_ascii_digit())
        .filter(|part| !part.is_empty())
        .collect();
    let digits = if last_group {
        groups.last()?.to_string()
    } else {
        groups.join("")
    };
    digits[digits.len().saturating_sub(8)..].parse().ok()
}

pub(crate) fn sar_version(value: &Option<serde_json::Value>) -> Result<String, CoreError> {
    let bytes = serde_json::to_vec(value).map_err(|_| CoreError::Invalid)?;
    Ok(hex::encode(Sha256::digest(bytes)))
}

pub(crate) fn sar_record(value: Option<serde_json::Value>) -> Result<SarSettingsRecord, CoreError> {
    let version = sar_version(&value)?;
    let settings: SarSettings = value
        .map(serde_json::from_value)
        .transpose()
        .map_err(|_| CoreError::Invalid)?
        .unwrap_or_default();
    let today = chrono::Utc::now()
        .with_timezone(&chrono::FixedOffset::west_opt(6 * 3600).expect("Honduras offset"))
        .date_naive();
    let status = settings.status(today);
    Ok(SarSettingsRecord {
        settings,
        version,
        status,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_preserves_legacy_suffix_and_date_boundaries() {
        let today = NaiveDate::from_ymd_opt(2026, 10, 23).unwrap();
        let mut settings = SarSettings {
            limit_range: "000-001-01-00000001 al 000-001-01-00000100".into(),
            current_invoice: "000-001-01-00000090".into(),
            limit_date: Some(today),
            sar_text: String::new(),
        };
        let status = settings.status(today);
        assert_eq!(status.invoices_remaining, Some(10));
        assert_eq!(status.days_remaining, Some(0));
        assert!(!status.date_expired);
        settings.current_invoice = "101".into();
        assert!(settings.status(today.succ_opt().unwrap()).date_expired);
        assert!(settings.status(today).range_expired);
        let missing = SarSettings::default().status(today);
        assert_eq!(missing.invoices_remaining, None);
        assert_eq!(missing.days_remaining, None);
        assert!(!missing.range_expired);
    }

    #[test]
    fn schema_limits_normalization_and_versions_are_enforced() {
        let input = serde_json::json!({"limit_range":"  100  ","limit_date":null,"current_invoice":"90","sar_text":" Texto "});
        let settings: SarSettings = serde_json::from_value(input.clone()).unwrap();
        assert_eq!(settings.normalize().unwrap().sar_text, "Texto");
        for (key, limit) in [
            ("limit_range", 255),
            ("current_invoice", 80),
            ("sar_text", 3000),
        ] {
            let mut oversized = input.clone();
            oversized[key] = serde_json::json!("x".repeat(limit + 1));
            assert!(
                serde_json::from_value::<SarSettings>(oversized)
                    .unwrap()
                    .normalize()
                    .is_err()
            );
        }
        let mut unknown = input.clone();
        unknown["next_invoice"] = serde_json::json!("123");
        assert!(serde_json::from_value::<SarSettings>(unknown).is_err());
        let mut invalid_date = input.clone();
        invalid_date["limit_date"] = serde_json::json!("2026-02-30");
        assert!(serde_json::from_value::<SarSettings>(invalid_date).is_err());
        assert_ne!(
            sar_version(&None).unwrap(),
            sar_version(&Some(input)).unwrap()
        );
    }
}

