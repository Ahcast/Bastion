use bastion_core::CoreError;
use chrono::Days;
use serde::{Deserialize, Serialize};

use crate::{DiscountType, QuoteInput, QuoteItem, TaxType, decimal_units};

#[derive(Debug, Deserialize, Serialize)]
pub struct QuoteCalculation {
    pub data: QuoteInput,
    pub valid_until: String,
    pub items: Vec<CalculatedItem>,
    pub totals: QuoteTotals,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct QuoteTotals {
    pub subtotal: String,
    pub importe_exonerado: String,
    pub importe_exento: String,
    pub importe_gravado_15: String,
    pub importe_gravado_18: String,
    pub descuentos_rebajas: String,
    pub isv_15: String,
    pub isv_18: String,
    pub total: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct CalculatedItem {
    pub subtotal: String,
    pub unit_discount: String,
    pub item_discount_total: String,
    pub global_line_discount: String,
    pub discount_amount: String,
    pub taxable_base: String,
    pub tax_type: TaxType,
    pub tax_amount: String,
    // Legacy meaning: after item discount, before global discount and tax.
    pub total: String,
}

struct Line {
    subtotal: i128,
    unit_discount: i128,
    item_discount: i128,
    base: i128,
    tax_type: TaxType,
}

pub fn calculate(input: &QuoteInput) -> Result<QuoteCalculation, CoreError> {
    input.validate()?;
    let mut data = input.clone();
    data.client_name = data.client_name.trim().into();
    data.order_number = data.order_number.trim().into();
    data.notes = data.notes.trim().into();
    data.terms = data.terms.trim().into();
    let mut gross = 0_i128; // hundredths of one cent, not rounded per line.
    let mut bases = 0_i128;
    let mut normalized = Vec::new();
    let mut lines = Vec::new();
    for item in &input.items {
        let quantity = cents(&item.quantity)?;
        let price = cents(&item.unit_price)?;
        let discount = cents(&item.discount_value)?;
        let description = item.description.trim();
        if description.is_empty() && quantity == 0 && price == 0 {
            continue;
        }
        let unit_discount = match item.discount_type {
            DiscountType::Percent => {
                if discount > 10000 {
                    return Err(CoreError::Invalid);
                }
                round_div(mul(price, discount)?, 10000)?
            }
            DiscountType::Amount => {
                if discount > price {
                    return Err(CoreError::Invalid);
                }
                discount
            }
        };
        let product = mul(quantity, price)?;
        gross = add(gross, product)?;
        let base = round_div(mul(quantity, price - unit_discount)?, 100)?;
        bases = add(bases, base)?;
        normalized.push(QuoteItem {
            description: description.into(),
            quantity: money(quantity),
            unit_price: money(price),
            discount_type: item.discount_type,
            discount_value: money(discount),
            tax_type: item.tax_type,
        });
        lines.push(Line {
            subtotal: round_div(product, 100)?,
            unit_discount,
            item_discount: round_div(mul(quantity, unit_discount)?, 100)?,
            base,
            tax_type: item.tax_type,
        });
    }
    data.items = normalized;
    let discount_value = cents(&data.discount_value)?;
    data.discount_value = money(discount_value);
    let global_discount = match data.discount_type {
        DiscountType::Percent => {
            if discount_value > 10000 {
                return Err(CoreError::Invalid);
            }
            round_div(mul(bases, discount_value)?, 10000)?
        }
        DiscountType::Amount => {
            if discount_value > bases {
                return Err(CoreError::Invalid);
            }
            discount_value
        }
    };
    let exonerated = [
        &data.purchase_order_exempt_number,
        &data.registration_certificate_number,
        &data.sag_registration_number,
    ]
    .iter()
    .any(|value| !value.trim().is_empty());
    if exonerated {
        for item in &mut data.items {
            item.tax_type = TaxType::Exempt;
        }
    }
    let mut tax_bases = [0_i128; 4]; // exonerated, exempt, 15%, 18%.
    let mut discounts = 0_i128;
    let mut items = Vec::new();
    for line in lines {
        let allocated = if bases > 0 {
            round_div(mul(global_discount, line.base)?, bases)?
        } else {
            0
        };
        let discount = add(allocated, line.item_discount)?.min(line.subtotal);
        discounts = add(discounts, discount)?;
        let base = (line.base - allocated).max(0);
        let tax_type = if exonerated {
            TaxType::Exempt
        } else {
            line.tax_type
        };
        let (index, rate) = if exonerated {
            (0, 0)
        } else {
            match tax_type {
                TaxType::Exempt => (1, 0),
                TaxType::Isv15 => (2, 15),
                TaxType::Isv18 => (3, 18),
            }
        };
        tax_bases[index] = add(tax_bases[index], base)?;
        items.push(CalculatedItem {
            subtotal: money(line.subtotal),
            unit_discount: money(line.unit_discount),
            item_discount_total: money(line.item_discount),
            global_line_discount: money(allocated),
            discount_amount: money(discount),
            taxable_base: money(base),
            tax_type,
            tax_amount: money(round_div(mul(base, rate)?, 100)?),
            total: money(line.base),
        });
    }
    let tax15 = round_div(mul(tax_bases[2], 15)?, 100)?;
    let tax18 = round_div(mul(tax_bases[3], 18)?, 100)?;
    let adjustment = mul(add(add(-discounts, tax15)?, tax18)?, 100)?;
    let total = round_div(add(gross, adjustment)?, 100)?;
    let valid_until = data
        .quote_date
        .checked_add_days(Days::new(u64::from(data.valid_days)))
        .filter(|date| *date <= chrono::NaiveDate::from_ymd_opt(9999, 12, 31).unwrap())
        .ok_or(CoreError::Invalid)?
        .to_string();
    Ok(QuoteCalculation {
        data,
        valid_until,
        items,
        totals: QuoteTotals {
            subtotal: money(round_div(gross, 100)?),
            importe_exonerado: money(tax_bases[0]),
            importe_exento: money(tax_bases[1]),
            importe_gravado_15: money(tax_bases[2]),
            importe_gravado_18: money(tax_bases[3]),
            descuentos_rebajas: money(discounts),
            isv_15: money(tax15),
            isv_18: money(tax18),
            total: money(total),
        },
    })
}

fn cents(value: &str) -> Result<i128, CoreError> {
    let rounded = round_div(i128::from(decimal_units(value)?), 100)?;
    if rounded > 99_999_999_999_999 {
        return Err(CoreError::Invalid);
    }
    Ok(rounded)
}
fn mul(a: i128, b: i128) -> Result<i128, CoreError> {
    a.checked_mul(b).ok_or(CoreError::Invalid)
}
fn add(a: i128, b: i128) -> Result<i128, CoreError> {
    a.checked_add(b).ok_or(CoreError::Invalid)
}
fn round_div(value: i128, divisor: i128) -> Result<i128, CoreError> {
    if divisor <= 0 {
        return Err(CoreError::Invalid);
    }
    let magnitude = value.checked_abs().ok_or(CoreError::Invalid)?;
    // Remainder comparison avoids overflow from doubling the numerator.
    let remainder = magnitude % divisor;
    let rounded = add(
        magnitude / divisor,
        if remainder >= divisor / 2 + divisor % 2 {
            1
        } else {
            0
        },
    )?;
    Ok(if value < 0 { -rounded } else { rounded })
}
fn money(cents: i128) -> String {
    let sign = if cents < 0 { "-" } else { "" };
    let value = cents.abs();
    format!("{sign}{}.{:02}", value / 100, value % 100)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn quote(items: serde_json::Value, kind: &str, value: &str) -> QuoteInput {
        serde_json::from_value(serde_json::json!({"client_name":"", "order_number":"", "quote_date":"2026-10-22", "valid_days":30, "notes":"", "terms":"", "discount_type":kind, "discount_value":value, "items":items})).unwrap()
    }
    fn item(q: &str, p: &str, kind: &str, d: &str, tax: &str) -> serde_json::Value {
        serde_json::json!({"description":"Equipo", "quantity":q, "unit_price":p, "discount_type":kind, "discount_value":d, "tax_type":tax})
    }
    #[test]
    fn unit_discount_then_global_discount_and_mixed_taxes() {
        let q = quote(
            serde_json::json!([
                item("2", "100", "percent", "10", "isv15"),
                item("1", "50", "amount", "5", "isv18"),
                item("1", "25", "amount", "0", "none")
            ]),
            "percent",
            "10",
        );
        let c = calculate(&q).unwrap();
        assert_eq!(c.valid_until, "2026-11-21");
        assert_eq!(c.totals.subtotal, "275.00");
        assert_eq!(c.items[0].unit_discount, "10.00");
        assert_eq!(c.items[0].discount_amount, "38.00");
        assert_eq!(c.items[0].total, "180.00");
        assert_eq!(c.totals.descuentos_rebajas, "50.00");
        assert_eq!(c.totals.isv_15, "24.30");
        assert_eq!(c.totals.isv_18, "7.29");
        assert_eq!(c.totals.total, "256.59");
    }
    #[test]
    fn zero_days_blank_rows_and_normalization() {
        let mut q = quote(
            serde_json::json!([item("0.125","10.125","amount","0","none"), {"description":" ","quantity":"0","unit_price":"0","discount_type":"amount","discount_value":"0","tax_type":"none"}]),
            "amount",
            "0",
        );
        q.valid_days = 0;
        let c = calculate(&q).unwrap();
        assert_eq!(c.valid_until, "2026-10-22");
        assert_eq!(c.data.items.len(), 1);
        assert_eq!(c.data.items[0].quantity, "0.13");
        assert_eq!(c.data.items[0].unit_price, "10.13");
        assert_eq!(c.totals.total, "1.32");
    }
    #[test]
    fn document_tax_is_rounded_on_group_base_and_no_residue_is_redistributed() {
        let line = item("1", "0.03", "amount", "0", "isv15");
        let c = calculate(&quote(
            serde_json::json!([line.clone(), line]),
            "amount",
            "0",
        ))
        .unwrap();
        assert_eq!(c.items[0].tax_amount, "0.00");
        assert_eq!(c.totals.isv_15, "0.01");
        let line = item("1", "0.01", "amount", "0", "none");
        let c = calculate(&quote(
            serde_json::json!([line.clone(), line.clone(), line]),
            "amount",
            "0.02",
        ))
        .unwrap();
        assert_eq!(c.totals.descuentos_rebajas, "0.03");
        assert_eq!(c.totals.total, "0.00");
    }
    #[test]
    fn reject_excess_discounts_and_out_of_range_dates() {
        assert!(
            calculate(&quote(
                serde_json::json!([item("2", "10", "amount", "11", "none")]),
                "amount",
                "0"
            ))
            .is_err()
        );
        assert!(
            calculate(&quote(
                serde_json::json!([item("1", "10", "amount", "0", "none")]),
                "amount",
                "10.01"
            ))
            .is_err()
        );
        let mut q = quote(
            serde_json::json!([item("1", "10", "amount", "0", "isv18")]),
            "amount",
            "0",
        );
        q.sag_registration_number = "TEST".into();
        let c = calculate(&q).unwrap();
        assert_eq!(c.totals.importe_exonerado, "10.00");
        assert_eq!(c.totals.isv_18, "0.00");
        q.quote_date = "9999-12-31".parse().unwrap();
        assert!(calculate(&q).is_err());
        assert!(
            calculate(&quote(
                serde_json::json!([item("1", "999999999999.9999", "amount", "0", "none")]),
                "amount",
                "0"
            ))
            .is_err()
        );
    }

    #[test]
    fn rounding_uses_away_from_zero_and_preserves_legacy_fractional_line_behavior() {
        assert_eq!(round_div(-5, 10).unwrap(), -1);
        assert_eq!(round_div(5, 10).unwrap(), 1);
        let c = calculate(&quote(
            serde_json::json!([item("0.01", "0.50", "amount", "0.50", "none")]),
            "amount",
            "0",
        ))
        .unwrap();
        assert_eq!(c.totals.total, "-0.01");
    }
}
