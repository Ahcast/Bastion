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
            source_quote: Some(origin),
            data: calculation.data.clone(),
            calculation,
        })
    }
}
