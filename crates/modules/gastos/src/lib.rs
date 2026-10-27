#![forbid(unsafe_code)]

mod calculation;
mod service;

pub use calculation::{ExpenseCalculation, calculate};
pub use service::ExpenseService;

use bastion_core::CoreError;
use chrono::{Datelike, NaiveDate};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ExpenseInput {
    pub expense_date: NaiveDate,
    pub invoice_number: String,
    pub provider: String,
    pub currency: String,
    pub taxable_amount: String,
    pub exempt_amount: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Expense {
    pub id: u64,
    pub version: u64,
    #[serde(flatten)]
    pub data: ExpenseInput,
    pub isv_15: String,
    pub total: String,
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExpenseFilter {
    pub search: Option<String>,
    pub date_from: Option<NaiveDate>,
    pub date_to: Option<NaiveDate>,
    pub page: Option<u32>,
    pub page_size: Option<u32>,
}

impl ExpenseFilter {
    pub fn bounds(&self) -> Result<(u32, u32), CoreError> {
        let page = self.page.unwrap_or(1);
        let size = self.page_size.unwrap_or(20);
        if !(1..=10000).contains(&page)
            || !(1..=100).contains(&size)
            || self
                .search
                .as_ref()
                .is_some_and(|value| value.chars().count() > 200)
            || self
                .date_from
                .is_some_and(|date| !(1000..=9999).contains(&date.year()))
            || self
                .date_to
                .is_some_and(|date| !(1000..=9999).contains(&date.year()))
            || matches!((self.date_from, self.date_to), (Some(from), Some(to)) if from > to)
        {
            return Err(CoreError::Invalid);
        }
        Ok((page, size))
    }

    pub(crate) fn search(&self) -> Option<&str> {
        self.search
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
    }
}

#[derive(Debug, Serialize)]
pub struct ExpensePage {
    pub items: Vec<Expense>,
    pub page: u32,
    pub page_size: u32,
    pub has_more: bool,
    pub summary: ExpenseSummary,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct ExpenseSummary {
    pub count: u64,
    pub taxable_amount: String,
    pub exempt_amount: String,
    pub isv_15: String,
    pub total: String,
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExpenseReportFilter {
    pub date_from: NaiveDate,
    pub date_to: NaiveDate,
    pub search: Option<String>,
    pub page: Option<u32>,
    pub page_size: Option<u32>,
}

impl ExpenseReportFilter {
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
        {
            return Err(CoreError::Invalid);
        }
        Ok((page, page_size))
    }

    pub(crate) fn search(&self) -> Option<&str> {
        self.search
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
    }
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct ExpenseReportRow {
    pub id: u64,
    pub invoice_number: String,
    pub expense_date: String,
    pub provider: String,
    pub currency: String,
    pub taxable_amount: String,
    pub exempt_amount: String,
    pub isv_15: String,
    pub total: String,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct ExpenseCurrencyTotals {
    pub currency: String,
    pub records: u64,
    pub taxable_amount: String,
    pub exempt_amount: String,
    pub isv_15: String,
    pub total: String,
}

#[derive(Debug, Serialize)]
pub struct ExpenseReport {
    pub items: Vec<ExpenseReportRow>,
    pub totals: Vec<ExpenseCurrencyTotals>,
    pub page: u32,
    pub page_size: u32,
    pub has_more: bool,
}
