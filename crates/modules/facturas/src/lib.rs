#![forbid(unsafe_code)]

mod calculation;
mod service;
pub use bastion_cotizaciones::{DiscountType, TaxType};
pub use calculation::{InvoiceCalculation, calculate};
pub use service::InvoiceService;

use bastion_core::CoreError;
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PaymentTerm {
    Contado,
    Credito,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct InvoiceItem {
    pub description: String,
    pub quantity: String,
    pub unit_price: String,
    pub cost_price: String,
    pub discount_type: DiscountType,
    pub discount_value: String,
    pub tax_type: TaxType,
    #[serde(default)]
    pub tracking: String,
    #[serde(default)]
    pub coments: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct InvoiceExpense {
    pub description: String,
    pub amount: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct InvoiceInput {
    #[serde(default)]
    pub id_client: Option<u64>,
    pub client_name: String,
    #[serde(default)]
    pub client_rtn: String,
    #[serde(default)]
    pub client_email: String,
    pub order_number: String,
    pub invoice_date: NaiveDate,
    pub payment_term_type: PaymentTerm,
    pub credit_days: u16,
    pub discount_type: DiscountType,
    pub discount_value: String,
    #[serde(default)]
    pub purchase_order_exempt_number: String,
    #[serde(default)]
    pub registration_certificate_number: String,
    #[serde(default)]
    pub sag_registration_number: String,
    pub notes: String,
    pub public_notes: String,
    pub items: Vec<InvoiceItem>,
    pub expenses: Vec<InvoiceExpense>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct InvoiceDraft {
    pub id: u64,
    pub version: u64,
    pub status: String,
    pub currency: String,
    pub invoice_number: Option<String>,
    pub data: InvoiceInput,
    pub calculation: InvoiceCalculation,
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InvoiceFilter {
    pub search: Option<String>,
    pub page: Option<u32>,
    pub page_size: Option<u32>,
}
impl InvoiceFilter {
    pub fn bounds(&self) -> Result<(u32, u32), CoreError> {
        let page = self.page.unwrap_or(1);
        let size = self.page_size.unwrap_or(25);
        if !(1..=10000).contains(&page)
            || !(1..=100).contains(&size)
            || self
                .search
                .as_ref()
                .is_some_and(|s| s.chars().count() > 200)
        {
            return Err(CoreError::Invalid);
        }
        Ok((page, size))
    }
}

#[derive(Serialize)]
pub struct InvoicePage {
    pub items: Vec<InvoiceSummary>,
    pub page: u32,
    pub page_size: u32,
    pub has_more: bool,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct InvoiceSummary {
    pub id: u64,
    pub version: u64,
    pub client_name: String,
    pub order_number: String,
    pub invoice_date: String,
    pub item_count: u64,
    pub status: String,
    pub total: String,
    pub balance: String,
}
