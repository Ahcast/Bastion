#![forbid(unsafe_code)]

mod calculation;
mod service;
pub use calculation::{OrderCalculation, OrderTotals, calculate};
pub use service::{OrderService, quote_order_input};

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
    pub cantidad: u32,
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
    #[serde(default)]
    pub source_quote: Option<bastion_cotizaciones::QuoteOrigin>,
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
