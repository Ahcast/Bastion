use bastion_core::CoreError;
use bastion_cotizaciones::{CalculatedItem, QuoteInput, QuoteItem, QuoteStatus, QuoteTotals};
use serde::{Deserialize, Serialize};

use crate::{InvoiceInput, PaymentTerm};

#[derive(Debug, Deserialize, Serialize)]
pub struct InvoiceCalculation {
    pub data: InvoiceInput,
    pub items: Vec<CalculatedItem>,
    pub totals: QuoteTotals,
    pub product_investment: String,
    pub other_expenses: String,
    pub investment_total: String,
    pub profit: String,
    pub paid_total: String,
    pub balance: String,
}

pub fn calculate(input: &InvoiceInput) -> Result<InvoiceCalculation, CoreError> {
    if input.items.len() > 200
        || input.expenses.len() > 200
        || input.credit_days > 3650
        || input.public_notes.chars().count() > 3000
    {
        return Err(CoreError::Invalid);
    }
    let mut data = input.clone();
    data.public_notes = data.public_notes.trim().into();
    if matches!(data.payment_term_type, PaymentTerm::Contado) {
        data.credit_days = 0;
    }
    let mut retained = Vec::new();
    for item in &data.items {
        let mut item = item.clone();
        item.cost_price = money(cents(&item.cost_price)?);
        // Keep cost associated with the original row when the shared
        // calculator discards a completely empty commercial line.
        if !item.description.trim().is_empty()
            || cents(&item.quantity)? != 0
            || cents(&item.unit_price)? != 0
        {
            retained.push(item);
        } else {
            // Validate even discarded rows through the shared calculator below.
            cents(&item.discount_value)?;
        }
    }
    let quote = QuoteInput {
        id_client: data.id_client,
        client_name: data.client_name.clone(),
        client_rtn: data.client_rtn.clone(),
        client_email: data.client_email.clone(),
        order_number: data.order_number.clone(),
        quote_date: data.invoice_date,
        valid_days: 0,
        status: QuoteStatus::Borrador,
        contact_email: String::new(),
        prepared_by_name: String::new(),
        purchase_order_exempt_number: data.purchase_order_exempt_number.clone(),
        registration_certificate_number: data.registration_certificate_number.clone(),
        sag_registration_number: data.sag_registration_number.clone(),
        notes: data.notes.clone(),
        terms: String::new(),
        discount_type: data.discount_type,
        discount_value: data.discount_value.clone(),
        items: input
            .items
            .iter()
            .map(|i| QuoteItem {
                description: i.description.clone(),
                quantity: i.quantity.clone(),
                unit_price: i.unit_price.clone(),
                discount_type: i.discount_type,
                discount_value: i.discount_value.clone(),
                tax_type: i.tax_type,
            })
            .collect(),
    };
    let commercial = bastion_cotizaciones::calculate(&quote)?;
    data.client_name = commercial.data.client_name;
    data.order_number = commercial.data.order_number;
    data.notes = commercial.data.notes;
    data.discount_value = commercial.data.discount_value;
    let mut investment_raw = 0_i128;
    for (item, normalized) in retained.iter_mut().zip(&commercial.data.items) {
        item.description = normalized.description.clone();
        item.quantity = normalized.quantity.clone();
        item.unit_price = normalized.unit_price.clone();
        item.discount_value = normalized.discount_value.clone();
        item.tax_type = normalized.tax_type;
        investment_raw = add(
            investment_raw,
            mul(cents(&item.quantity)?, cents(&item.cost_price)?)?,
        )?;
    }
    data.items = retained;
    let product_investment = rounded(investment_raw, 100)?;
    let mut other_expenses = 0_i128;
    let mut expenses = Vec::new();
    for expense in &data.expenses {
        if expense.description.chars().count() > 1000 {
            return Err(CoreError::Invalid);
        }
        let amount = cents(&expense.amount)?;
        let mut expense = expense.clone();
        expense.description = expense.description.trim().into();
        expense.amount = money(amount);
        if !expense.description.is_empty() || amount > 0 {
            other_expenses = add(other_expenses, amount)?;
            expenses.push(expense);
        }
    }
    data.expenses = expenses;
    let investment = add(product_investment, other_expenses)?;
    let total = cents(&commercial.totals.total)?;
    let tax = add(
        cents(&commercial.totals.isv_15)?,
        cents(&commercial.totals.isv_18)?,
    )?;
    Ok(InvoiceCalculation {
        data,
        items: commercial.items,
        totals: commercial.totals,
        product_investment: money(product_investment),
        other_expenses: money(other_expenses),
        investment_total: money(investment),
        profit: money(total - tax - investment),
        paid_total: "0.00".into(),
        balance: money(total),
    })
}

impl InvoiceCalculation {
    pub(crate) fn ensure_saveable(&self) -> Result<(), CoreError> {
        // No payment command exists in this first cut. Preserve the documented
        // full-payment requirement instead of persisting unpaid cash invoices.
        if matches!(self.data.payment_term_type, PaymentTerm::Contado) && self.balance != "0.00" {
            return Err(CoreError::Invalid);
        }
        Ok(())
    }
}

fn cents(value: &str) -> Result<i128, CoreError> {
    let (whole, fraction) = value.split_once('.').unwrap_or((value, ""));
    if whole.is_empty()
        || whole.len() > 12
        || !whole.bytes().all(|b| b.is_ascii_digit())
        || fraction.len() > 4
        || !fraction.bytes().all(|b| b.is_ascii_digit())
        || (value.contains('.') && fraction.is_empty())
    {
        return Err(CoreError::Invalid);
    }
    let whole: i128 = whole.parse().map_err(|_| CoreError::Invalid)?;
    let fraction: i128 = if fraction.is_empty() {
        0
    } else {
        fraction.parse::<i128>().map_err(|_| CoreError::Invalid)?
            * 10_i128.pow(4 - fraction.len() as u32)
    };
    let result = rounded(whole * 10000 + fraction, 100)?;
    if result > 99_999_999_999_999 {
        return Err(CoreError::Invalid);
    }
    Ok(result)
}
fn rounded(value: i128, divisor: i128) -> Result<i128, CoreError> {
    add(value / divisor, i128::from(value % divisor >= divisor / 2))
}
fn add(a: i128, b: i128) -> Result<i128, CoreError> {
    a.checked_add(b).ok_or(CoreError::Invalid)
}
fn mul(a: i128, b: i128) -> Result<i128, CoreError> {
    a.checked_mul(b).ok_or(CoreError::Invalid)
}
fn money(value: i128) -> String {
    let sign = if value < 0 { "-" } else { "" };
    let value = value.abs();
    format!("{sign}{}.{:02}", value / 100, value % 100)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn input() -> InvoiceInput {
        serde_json::from_value(serde_json::json!({
            "client_name":"Cliente", "order_number":"ORD-1", "invoice_date":"2026-10-22",
            "payment_term_type":"credito", "credit_days":30, "discount_type":"amount", "discount_value":"0",
            "notes":"", "public_notes":"Nota pública", "expenses":[{"description":"Flete", "amount":"5.125"}],
            "items":[{"description":"Equipo", "quantity":"2", "unit_price":"100", "cost_price":"60.125", "discount_type":"percent", "discount_value":"10", "tax_type":"isv15"}]
        })).unwrap()
    }
    #[test]
    fn legacy_item_metadata_is_readable_but_never_serialized() {
        let mut stored = serde_json::to_value(input()).unwrap();
        stored["items"][0]["tracking"] = serde_json::json!("Anterior");
        stored["items"][0]["coments"] = serde_json::json!("Comentario anterior");
        let previous: InvoiceInput = serde_json::from_value(stored).unwrap();
        let result = calculate(&previous).unwrap();
        assert_eq!(result.totals.total, "207.00");
        let serialized = serde_json::to_value(result).unwrap();
        let item = serialized["data"]["items"][0].as_object().unwrap();
        assert!(!item.contains_key("tracking"));
        assert!(!item.contains_key("coments"));
    }
    #[test]
    fn commercial_taxes_costs_expenses_and_profit_are_decimal() {
        let c = calculate(&input()).unwrap();
        assert_eq!(c.totals.total, "207.00");
        assert_eq!(c.totals.isv_15, "27.00");
        assert_eq!(c.product_investment, "120.26");
        assert_eq!(c.other_expenses, "5.13");
        assert_eq!(c.investment_total, "125.39");
        assert_eq!(c.profit, "54.61");
        assert_eq!(c.balance, "207.00");
        assert_eq!(c.paid_total, "0.00");
        assert!(c.ensure_saveable().is_ok());
    }
    #[test]
    fn profit_excludes_both_isv_rates_and_includes_all_investment() {
        let mut i = input();
        i.items[0].quantity = "1".into();
        i.items[0].discount_value = "0".into();
        i.items[0].cost_price = "60".into();
        i.expenses[0].amount = "5".into();
        let mut second = i.items[0].clone();
        second.unit_price = "50".into();
        second.cost_price = "10".into();
        second.tax_type = crate::TaxType::Isv18;
        let mut third = i.items[0].clone();
        third.unit_price = "20".into();
        third.cost_price = "5".into();
        third.tax_type = crate::TaxType::Exempt;
        i.items.extend([second, third]);
        let c = calculate(&i).unwrap();
        assert_eq!(c.totals.total, "194.00");
        assert_eq!(c.totals.isv_15, "15.00");
        assert_eq!(c.totals.isv_18, "9.00");
        assert_eq!(c.investment_total, "80.00");
        assert_eq!(c.profit, "90.00");
        assert_eq!(c.balance, "194.00");
        for tax in [
            crate::TaxType::Exempt,
            crate::TaxType::Isv15,
            crate::TaxType::Isv18,
        ] {
            i.items[0].tax_type = tax;
            assert_eq!(calculate(&i).unwrap().profit, "90.00");
        }
    }
    #[test]
    fn cash_requires_full_payment_and_no_status_or_payment_mass_assignment() {
        let mut i = input();
        i.payment_term_type = PaymentTerm::Contado;
        let c = calculate(&i).unwrap();
        assert_eq!(c.data.credit_days, 0);
        assert!(c.ensure_saveable().is_err());
        i.items.clear();
        assert!(calculate(&i).unwrap().ensure_saveable().is_ok());
        for field in [
            "status",
            "invoice_number",
            "total",
            "payments",
            "paid_total",
            "quote_id",
            "client_request_id",
        ] {
            let mut value = serde_json::to_value(input()).unwrap();
            value[field] = serde_json::json!("untrusted");
            assert!(
                serde_json::from_value::<InvoiceInput>(value).is_err(),
                "{field}"
            );
        }
    }
    #[test]
    fn empty_rows_do_not_shift_costs_and_grouped_investment_rounds_once() {
        let mut i = input();
        let mut blank = i.items[0].clone();
        blank.description.clear();
        blank.quantity = "0".into();
        blank.unit_price = "0".into();
        blank.cost_price = "999".into();
        i.items.insert(0, blank);
        i.items[1].quantity = "0.5".into();
        i.items[1].cost_price = "0.01".into();
        i.items.push(i.items[1].clone());
        let c = calculate(&i).unwrap();
        assert_eq!(c.data.items.len(), 2);
        assert_eq!(c.product_investment, "0.01");
        i.items[1].cost_price = "2000".into();
        assert!(calculate(&i).unwrap().profit.starts_with('-'));
    }
    #[test]
    fn reject_lossy_unbounded_and_invalid_inputs() {
        let mut invalid_client = input();
        invalid_client.id_client = Some(0);
        assert!(calculate(&invalid_client).is_err());
        for value in [
            "NaN",
            "1e3",
            "-1",
            ".1",
            "1.",
            "1,2",
            "1.00001",
            "1000000000000",
        ] {
            let mut i = input();
            i.items[0].cost_price = value.into();
            assert!(calculate(&i).is_err(), "{value}");
        }
        let mut i = input();
        i.items = vec![i.items[0].clone(); 201];
        assert!(calculate(&i).is_err());
        let mut i = input();
        i.expenses = vec![i.expenses[0].clone(); 201];
        assert!(calculate(&i).is_err());
        let mut i = input();
        i.credit_days = 3651;
        assert!(calculate(&i).is_err());
        let mut i = input();
        i.items[0].discount_value = "101".into();
        assert!(calculate(&i).is_err());
        assert!(
            crate::InvoiceFilter {
                page_size: Some(101),
                ..Default::default()
            }
            .bounds()
            .is_err()
        );
    }
}
