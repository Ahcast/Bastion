#![forbid(unsafe_code)]
mod service;
mod sources;
use bastion_core::CoreError;
use chrono::{Datelike, NaiveDate};
use serde::{Deserialize, Serialize};
pub use service::AccountingService;

pub const MAX_CENTS: i64 = 99_999_999_999_999;
pub fn cents(value: &str) -> Result<i64, CoreError> {
    let (whole, fraction) = value.split_once('.').unwrap_or((value, ""));
    if whole.is_empty()
        || whole.len() > 12
        || fraction.len() > 2
        || !whole.bytes().all(|c| c.is_ascii_digit())
        || !fraction.bytes().all(|c| c.is_ascii_digit())
        || (value.contains('.') && fraction.is_empty())
    {
        return Err(CoreError::Invalid);
    }
    let units: i64 = whole.parse().map_err(|_| CoreError::Invalid)?;
    let fractional: i64 = if fraction.is_empty() {
        0
    } else {
        fraction.parse().map_err(|_| CoreError::Invalid)?
    };
    let result = units * 100 + fractional * if fraction.len() == 1 { 10 } else { 1 };
    if result > MAX_CENTS {
        return Err(CoreError::Invalid);
    }
    Ok(result)
}
pub fn money(value: i64) -> String {
    let n = value.unsigned_abs();
    format!(
        "{}{}.{:02}",
        if value < 0 { "-" } else { "" },
        n / 100,
        n % 100
    )
}
pub fn valid_month(month: &str) -> Result<(), CoreError> {
    if month.len() != 7 {
        return Err(CoreError::Invalid);
    }
    let date = NaiveDate::parse_from_str(&format!("{month}-01"), "%Y-%m-%d")
        .map_err(|_| CoreError::Invalid)?;
    if !(2000..=2100).contains(&date.year()) || date.format("%Y-%m").to_string() != month {
        return Err(CoreError::Invalid);
    }
    Ok(())
}
pub(crate) fn valid_text(value: &str, max: usize, required: bool) -> Result<(), CoreError> {
    if (required && value.trim().is_empty())
        || value.chars().count() > max
        || value.chars().any(|c| c.is_control())
    {
        return Err(CoreError::Invalid);
    }
    Ok(())
}
#[derive(Clone, Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct Account {
    pub code: String,
    pub name: String,
    pub class: String,
    pub parent_code: Option<String>,
    pub posting: bool,
    pub active: bool,
    pub version: u64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AccountInput {
    pub code: String,
    pub name: String,
    pub class: String,
    pub parent_code: Option<String>,
    pub posting: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AccountUpdate {
    pub name: String,
    pub active: bool,
}
#[derive(Serialize, sqlx::FromRow)]
pub struct Period {
    pub month: String,
    pub closed: bool,
    pub version: u64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PeriodInput {
    pub month: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PeriodUpdate {
    pub closed: bool,
    pub reason: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SourceRef {
    pub module: String,
    pub id: u64,
    pub version: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SourceSnapshot {
    pub reference: SourceRef,
    pub label: String,
    pub amount: String,
    pub date: NaiveDate,
    pub tax: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LineInput {
    pub account: String,
    pub debit: String,
    pub credit: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EntryInput {
    pub date: NaiveDate,
    pub description: String,
    pub reference: String,
    pub kind: String,
    pub source: Option<SourceRef>,
    pub lines: Vec<LineInput>,
}
impl EntryInput {
    pub fn validate(&self) -> Result<i64, CoreError> {
        valid_month(&self.date.format("%Y-%m").to_string())?;
        valid_text(&self.description, 240, true)?;
        valid_text(&self.reference, 120, false)?;
        if !matches!(self.kind.as_str(), "manual" | "apertura" | "origen")
            || (self.kind == "origen") != self.source.is_some()
            || !(2..=100).contains(&self.lines.len())
        {
            return Err(CoreError::Invalid);
        }
        let (mut debit, mut credit) = (0_i64, 0_i64);
        for line in &self.lines {
            valid_code(&line.account)?;
            let (d, c) = (cents(&line.debit)?, cents(&line.credit)?);
            if (d > 0) == (c > 0) {
                return Err(CoreError::Invalid);
            }
            debit = debit.checked_add(d).ok_or(CoreError::Invalid)?;
            credit = credit.checked_add(c).ok_or(CoreError::Invalid)?;
        }
        if debit != credit || debit == 0 || debit > MAX_CENTS {
            return Err(CoreError::Invalid);
        }
        Ok(debit)
    }
}
pub(crate) fn valid_code(code: &str) -> Result<(), CoreError> {
    if code.is_empty() || code.len() > 12 || !code.bytes().all(|c| c.is_ascii_digit()) {
        return Err(CoreError::Invalid);
    }
    Ok(())
}
#[derive(Debug, Serialize, Deserialize)]
pub struct Entry {
    pub id: u64,
    pub version: u64,
    pub status: String,
    pub total: String,
    pub reversal_of: Option<u64>,
    pub reversed_by: Option<u64>,
    pub source_snapshot: Option<SourceSnapshot>,
    pub source_status: String,
    #[serde(flatten)]
    pub data: EntryInput,
}
#[derive(Serialize, sqlx::FromRow)]
pub struct EntrySummary {
    pub id: u64,
    pub version: u64,
    pub date: NaiveDate,
    pub description: String,
    pub status: String,
    pub kind: String,
    pub total: String,
    pub reversed_by: Option<u64>,
}
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EntryFilter {
    pub month: Option<String>,
    pub page: Option<u32>,
}
#[derive(Serialize)]
pub struct EntryPage {
    pub items: Vec<EntrySummary>,
    pub page: u32,
    pub has_more: bool,
}
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EntryAction {
    pub date: Option<NaiveDate>,
    pub reason: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BalanceFilter {
    pub date_from: NaiveDate,
    pub date_to: NaiveDate,
}
#[derive(Serialize)]
pub struct BalanceRow {
    pub code: String,
    pub name: String,
    pub class: String,
    pub opening: String,
    pub debit: String,
    pub credit: String,
    pub balance: String,
}
#[derive(Serialize)]
pub struct Balance {
    pub date_from: NaiveDate,
    pub date_to: NaiveDate,
    pub currency: String,
    pub items: Vec<BalanceRow>,
    pub total_debit: String,
    pub total_credit: String,
    pub balanced: bool,
}
#[derive(Serialize)]
pub struct Proposal {
    pub entry: EntryInput,
    pub source: SourceSnapshot,
    pub notice: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn money_is_exact_and_rejects_invalid_precision() {
        assert_eq!(cents("0.29").unwrap(), 29);
        assert_eq!(cents("999999999999.99").unwrap(), MAX_CENTS);
        for bad in [
            "-1",
            "1.001",
            "1.",
            "1e2",
            "NaN",
            "1000000000000",
            " 1",
            "1,00",
        ] {
            assert!(cents(bad).is_err(), "{bad}");
        }
        assert_eq!(money(-101), "-1.01");
        assert!(valid_month("2026-13").is_err());
    }
    #[test]
    fn entries_must_balance_and_have_only_one_side_per_line() {
        let mut input = EntryInput {
            date: NaiveDate::from_ymd_opt(2026, 11, 1).unwrap(),
            description: "Apertura".into(),
            reference: "".into(),
            kind: "apertura".into(),
            source: None,
            lines: vec![
                LineInput {
                    account: "1101".into(),
                    debit: "0.30".into(),
                    credit: "0".into(),
                },
                LineInput {
                    account: "3101".into(),
                    debit: "0".into(),
                    credit: "0.30".into(),
                },
            ],
        };
        assert_eq!(input.validate().unwrap(), 30);
        input.lines[1].credit = "0.29".into();
        assert!(input.validate().is_err());
        input.lines[1].credit = "0.30".into();
        input.lines[0].credit = "0.01".into();
        assert!(input.validate().is_err());
    }
}
