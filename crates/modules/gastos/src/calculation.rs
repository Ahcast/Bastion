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
    text(&mut data.invoice_number, 50)?;
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
    let fraction = match parts.next() {
        Some("") => return Err(CoreError::Invalid),
        Some(value) => value,
        None => "",
    };
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

#[cfg(test)]
mod tests {
    use super::*;
    fn input(taxable: &str, exempt: &str) -> ExpenseInput {
        ExpenseInput {
            expense_date: chrono::NaiveDate::from_ymd_opt(2026, 10, 24).unwrap(),
            invoice_number: " FACT-1 ".into(),
            provider: " Proveedor ".into(),
            currency: "HNL".into(),
            taxable_amount: taxable.into(),
            exempt_amount: exempt.into(),
        }
    }
    #[test]
    fn legacy_rounding_sequence_and_decimal_boundaries() {
        for (taxable, exempt, base, exempt_base, tax, total) in [
            ("100.005", "20.005", "100.01", "20.01", "15.00", "135.02"),
            ("0.10", "0", "0.10", "0.00", "0.02", "0.12"),
            ("0.005", "0", "0.01", "0.00", "0.00", "0.01"),
            ("0", "123.4567", "0.00", "123.46", "0.00", "123.46"),
            (
                "0",
                "999999999999.9949",
                "0.00",
                "999999999999.99",
                "0.00",
                "999999999999.99",
            ),
        ] {
            let result = calculate(&input(taxable, exempt)).unwrap();
            assert_eq!(result.data.taxable_amount, base);
            assert_eq!(result.data.exempt_amount, exempt_base);
            assert_eq!(result.isv_15, tax);
            assert_eq!(result.total, total);
            assert_eq!(result.data.invoice_number, "FACT-1");
            assert_eq!(result.data.provider, "Proveedor");
        }
        for value in [
            "",
            "-1",
            "+1",
            "NaN",
            "1e2",
            "1,00",
            "1.",
            "1..0",
            "1.00001",
            "999999999999.9950",
            "1000000000000",
        ] {
            assert!(
                matches!(calculate(&input(value, "0")), Err(CoreError::Invalid)),
                "{value}"
            );
        }
        assert!(calculate(&input("0.0049", "0")).is_err());
        assert!(calculate(&input("999999999999.99", "0")).is_err());
    }
    #[test]
    fn legacy_fields_and_required_values_are_enforced() {
        let mut value = input("1", "0");
        value.invoice_number = "a".repeat(50);
        assert!(calculate(&value).is_ok());
        value.invoice_number.push('a');
        assert!(calculate(&value).is_err());
        value.invoice_number = "FACT-1".into();
        value.provider = "á".repeat(255);
        assert!(calculate(&value).is_ok());
        value.provider.push('a');
        assert!(calculate(&value).is_err());
        value.provider = " ".into();
        assert!(calculate(&value).is_err());
        value.provider = "test\nprovider".into();
        assert!(calculate(&value).is_err());
        value.provider = "Proveedor".into();
        value.currency = "USD".into();
        assert!(calculate(&value).is_err());
        value.currency = "HNL".into();
        value.expense_date = chrono::NaiveDate::from_ymd_opt(999, 1, 1).unwrap();
        assert!(calculate(&value).is_err());
    }
}
