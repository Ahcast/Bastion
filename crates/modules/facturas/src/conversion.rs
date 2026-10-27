use super::*;
use crate::{InvoiceItem, PaymentTerm};
use bastion_cotizaciones::{QuoteDraft, QuoteOrigin};
impl InvoiceService {
    pub async fn lock_conversion(
        &self,
        actor: u64,
        tx: &mut Transaction<'_, MySql>,
    ) -> Result<(), CoreError> {
        self.admin
            .authorize(actor, "facturas", "documentos", "ver")
            .await?;
        self.admin
            .authorize(actor, "facturas", "documentos", "editar")
            .await?;
        let enabled:Option<bool>=sqlx::query_scalar("SELECT (included_in_release AND environment_allowed AND enabled) FROM module_registry WHERE module_id='facturas' FOR UPDATE").fetch_optional(&mut **tx).await?;
        if enabled != Some(true) {
            return Err(CoreError::NotFound);
        }
        Ok(())
    }
    pub async fn create_from_quote(
        &self,
        actor: u64,
        source: &QuoteDraft,
        tx: &mut Transaction<'_, MySql>,
    ) -> Result<InvoiceDraft, CoreError> {
        self.lock_conversion(actor, tx).await?;
        let data = &source.data;
        let input = InvoiceInput {
            initial_payment: None,
            id_client: data.id_client,
            client_name: data.client_name.clone(),
            client_rtn: data.client_rtn.clone(),
            client_email: data.client_email.clone(),
            order_number: data.order_number.clone(),
            invoice_date: chrono::Utc::now()
                .with_timezone(&chrono::FixedOffset::west_opt(6 * 3600).ok_or(CoreError::Invalid)?)
                .date_naive(),
            payment_term_type: PaymentTerm::Credito,
            credit_days: 30,
            discount_type: data.discount_type,
            discount_value: data.discount_value.clone(),
            purchase_order_exempt_number: data.purchase_order_exempt_number.clone(),
            registration_certificate_number: data.registration_certificate_number.clone(),
            sag_registration_number: data.sag_registration_number.clone(),
            notes: data.notes.clone(),
            public_notes: String::new(),
            items: data
                .items
                .iter()
                .map(|row| InvoiceItem {
                    description: row.description.clone(),
                    quantity: row.quantity.clone(),
                    unit_price: row.unit_price.clone(),
                    cost_price: "0".into(),
                    discount_type: row.discount_type,
                    discount_value: row.discount_value.clone(),
                    tax_type: row.tax_type,
                    _legacy_tracking: String::new(),
                    _legacy_comments: String::new(),
                })
                .collect(),
            expenses: Vec::new(),
        };
        let calculation = calculate(&input)?;
        calculation.ensure_saveable()?;
        // This trusted snapshot is already resolved by Cotizaciones; never reread a mutable catalog.
        if serde_json::to_value(&calculation.totals).map_err(|_| CoreError::Invalid)?
            != serde_json::to_value(&source.calculation.totals).map_err(|_| CoreError::Invalid)?
        {
            return Err(CoreError::Invalid);
        }
        let origin = QuoteOrigin::from(source);
        let id=sqlx::query("INSERT INTO facturas_drafts (client_name,order_number,invoice_date,data_json,calculation_json,source_quote_json,created_by,updated_by,created_at,updated_at) VALUES (?,?,?,?,?,?,?,?,UTC_TIMESTAMP(6),UTC_TIMESTAMP(6))")
            .bind(&calculation.data.client_name).bind(&calculation.data.order_number).bind(calculation.data.invoice_date).bind(serde_json::to_value(&calculation.data).map_err(|_|CoreError::Invalid)?).bind(serde_json::to_value(&calculation).map_err(|_|CoreError::Invalid)?).bind(serde_json::to_value(&origin).map_err(|_|CoreError::Invalid)?).bind(actor).bind(actor).execute(&mut **tx).await?.last_insert_id();
        CoreAdmin::record_domain_event(tx,actor,"facturas","invoice.draft.create",&id.to_string(),serde_json::json!({"version":1,"source_quote_id":source.id,"source_quote_version":source.version})).await?;
        Ok(InvoiceDraft {
            id,
            version: 1,
            status: "Borrador".into(),
            currency: "HNL".into(),
            invoice_number: None,
            payments: Vec::new(),
            sar_snapshot: None,
            company_snapshot: None,
            source_quote: Some(origin),
            source_order: None,
            data: calculation.data.clone(),
            calculation,
        })
    }
}

impl InvoiceService {
    pub async fn create_from_order(
        &self,
        actor: u64,
        source: &bastion_ordenes_compra::PurchaseOrder,
        tx: &mut Transaction<'_, MySql>,
    ) -> Result<InvoiceDraft, CoreError> {
        self.lock_conversion(actor, tx).await?;
        let c = order_invoice_calculation(source)?;
        let origin = crate::OrderOrigin {
            id: source.id,
            version: source.version,
            number: source.data.n_orders.clone(),
        };
        let id=sqlx::query("INSERT INTO facturas_drafts(client_name,order_number,invoice_date,data_json,calculation_json,source_order_json,created_by,updated_by,created_at,updated_at) VALUES (?,?,?,?,?,?,?,?,UTC_TIMESTAMP(6),UTC_TIMESTAMP(6))")
            .bind(&c.data.client_name).bind(&c.data.order_number).bind(c.data.invoice_date).bind(serde_json::to_value(&c.data).map_err(|_|CoreError::Invalid)?).bind(serde_json::to_value(&c).map_err(|_|CoreError::Invalid)?).bind(serde_json::to_value(&origin).map_err(|_|CoreError::Invalid)?).bind(actor).bind(actor).execute(&mut **tx).await?.last_insert_id();
        CoreAdmin::record_domain_event(tx,actor,"facturas","invoice.draft.create",&id.to_string(),serde_json::json!({"version":1,"source_order_id":source.id,"source_order_version":source.version})).await?;
        Ok(InvoiceDraft {
            id,
            version: 1,
            status: "Borrador".into(),
            currency: "HNL".into(),
            invoice_number: None,
            payments: Vec::new(),
            sar_snapshot: None,
            company_snapshot: None,
            source_quote: None,
            source_order: Some(origin),
            data: c.data.clone(),
            calculation: c,
        })
    }
}
fn order_invoice_calculation(
    source: &bastion_ordenes_compra::PurchaseOrder,
) -> Result<crate::InvoiceCalculation, CoreError> {
    let price = |s: &str| -> Result<String, CoreError> {
        let (whole, fraction) = s.split_once('.').unwrap_or((s, ""));
        if fraction.trim_end_matches('0').len() > 2 {
            return Err(CoreError::Invalid);
        }
        Ok(format!("{whole}.{:0<2}", fraction.trim_end_matches('0')))
    };
    let mut items = Vec::new();
    for row in &source.data.products {
        items.push(InvoiceItem {
            description: row.description.clone(),
            quantity: row.cantidad.to_string(),
            unit_price: price(&row.purchase_price)?,
            cost_price: price(&row.sale_price)?,
            discount_type: crate::DiscountType::Amount,
            discount_value: "0".into(),
            tax_type: if row.isv {
                crate::TaxType::Isv15
            } else {
                crate::TaxType::Exempt
            },
            _legacy_tracking: String::new(),
            _legacy_comments: String::new(),
        });
    }
    let input = InvoiceInput {
        initial_payment: None,
        id_client: None,
        client_name: source.data.client_name.clone(),
        client_rtn: source.data.client_rtn.clone(),
        client_email: String::new(),
        order_number: source.data.n_orders.clone(),
        invoice_date: chrono::Utc::now()
            .with_timezone(&chrono::FixedOffset::west_opt(21600).ok_or(CoreError::Invalid)?)
            .date_naive(),
        payment_term_type: PaymentTerm::Credito,
        credit_days: 30,
        discount_type: crate::DiscountType::Amount,
        discount_value: "0".into(),
        purchase_order_exempt_number: String::new(),
        registration_certificate_number: String::new(),
        sag_registration_number: String::new(),
        notes: String::new(),
        public_notes: String::new(),
        items,
        expenses: source
            .data
            .expenses
            .iter()
            .map(|e| crate::InvoiceExpense {
                description: e.description.clone(),
                amount: e.amount.clone(),
            })
            .collect(),
    };
    let c = calculate(&input)?;
    c.ensure_saveable()?;
    let t = &source.calculation.totals;
    if c.totals.total != t.total
        || c.totals.subtotal != t.subtotal
        || c.totals.isv_15 != t.tax
        || c.product_investment != t.products_investment
        || c.other_expenses != t.other_expenses
        || c.investment_total != t.investment_total
        || c.profit != t.profit
    {
        return Err(CoreError::Invalid);
    }
    Ok(c)
}

#[cfg(test)]
mod order_tests {
    use super::*;
    fn order(price: &str, cost: &str) -> bastion_ordenes_compra::PurchaseOrder {
        let input=serde_json::from_value(serde_json::json!({"n_orders":"OC-1","n_bill":"","order_date":"2026-10-23","received_date":null,"client_name":"Cliente","client_rtn":"RTN","id_destination":1,"comments":"NO-COPIAR","products":[{"description":"Equipo","cantidad":2,"sale_price":cost,"purchase_price":price,"tracking":"NO-COPIAR","isv":true,"status":"Comprado","coments":"NO-COPIAR"}],"expenses":[{"description":"Flete","amount":"5"}]})).unwrap();
        let c = bastion_ordenes_compra::calculate(&input).unwrap();
        serde_json::from_value(serde_json::json!({"id":7,"version":3,"status":c.status,"currency":"HNL","data":c.data,"calculation":c})).unwrap()
    }
    #[test]
    fn order_snapshot_preserves_cost_sales_tax_expenses_and_credit_without_retired_metadata() {
        let source = order("100.0000", "60.0000");
        let c = order_invoice_calculation(&source).unwrap();
        assert_eq!(c.totals.total, "230.00");
        assert_eq!(c.product_investment, "120.00");
        assert_eq!(c.investment_total, "125.00");
        assert_eq!(c.profit, "75.00");
        assert_eq!(c.data.credit_days, 30);
        assert!(c.data.notes.is_empty());
        assert_eq!(c.data.client_rtn, "RTN");
        assert!(!serde_json::to_string(&c).unwrap().contains("NO-COPIAR"));
        assert!(order_invoice_calculation(&order("100.0001", "60")).is_err());
        assert!(order_invoice_calculation(&order("100", "60.0001")).is_err());
        let mut corrupted = source;
        corrupted.calculation.totals.total = "999.00".into();
        assert!(order_invoice_calculation(&corrupted).is_err());
    }
}
