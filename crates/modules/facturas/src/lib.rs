#![forbid(unsafe_code)]

mod calculation;
mod service;
pub use bastion_cotizaciones::{DiscountType, TaxType};
pub use calculation::{InvoiceCalculation, calculate};
pub use service::InvoiceService;

use bastion_core::CoreError;
use chrono::{Datelike, NaiveDate};
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
    // Read old drafts without returning or persisting these retired fields.
    #[serde(default, rename = "tracking", skip_serializing)]
    _legacy_tracking: String,
    #[serde(default, rename = "coments", skip_serializing)]
    _legacy_comments: String,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub initial_payment: Option<PaymentInput>,
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

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct OrderOrigin {
    pub id: u64,
    pub version: u64,
    pub number: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct InvoiceDraft {
    #[serde(default)]
    pub payments: Vec<InvoicePayment>,
    #[serde(default)]
    pub sar_snapshot: Option<bastion_core::SarSettings>,
    #[serde(default)]
    pub company_snapshot: Option<bastion_core::DocumentCompany>,
    #[serde(default)]
    pub source_order: Option<OrderOrigin>,
    #[serde(default)]
    pub source_quote: Option<bastion_cotizaciones::QuoteOrigin>,
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
    pub invoice_number: Option<String>,
    pub paid_total: String,
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

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InvoiceReportFilter {
    pub date_from: NaiveDate,
    pub date_to: NaiveDate,
    pub search: Option<String>,
    pub status: Option<String>,
    pub page: Option<u32>,
    pub page_size: Option<u32>,
}

impl InvoiceReportFilter {
    pub fn bounds(&self) -> Result<(u32, u32), CoreError> {
        let page = self.page.unwrap_or(1);
        let page_size = self.page_size.unwrap_or(25);
        if !(1000..=9999).contains(&self.date_from.year())
            || !(1000..=9999).contains(&self.date_to.year())
            || self.date_from > self.date_to
            || self
                .date_to
                .signed_duration_since(self.date_from)
                .num_days()
                > 365
            || self
                .search
                .as_ref()
                .is_some_and(|value| value.chars().count() > 200)
            || !(1..=10000).contains(&page)
            || !(1..=1000).contains(&page_size)
            || self
                .status
                .as_deref()
                .is_some_and(|status| !matches!(status, "Borrador" | "Enviado" | "Pagado"))
        {
            return Err(CoreError::Invalid);
        }
        Ok((page, page_size))
    }
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct InvoiceReportRow {
    pub invoice_number: Option<String>,
    pub id: u64,
    pub client_name: String,
    pub order_number: String,
    pub invoice_date: String,
    pub status: String,
    pub total: String,
    pub paid_total: String,
    pub balance: String,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct InvoiceReportTotals {
    pub records: u64,
    pub total: String,
    pub paid_total: String,
    pub balance: String,
}

#[derive(Debug, Serialize)]
pub struct InvoiceReport {
    pub items: Vec<InvoiceReportRow>,
    pub totals: InvoiceReportTotals,
    pub page: u32,
    pub page_size: u32,
    pub has_more: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PaymentInput {
    pub amount: String,
    #[serde(default = "payment_today", deserialize_with = "payment_date")]
    pub date: NaiveDate,
    #[serde(default)]
    pub method: String,
    #[serde(default)]
    pub reference: String,
}

fn payment_today() -> NaiveDate {
    chrono::Utc::now()
        .with_timezone(&chrono::FixedOffset::west_opt(21600).expect("Honduras offset"))
        .date_naive()
}
fn payment_date<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<NaiveDate, D::Error> {
    Ok(Option::<NaiveDate>::deserialize(deserializer)?.unwrap_or_else(payment_today))
}

#[derive(Clone, Debug, Deserialize, Serialize, sqlx::FromRow)]
pub struct InvoicePayment {
    pub id: u64,
    pub amount: String,
    pub date: NaiveDate,
    pub method: String,
    pub reference: String,
}
