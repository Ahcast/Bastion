#![forbid(unsafe_code)]

mod calculation;
mod service;
pub use calculation::{OrderCalculation, OrderTotals, calculate};
pub use service::OrderService;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SourceQuote {
    pub id: u64,
    pub version: u64,
    pub quote_number: Option<String>,
}

// Old persisted documents and clients use integer JSON quantities.
fn quantity<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Quantity {
        Text(String),
        Integer(u32),
    }
    Ok(match Quantity::deserialize(deserializer)? {
        Quantity::Text(s) => s,
        Quantity::Integer(n) => n.to_string(),
    })
}
use bastion_core::CoreError;

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum ProductStatus {
    Pendiente,
    Comprado,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OrderProduct {
    pub description: String,
    #[serde(deserialize_with = "quantity")]
    pub cantidad: String,
    // Legacy names: sale_price is cost; purchase_price is selling price.
    pub sale_price: String,
    pub purchase_price: String,
    pub tracking: String,
    pub isv: bool,
    pub status: ProductStatus,
    pub coments: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OrderExpense {
    pub description: String,
    pub amount: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OrderInput {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approved_quote: Option<bastion_cotizaciones::QuoteInput>,
    pub n_orders: String,
    pub n_bill: String,
    pub order_date: NaiveDate,
    pub received_date: Option<NaiveDate>,
    pub client_name: String,
    pub client_rtn: String,
    pub id_destination: Option<u64>,
    pub comments: String,
    pub products: Vec<OrderProduct>,
    pub expenses: Vec<OrderExpense>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PurchaseOrder {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_quote: Option<SourceQuote>,
    pub id: u64,
    pub version: u64,
    pub status: String,
    pub currency: String,
    pub data: OrderInput,
    pub calculation: OrderCalculation,
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OrderFilter {
    pub search: Option<String>,
    pub status: Option<String>,
    pub min_total: Option<String>,
    pub max_total: Option<String>,
    pub page: Option<u32>,
    pub page_size: Option<u32>,
}
impl OrderFilter {
    pub fn bounds(&self) -> Result<(u32, u32), CoreError> {
        let page = self.page.unwrap_or(1);
        let size = self.page_size.unwrap_or(25);
        if !(1..=10000).contains(&page)
            || !(1..=100).contains(&size)
            || self
                .search
                .as_ref()
                .is_some_and(|s| s.chars().count() > 200)
            || self
                .status
                .as_deref()
                .is_some_and(|s| !["Pendiente", "Listo", "Entregado"].contains(&s))
        {
            return Err(CoreError::Invalid);
        }
        let min = self
            .min_total
            .as_deref()
            .map(calculation::decimal)
            .transpose()?;
        let max = self
            .max_total
            .as_deref()
            .map(calculation::decimal)
            .transpose()?;
        if matches!((min, max), (Some(a), Some(b)) if a > b) {
            return Err(CoreError::Invalid);
        }
        Ok((page, size))
    }
}

#[derive(Serialize)]
pub struct OrderPage {
    pub items: Vec<OrderSummary>,
    pub page: u32,
    pub page_size: u32,
    pub has_more: bool,
}
#[derive(Serialize, sqlx::FromRow)]
pub struct OrderSummary {
    pub id: u64,
    pub version: u64,
    pub n_orders: String,
    pub n_bill: String,
    pub client_name: String,
    pub order_date: String,
    pub status: String,
    pub total: String,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Destination {
    pub id: u64,
    pub version: u64,
    pub destination: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DestinationInput {
    pub destination: String,
}
