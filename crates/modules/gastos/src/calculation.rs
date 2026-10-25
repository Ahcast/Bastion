use crate::ExpenseInput;
use bastion_core::CoreError;
use chrono::Datelike;
use serde::{Deserialize, Serialize};

const MAX_CENTS: i128 = 99_999_999_999_999;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ExpenseCalculation {
    pub data: ExpenseInput,
    pub isv_15: String,
    pub total: String,
}

pub fn calculate(input: &ExpenseInput) -> Result<ExpenseCalculation, CoreError> {
    if !(1000..=9999).contains(&input.expense_date.year()) {
        return Err(CoreError::Invalid);
    }
    let mut data = input.clone();
    text(&mut data.invoice_number, 120)?;
    text(&mut data.provider, 255)?;
    if data.invoice_number.is_empty() || data.provider.is_empty() || data.currency != "HNL" {
        return Err(CoreError::Invalid);
    }

    let taxable = cents(&data.taxable_amount)?;
    let exempt = cents(&data.exempt_amount)?;
    if taxable == 0 && exempt == 0 {
        return Err(CoreError::Invalid);
    }
    data.taxable_amount = money(taxable)?;
    data.exempt_amount = money(exempt)?;

    let tax = taxable
        .checked_mul(15)
        .and_then(|value| value.checked_add(50))
        .map(|value| value / 100)
        .ok_or(CoreError::Invalid)?;
    let total = taxable
        .checked_add(exempt)
        .and_then(|value| value.checked_add(tax))
        .filter(|value| *value <= MAX_CENTS)
        .ok_or(CoreError::Invalid)?;

    Ok(ExpenseCalculation {
        data,
        isv_15: money(tax)?,
        total: money(total)?,
    })
}

fn text(value: &mut String, max: usize) -> Result<(), CoreError> {
    *value = value.trim().to_owned();
    if value.chars().count() > max || value.chars().any(char::is_control) {
        return Err(CoreError::Invalid);
    }
    Ok(())
}

fn cents(value: &str) -> Result<i128, CoreError> {
    let value = value.trim();
    let mut parts = value.split('.');
    let whole = parts.next().ok_or(CoreError::Invalid)?;
    let fraction = parts.next().unwrap_or("");
    if parts.next().is_some()
        || whole.is_empty()
        || whole.len() > 12
        || !whole.bytes().all(|byte| byte.is_ascii_digit())
        || fraction.len() > 4
        || !fraction.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(CoreError::Invalid);
    }
    let whole: i128 = whole.parse().map_err(|_| CoreError::Invalid)?;
    let padded = format!("{fraction:0<4}");
    let fraction: i128 = padded.parse().map_err(|_| CoreError::Invalid)?;
    let scaled = whole
        .checked_mul(10_000)
        .and_then(|number| number.checked_add(fraction))
        .ok_or(CoreError::Invalid)?;
    let normalized = (scaled + 50) / 100;
    if normalized > MAX_CENTS {
        return Err(CoreError::Invalid);
    }
    Ok(normalized)
}

fn money(cents: i128) -> Result<String, CoreError> {
    if !(0..=MAX_CENTS).contains(&cents) {
        return Err(CoreError::Invalid);
    }
    Ok(format!("{}.{:02}", cents / 100, cents % 100))
}
