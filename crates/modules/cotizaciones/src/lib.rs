#![forbid(unsafe_code)]

mod calculation;
mod service;
pub use calculation::{CalculatedItem, QuoteCalculation, QuoteTotals, calculate};
pub use service::{
    QuoteDocument, QuoteDocumentReadiness, QuoteEmailInput, QuoteEmailJob, QuoteMailService,
    QuoteService,
};

use bastion_core::CoreError;
use chrono::{Datelike, NaiveDate};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DiscountType {
    Amount,
    Percent,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub enum TaxType {
    #[serde(rename = "none", alias = "exempt")]
    Exempt,
    #[serde(rename = "isv15")]
    Isv15,
    #[serde(rename = "isv18")]
    Isv18,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize)]
pub enum QuoteStatus {
    #[default]
    Borrador,
    Enviada,
    Aprobada,
    Rechazada,
    Convertido,
}

impl QuoteStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Borrador => "Borrador",
            Self::Enviada => "Enviada",
            Self::Aprobada => "Aprobada",
            Self::Rechazada => "Rechazada",
            Self::Convertido => "Convertido",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct QuoteItem {
    pub description: String,
    pub quantity: String,
    pub unit_price: String,
    pub discount_type: DiscountType,
    pub discount_value: String,
    pub tax_type: TaxType,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct QuoteInput {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id_client: Option<u64>,
    pub client_name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub client_rtn: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub client_email: String,
    pub order_number: String,
    pub quote_date: NaiveDate,
    pub valid_days: u16,
    #[serde(default)]
    pub status: QuoteStatus,
    #[serde(default)]
    pub contact_email: String,
    #[serde(default)]
    pub prepared_by_name: String,
    #[serde(default)]
    pub purchase_order_exempt_number: String,
    #[serde(default)]
    pub registration_certificate_number: String,
    #[serde(default)]
    pub sag_registration_number: String,
    pub notes: String,
    pub terms: String,
    pub discount_type: DiscountType,
    pub discount_value: String,
    pub items: Vec<QuoteItem>,
}

impl QuoteInput {
    pub fn validate(&self) -> Result<(), CoreError> {
        if self.client_name.chars().count() > 255
            || self.id_client == Some(0)
            || self.client_rtn.chars().count() > 30
            || self.client_email.chars().count() > 254
            || self.order_number.chars().count() > 100
            || self.contact_email.chars().count() > 254
            || self.prepared_by_name.chars().count() > 250
            || self.notes.chars().count() > 3000
            || self.terms.chars().count() > 5000
            || self.valid_days > 3650
            || self.quote_date.year() < 1000
            || self.quote_date.year() > 9999
            || [
                &self.purchase_order_exempt_number,
                &self.registration_certificate_number,
                &self.sag_registration_number,
            ]
            .iter()
            .any(|v| v.chars().count() > 150)
            || self.items.len() > 200
        {
            return Err(CoreError::Invalid);
        }
        validate_discount(self.discount_type, &self.discount_value)?;
        for item in &self.items {
            if item.description.chars().count() > 3000 {
                return Err(CoreError::Invalid);
            }
            decimal_units(&item.quantity)?;
            decimal_units(&item.unit_price)?;
            decimal_units(&item.discount_value)?;
        }
        Ok(())
    }
}

// Parsing only, not a fiscal calculation. Four decimal places in an integer.
fn decimal_units(value: &str) -> Result<u64, CoreError> {
    let (whole, fraction) = value.split_once('.').unwrap_or((value, ""));
    if whole.is_empty()
        || whole.len() > 12
        || !whole.bytes().all(|c| c.is_ascii_digit())
        || fraction.len() > 4
        || !fraction.bytes().all(|c| c.is_ascii_digit())
        || (value.contains('.') && fraction.is_empty())
    {
        return Err(CoreError::Invalid);
    }
    let whole = whole.parse::<u64>().map_err(|_| CoreError::Invalid)?;
    let fraction = if fraction.is_empty() {
        0
    } else {
        fraction.parse::<u64>().map_err(|_| CoreError::Invalid)?
            * 10_u64.pow(4 - fraction.len() as u32)
    };
    Ok(whole * 10000 + fraction)
}

fn validate_discount(kind: DiscountType, value: &str) -> Result<(), CoreError> {
    let units = decimal_units(value)?;
    if matches!(kind, DiscountType::Percent) && (units + 50) / 100 > 10000 {
        return Err(CoreError::Invalid);
    }
    Ok(())
}

#[derive(Debug, Deserialize, Serialize)]
pub struct QuoteDraft {
    pub id: u64,
    pub version: u64,
    pub status: String,
    pub currency: String,
    pub data: QuoteInput,
    pub quote_number: Option<String>,
    pub calculation: QuoteCalculation,
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QuoteFilter {
    pub q: Option<String>,
    pub page: Option<u32>,
    pub page_size: Option<u32>,
}

impl QuoteFilter {
    pub fn bounds(&self) -> Result<(u32, u32), CoreError> {
        let page = self.page.unwrap_or(1);
        let size = self.page_size.unwrap_or(25);
        if !(1..=10000).contains(&page)
            || !(1..=100).contains(&size)
            || self.q.as_ref().is_some_and(|q| q.chars().count() > 200)
        {
            return Err(CoreError::Invalid);
        }
        Ok((page, size))
    }
}

#[derive(Serialize)]
pub struct QuotePage {
    pub items: Vec<QuoteSummary>,
    pub page: u32,
    pub page_size: u32,
    pub has_more: bool,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct QuoteSummary {
    pub id: u64,
    pub version: u64,
    pub client_name: String,
    pub order_number: String,
    pub quote_date: String,
    pub item_count: u64,
    pub status: String,
    pub quote_number: Option<String>,
    pub valid_until: Option<String>,
    pub total: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decimal_precision_and_malicious_or_lossy_inputs() {
        assert_eq!(decimal_units("0.0001").unwrap(), 1);
        assert_eq!(
            decimal_units("999999999999.9999").unwrap(),
            9999999999999999
        );
        for value in [
            "-1",
            "NaN",
            "1e2",
            "1,20",
            ".5",
            "1.",
            "1.00001",
            " 1",
            "1.2.3",
            "1000000000000",
            "",
        ] {
            assert!(decimal_units(value).is_err(), "{value}");
        }
        assert!(validate_discount(DiscountType::Percent, "100.005").is_err());
        assert!(validate_discount(DiscountType::Percent, "100").is_ok());
    }

    #[test]
    fn reject_client_totals_status_and_numbering() {
        let input = serde_json::json!({"client_name":"", "order_number":"", "quote_date":"2026-10-22", "valid_days":15, "contact_email":"", "prepared_by_name":"", "notes":"", "terms":"", "discount_type":"amount", "discount_value":"0", "items":[]});
        let parsed: QuoteInput = serde_json::from_value(input.clone()).unwrap();
        assert!(parsed.validate().is_ok());
        assert_eq!(parsed.id_client, None);
        assert!(parsed.client_rtn.is_empty());
        assert!(parsed.client_email.is_empty());
        let serialized = serde_json::to_value(&parsed).unwrap();
        for field in ["id_client", "client_rtn", "client_email"] {
            assert!(
                serialized.get(field).is_none(),
                "Preserve pre-catalog request hashes"
            );
        }
        let mut invalid_client = parsed.clone();
        invalid_client.id_client = Some(0);
        assert!(invalid_client.validate().is_err());
        for status in ["Borrador", "Enviada", "Aprobada", "Rechazada", "Convertido"] {
            let mut value = input.clone();
            value["status"] = serde_json::json!(status);
            assert!(serde_json::from_value::<QuoteInput>(value).is_ok());
        }
        let mut invalid_status = input.clone();
        invalid_status["status"] = serde_json::json!("Pagada");
        assert!(serde_json::from_value::<QuoteInput>(invalid_status).is_err());
        for field in ["total", "quote_number", "unknown_client_field"] {
            let mut value = input.clone();
            value[field] = serde_json::json!("untrusted");
            assert!(serde_json::from_value::<QuoteInput>(value).is_err());
        }
        let mut parsed = parsed;
        parsed.items = vec![QuoteItem {
            description: "Producto".into(),
            quantity: "0".into(),
            unit_price: "0.10".into(),
            discount_type: DiscountType::Amount,
            discount_value: "0".into(),
            tax_type: TaxType::Isv15,
        }];
        assert!(parsed.validate().is_ok());
        parsed.items[0].quantity = "0.5".into();
        assert!(parsed.validate().is_ok());
        parsed.items = vec![parsed.items[0].clone(); 201];
        assert!(parsed.validate().is_err());
    }
}
