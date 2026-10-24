use super::*;

/// Reuse the quotation fiscal calculation; only operational costs/gastos vary.
pub(super) fn commercial(mut data: OrderInput) -> Result<OrderCalculation, CoreError> {
    let snapshot = data.approved_quote.as_ref().ok_or(CoreError::Invalid)?;
    let quote = bastion_cotizaciones::calculate(snapshot)?;
    if data.products.len() != quote.data.items.len()
        || data.client_name != quote.data.client_name
        || data.client_rtn != quote.data.client_rtn.trim()
    {
        return Err(CoreError::Conflict);
    }
    let mut investment = 0_i128;
    for (product, approved) in data.products.iter_mut().zip(&quote.data.items) {
        if product.description.trim() != approved.description
            || decimal(&product.cantidad)? != decimal(&approved.quantity)?
            || decimal(&product.purchase_price)? != decimal(&approved.unit_price)?
        {
            return Err(CoreError::Conflict);
        }
        text(&mut product.tracking, 255)?;
        text(&mut product.coments, 1000)?;
        let cost = decimal(&product.sale_price)?;
        investment += cost * decimal(&approved.quantity)?;
        product.sale_price = unit(cost);
        product.purchase_price = approved.unit_price.clone();
        product.cantidad = approved.quantity.clone();
        product.description = approved.description.clone();
        product.isv = matches!(approved.tax_type, bastion_cotizaciones::TaxType::Isv15);
    }
    let mut expenses = 0_i128;
    for expense in &mut data.expenses {
        text(&mut expense.description, 255)?;
        let cents = round(decimal(&expense.amount)?, 100);
        expense.amount = money(cents)?;
        expenses += cents;
    }
    data.expenses.retain(|e| e.amount != "0.00");
    let investment = round(investment, SCALE * 100);
    let total = decimal(&quote.totals.total)? / 100;
    let tax = (decimal(&quote.totals.isv_15)? + decimal(&quote.totals.isv_18)?) / 100;
    let exempt =
        (decimal(&quote.totals.importe_exonerado)? + decimal(&quote.totals.importe_exento)?) / 100;
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
        totals: OrderTotals {
            subtotal: quote.totals.subtotal.clone(),
            subtotal_exonerado: money(exempt)?,
            tax: money(tax)?,
            total: quote.totals.total.clone(),
            products_investment: money(investment)?,
            other_expenses: money(expenses)?,
            investment_total: money(investment + expenses)?,
            profit: money(total - investment - expenses)?,
            commercial: Some(quote.totals),
        },
        data,
    })
}
