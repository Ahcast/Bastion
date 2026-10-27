use super::*;
use crate::{OrderProduct, ProductStatus};
use bastion_cotizaciones::{QuoteDraft, QuoteOrigin, TaxType};

/// A lossless boundary: never coerce quantities, discounts, tax rates or totals.
pub fn quote_order_input(source: &QuoteDraft) -> Result<OrderInput, &'static str> {
    let data = &source.data;
    if data.client_name.trim().is_empty() {
        return Err("Completa el cliente antes de convertir a una orden.");
    }
    if crate::calculation::decimal(&data.discount_value).map_err(|_| "Descuento inválido.")? != 0 {
        return Err("Órdenes todavía no admite el descuento general de esta cotización.");
    }
    let mut products = Vec::new();
    for row in &data.items {
        if row.description.chars().count() > 2000 {
            return Err("Una descripción excede el límite de Órdenes; no se truncará.");
        }
        let quantity =
            crate::calculation::decimal(&row.quantity).map_err(|_| "Cantidad inválida.")?;
        if quantity % 10000 != 0 || quantity / 10000 > 1000000 {
            return Err("Órdenes requiere cantidades enteras de hasta 1000000; no se redondearán.");
        }
        if crate::calculation::decimal(&row.discount_value).map_err(|_| "Descuento inválido.")? != 0
        {
            return Err(
                "Órdenes todavía no admite los descuentos de los ítems de esta cotización.",
            );
        }
        if matches!(row.tax_type, TaxType::Isv18) {
            return Err("Órdenes todavía no admite ISV del 18%; no se cambiará el impuesto.");
        }
        products.push(OrderProduct {
            line_id: String::new(),
            description: row.description.clone(),
            cantidad: (quantity / 10000) as u32,
            sale_price: "0".into(),
            purchase_price: row.unit_price.clone(),
            tracking: String::new(),
            isv: matches!(row.tax_type, TaxType::Isv15),
            status: ProductStatus::Pendiente,
            coments: String::new(),
        });
    }
    let input = OrderInput {
        n_orders: String::new(),
        n_bill: String::new(),
        order_date: chrono::Utc::now()
            .with_timezone(&chrono::FixedOffset::west_opt(6 * 3600).ok_or("Fecha inválida.")?)
            .date_naive(),
        received_date: None,
        client_name: data.client_name.clone(),
        client_rtn: data.client_rtn.clone(),
        id_destination: None,
        comments: data.notes.clone(),
        products,
        expenses: Vec::new(),
    };
    let value = calculate(&input).map_err(|_| "Los datos exceden los límites de Órdenes.")?;
    if value.totals.total != source.calculation.totals.total
        || value.totals.subtotal != source.calculation.totals.subtotal
        || value.totals.tax != source.calculation.totals.isv_15
    {
        return Err(
            "Las reglas de redondeo de Órdenes cambian los importes; no se realizará la conversión.",
        );
    }
    Ok(input)
}
impl OrderService {
    pub async fn lock_conversion(
        &self,
        actor: u64,
        tx: &mut Transaction<'_, MySql>,
    ) -> Result<(), CoreError> {
        self.admin
            .authorize(actor, "ordenes_compra", "documentos", "ver")
            .await?;
        self.admin
            .authorize(actor, "ordenes_compra", "documentos", "editar")
            .await?;
        let enabled:Option<bool>=sqlx::query_scalar("SELECT (included_in_release AND environment_allowed AND enabled) FROM module_registry WHERE module_id='ordenes_compra' FOR UPDATE").fetch_optional(&mut **tx).await?;
        if enabled != Some(true) {
            return Err(CoreError::NotFound);
        }
        Ok(())
    }
    pub async fn create_from_quote(
        &self,
        actor: u64,
        source: &QuoteDraft,
        n_orders: &str,
        destination: u64,
        tx: &mut Transaction<'_, MySql>,
    ) -> Result<PurchaseOrder, CoreError> {
        self.lock_conversion(actor, tx).await?;
        let mut input = quote_order_input(source).map_err(|_| CoreError::Invalid)?;
        input.n_orders = n_orders.to_owned();
        input.id_destination = Some(destination);
        let mut calculation = calculate(&input)?;
        super::actions::fresh_lines(&mut calculation.data);
        calculation.ensure_saveable()?;
        validate_destination(tx, calculation.data.id_destination).await?;
        let origin = QuoteOrigin::from(source);
        let id=sqlx::query("INSERT INTO ordenes_orders (status,n_orders,n_bill,client_name,order_date,id_destination,total,data_json,calculation_json,source_quote_json,created_by,updated_by,created_at,updated_at) VALUES (?,?,?,?,?,?,?,?,?,?,?,?,UTC_TIMESTAMP(6),UTC_TIMESTAMP(6))")
            .bind(&calculation.status).bind(&calculation.data.n_orders).bind(&calculation.data.n_bill).bind(&calculation.data.client_name).bind(calculation.data.order_date).bind(destination).bind(&calculation.totals.total)
            .bind(serde_json::to_value(&calculation.data).map_err(|_|CoreError::Invalid)?).bind(serde_json::to_value(&calculation).map_err(|_|CoreError::Invalid)?).bind(serde_json::to_value(&origin).map_err(|_|CoreError::Invalid)?).bind(actor).bind(actor).execute(&mut **tx).await?.last_insert_id();
        CoreAdmin::record_domain_event(tx,actor,"ordenes_compra","order.create",&id.to_string(),serde_json::json!({"version":1,"source_quote_id":source.id,"source_quote_version":source.version})).await?;
        let mut result = record(id, 1, calculation);
        result.source_quote = Some(origin);
        Ok(result)
    }
}
