#![forbid(unsafe_code)]

mod service;
use bastion_colaboracion::{FormValues, PublishedForm};
use bastion_core::CoreError;
use serde::{Deserialize, Serialize};
pub use service::TenderService;

pub const MAX_INPUT_BYTES: u64 = 20_000_000;
pub const MAX_EXPEDIENT_BYTES: u64 = 500_000_000;
pub const MAX_FINAL_PDF_BYTES: u64 = 500_000_000;

/// Independent budgets; revisions count until an explicit retention policy exists.
pub fn validate_input_budget(retained_bytes: u64, actual_bytes: u64) -> Result<u64, CoreError> {
    if actual_bytes == 0 || actual_bytes > MAX_INPUT_BYTES {
        return Err(CoreError::Invalid);
    }
    retained_bytes
        .checked_add(actual_bytes)
        .filter(|v| *v <= MAX_EXPEDIENT_BYTES)
        .ok_or(CoreError::Invalid)
}
pub fn validate_final_pdf_budget(actual_bytes: u64) -> Result<(), CoreError> {
    if actual_bytes == 0 || actual_bytes > MAX_FINAL_PDF_BYTES {
        return Err(CoreError::Invalid);
    }
    Ok(())
}
#[derive(Clone, Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct ExpedientSummary {
    pub id: u64,
    pub version: u64,
    pub state: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FormCopy {
    pub id: u64,
    pub source: PublishedForm,
    pub values: FormValues,
    pub missing_required: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Expedient {
    pub id: u64,
    pub version: u64,
    pub state: String,
    pub forms: Vec<FormCopy>,
}
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PageQuery {
    pub page: Option<u32>,
    pub page_size: Option<u32>,
}
impl PageQuery {
    pub fn bounds(&self) -> Result<(u32, u32), CoreError> {
        let page = self.page.unwrap_or(1);
        let size = self.page_size.unwrap_or(25);
        if !(1..=10000).contains(&page) || !(1..=100).contains(&size) {
            return Err(CoreError::Invalid);
        }
        Ok((page, size))
    }
}
#[derive(Serialize)]
pub struct ExpedientPage {
    pub items: Vec<ExpedientSummary>,
    pub page: u32,
    pub page_size: u32,
    pub has_more: bool,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CopyFormInput {
    pub source_id: u64,
    pub revision: u64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ValuesInput {
    pub values: FormValues,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EmptyInput {}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeleteInput {
    pub confirm_id: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn budgets_keep_decimal_mb_and_final_pdf_separate() {
        assert_eq!(
            validate_input_budget(480_000_000, 20_000_000).unwrap(),
            500_000_000
        );
        assert!(validate_input_budget(480_000_001, 20_000_000).is_err());
        assert!(validate_input_budget(0, 20_000_001).is_err());
        assert!(validate_input_budget(u64::MAX, 1).is_err());
        assert!(validate_input_budget(0, 0).is_err());
        assert!(validate_final_pdf_budget(500_000_000).is_ok());
        assert!(validate_final_pdf_budget(500_000_001).is_err());
    }
    #[test]
    fn clients_cannot_assign_state_schema_or_business_fields() {
        assert!(serde_json::from_str::<EmptyInput>(r#"{"state":"enviada"}"#).is_err());
        assert!(serde_json::from_str::<EmptyInput>(r#"{"nombre":"inventado"}"#).is_err());
        assert!(serde_json::from_str::<ValuesInput>(r#"{"values":{},"schema":{}}"#).is_err());
        assert!(
            PageQuery {
                page: Some(u32::MAX),
                page_size: None
            }
            .bounds()
            .is_err()
        );
    }
}
