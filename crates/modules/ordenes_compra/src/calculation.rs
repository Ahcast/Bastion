use crate::{OrderInput, ProductStatus};
use bastion_core::CoreError;
use chrono::Datelike;
use serde::{Deserialize, Serialize};

const SCALE: i128 = 10000;
const MAX: i128 = 99999999999999; // cents, normalized totals must remain editable

#[derive(Debug, Deserialize, Serialize)]
pub struct OrderTotals {
    pub subtotal: String,
    pub subtotal_exonerado: String,
    pub tax: String,
    pub total: String,
    pub products_investment: String,
    pub other_expenses: String,
    pub investment_total: String,
    pub profit: String,
}
#[derive(Debug, Deserialize, Serialize)]
pub struct OrderCalculation {
    pub data: OrderInput,
    pub status: String,
    pub totals: OrderTotals,
}
impl OrderCalculation {
    pub fn ensure_saveable(&self) -> Result<(), CoreError> {
        if self.data.n_orders.is_empty()
            || self.data.client_name.is_empty()
            || self.data.id_destination.is_none()
        {
            return Err(CoreError::Invalid);
        }
        Ok(())
    }
}

pub fn calculate(input: &OrderInput) -> Result<OrderCalculation, CoreError> {
    if input.products.len() > 200
        || input.expenses.len() > 200
        || !(1000..=9999).contains(&input.order_date.year())
        || input
            .received_date
            .is_some_and(|d| !(1000..=9999).contains(&d.year()))
        || input.id_destination == Some(0)
    {
        return Err(CoreError::Invalid);
    }
    let mut data = input.clone();
    text(&mut data.n_orders, 50)?;
    text(&mut data.n_bill, 120)?;
    text(&mut data.client_name, 255)?;
    text(&mut data.client_rtn, 30)?;
    text(&mut data.comments, 3000)?;
    let (mut subtotal, mut exempt, mut investment) = (0_i128, 0_i128, 0_i128);
    for row in &mut data.products {
        text(&mut row.description, 2000)?;
        text(&mut row.tracking, 255)?;
        text(&mut row.coments, 1000)?;
        if row.cantidad > 1000000 {
            return Err(CoreError::Invalid);
        }
        let cost = decimal(&row.sale_price)?;
        let sale = decimal(&row.purchase_price)?;
        // Preserve four decimal unit prices; rounding only after aggregation,
        // as calculateOrderTotals in Legacy. No floating-point arithmetic.
        row.sale_price = unit(cost);
        row.purchase_price = unit(sale);
        let line = sale * i128::from(row.cantidad);
        subtotal += line;
        if !row.isv {
            exempt += line;
        }
        investment += cost * i128::from(row.cantidad);
    }
    let mut expenses = 0_i128;
    for expense in &mut data.expenses {
        text(&mut expense.description, 255)?;
        let cents = round(decimal(&expense.amount)?, 100);
        expense.amount = money(cents)?;
        expenses += cents;
    }
    data.expenses.retain(|e| e.amount != "0.00");
    let tax_raw = (subtotal - exempt) * 15; // denominator SCALE * 100
    let tax = round(tax_raw, 10000);
    let total = round(subtotal * 100 + tax_raw, 10000);
    let product_investment = round(investment, 100);
    let investment_total = round(investment + expenses * 100, 100);
    let status = if !data.n_bill.is_empty() {
        "Entregado"
    } else if !data.products.is_empty()
        && data
            .products
            .iter()
            .all(|p| p.status == ProductStatus::Comprado)
    {
        "Listo"
    } else {
        "Pendiente"
    };
    Ok(OrderCalculation {
        status: status.into(),
        data,
        totals: OrderTotals {
            subtotal: money(round(subtotal, 100))?,
            subtotal_exonerado: money(round(exempt, 100))?,
            tax: money(tax)?,
            total: money(total)?,
            products_investment: money(product_investment)?,
            other_expenses: money(expenses)?,
            investment_total: money(investment_total)?,
            profit: money(total - tax - investment_total)?,
        },
    })
}

pub(crate) fn decimal(value: &str) -> Result<i128, CoreError> {
    let (whole, fraction) = value.split_once('.').unwrap_or((value, ""));
    if whole.is_empty()
        || whole.len() > 12
        || fraction.len() > 4
        || !whole.bytes().all(|b| b.is_ascii_digit())
        || !fraction.bytes().all(|b| b.is_ascii_digit())
        || (value.contains('.') && fraction.is_empty())
    {
        return Err(CoreError::Invalid);
    }
    let integer = whole.parse::<i128>().map_err(|_| CoreError::Invalid)?;
    let decimal = if fraction.is_empty() {
        0
    } else {
        fraction.parse::<i128>().map_err(|_| CoreError::Invalid)?
            * 10_i128.pow(4 - fraction.len() as u32)
    };
    Ok(integer * SCALE + decimal)
}
fn unit(value: i128) -> String {
    format!("{}.{:04}", value / SCALE, value % SCALE)
}
fn round(value: i128, divisor: i128) -> i128 {
    (value + divisor / 2) / divisor
}
fn money(cents: i128) -> Result<String, CoreError> {
    if cents.abs() > MAX {
        return Err(CoreError::Invalid);
    }
    Ok(format!(
        "{}{}.{:02}",
        if cents < 0 { "-" } else { "" },
        cents.abs() / 100,
        cents.abs() % 100
    ))
}
fn text(value: &mut String, max: usize) -> Result<(), CoreError> {
    if value.chars().count() > max
        || value
            .chars()
            .any(|c| c.is_control() && c != '\n' && c != '\r' && c != '\t')
    {
        return Err(CoreError::Invalid);
    }
    *value = value.trim().into();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn input() -> OrderInput {
        serde_json::from_value(serde_json::json!({"n_orders":" OC-1 ","n_bill":"","order_date":"2026-10-23","received_date":null,"client_name":"Cliente","client_rtn":"","id_destination":1,"comments":"","products":[{"description":"Equipo","cantidad":2,"sale_price":"60","purchase_price":"100","tracking":"","isv":true,"status":"Pendiente","coments":""}],"expenses":[{"description":"Flete","amount":"5.005"}]})).unwrap()
    }
    #[test]
    fn legacy_price_direction_expenses_tax_and_derived_states() {
        let mut value = input();
        let c = calculate(&value).unwrap();
        assert_eq!(c.totals.subtotal, "200.00");
        assert_eq!(c.totals.tax, "30.00");
        assert_eq!(c.totals.total, "230.00");
        assert_eq!(c.totals.investment_total, "125.01");
        assert_eq!(c.totals.profit, "74.99");
        assert_eq!(c.status, "Pendiente");
        value.products[0].status = ProductStatus::Comprado;
        assert_eq!(calculate(&value).unwrap().status, "Listo");
        value.n_bill = " FAC-1 ".into();
        assert_eq!(calculate(&value).unwrap().status, "Entregado");
        value.products.clear();
        value.n_bill.clear();
        assert_eq!(calculate(&value).unwrap().status, "Pendiente");
    }
    #[test]
    fn aggregate_rounding_exempt_lines_and_normalization_are_stable() {
        let mut value = input();
        value.expenses.clear();
        value.products[0].purchase_price = "0.0049".into();
        value.products[0].isv = false;
        let c = calculate(&value).unwrap();
        assert_eq!(c.totals.total, "0.01");
        assert_eq!(c.totals.subtotal_exonerado, "0.01");
        assert_eq!(c.totals.profit, "-119.99");
        assert_eq!(
            serde_json::to_value(calculate(&c.data).unwrap()).unwrap(),
            serde_json::to_value(c).unwrap()
        );
        value.products[0].isv = true;
        value.products[0].cantidad = 1;
        value.products[0].purchase_price = "0.0045".into();
        let c = calculate(&value).unwrap();
        assert_eq!(c.totals.subtotal, "0.00");
        assert_eq!(c.totals.tax, "0.00");
        assert_eq!(c.totals.total, "0.01");
    }
    #[test]
    fn invalid_prices_limits_and_server_fields_are_rejected() {
        for invalid in [
            "-1",
            "1e2",
            "1.",
            ".5",
            "NaN",
            " 1",
            "1.00001",
            "1000000000000",
        ] {
            let mut v = input();
            v.products[0].sale_price = invalid.into();
            assert!(calculate(&v).is_err(), "{invalid}");
        }
        let mut value = input();
        value.products[0].cantidad = 1000001;
        assert!(calculate(&value).is_err());
        value = input();
        value.products[0].purchase_price = "999999999999".into();
        assert!(calculate(&value).is_err());
        value = input();
        value.id_destination = None;
        assert!(calculate(&value).unwrap().ensure_saveable().is_err());
        let mut json = serde_json::to_value(input()).unwrap();
        json["total"] = serde_json::json!("1");
        assert!(serde_json::from_value::<OrderInput>(json).is_err());
        assert!(
            crate::OrderFilter {
                min_total: Some("10".into()),
                max_total: Some("1".into()),
                ..Default::default()
            }
            .bounds()
            .is_err()
        );
    }
}
