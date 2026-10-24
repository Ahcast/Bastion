use bastion_cotizaciones::{QuoteDraft, QuoteInput, QuoteStatus, calculate};
use bastion_ordenes_compra::{OrderInput, ProductStatus};
use bastion_workflows::seed_quote_order;
use serde_json::json;
fn date() -> chrono::NaiveDate {
    "2026-10-23".parse().unwrap()
}
pub fn source() -> QuoteDraft {
    let input: QuoteInput = serde_json::from_value(json!({
        "client_name":"Cliente sintético", "client_rtn":"08019000000000", "order_number":"OC-PRUEBA",
        "quote_date":"2026-10-22", "valid_days":30,"status":"Aprobada",
        "notes":"Notas privadas", "contact_email":"interno@example.test", "prepared_by_name":"Contacto interno", "terms":"Términos aprobados",
        "discount_type":"percent","discount_value":"5",
        "items":[
            {"description":"ISV 15","quantity":"1.25","unit_price":"100","discount_type":"amount","discount_value":"2","tax_type":"isv15"},
            {"description":"ISV 18","quantity":"2.5","unit_price":"50","discount_type":"percent","discount_value":"10","tax_type":"isv18"},
            {"description":"Exento","quantity":"0.5","unit_price":"25","discount_type":"amount","discount_value":"0","tax_type":"none"}
        ]
    })).unwrap();
    let calculation = calculate(&input).unwrap();
    QuoteDraft {
        id: 7,
        version: 4,
        status: "Aprobada".into(),
        currency: "HNL".into(),
        quote_number: Some("COT-007".into()),
        data: calculation.data.clone(),
        calculation,
    }
}
#[test]
fn preserves_all_commercial_amounts_and_source_without_database_mutations() {
    let source = source();
    let original = serde_json::to_value(&source).unwrap();
    let seed = seed_quote_order(&source, date()).unwrap();
    assert_eq!(seed.source.version, 4);
    assert_eq!(seed.source.id, 7);
    assert_eq!(seed.data.products[0].cantidad, "1.25");
    assert_eq!(seed.data.products[1].cantidad, "2.50");
    assert_eq!(
        serde_json::to_value(&seed.calculation.totals.commercial).unwrap(),
        serde_json::to_value(&source.calculation.totals).unwrap()
    );
    assert_eq!(
        seed.calculation.totals.total,
        source.calculation.totals.total
    );

    let approved = seed.data.approved_quote.as_ref().unwrap();
    assert_eq!(approved.terms, source.data.terms);
    assert_eq!(
        serde_json::to_value(&approved.items).unwrap(),
        serde_json::to_value(&source.data.items).unwrap()
    );
    assert_eq!(approved.discount_value, source.data.discount_value);
    assert!(approved.notes.is_empty());
    assert!(approved.contact_email.is_empty());
    assert!(approved.prepared_by_name.is_empty());
    assert!(approved.client_email.is_empty());
    assert!(approved.id_client.is_none());
    assert_eq!(serde_json::to_value(&source).unwrap(), original);
    assert!(seed.data.id_destination.is_none());
    assert!(seed.calculation.ensure_saveable().is_err());
}
#[test]
fn costs_expenses_tracking_and_delivery_do_not_change_approved_sale() {
    let source = source();
    let mut data = seed_quote_order(&source, date()).unwrap().data;
    for row in &mut data.products {
        row.sale_price = "0".into();
        row.status = ProductStatus::Comprado;
        row.tracking = "Operación".into();
    }
    data.expenses.push(bastion_ordenes_compra::OrderExpense {
        description: "Flete".into(),
        amount: "5.005".into(),
    });
    let calculation = bastion_ordenes_compra::calculate(&data).unwrap();
    assert_eq!(calculation.status, "Listo");
    assert_eq!(calculation.totals.total, source.calculation.totals.total);
    assert_eq!(calculation.totals.investment_total, "5.01");
    assert_eq!(calculation.totals.products_investment, "0.00");
    data.n_bill = "FAC-OPERACION".into();
    assert_eq!(
        bastion_ordenes_compra::calculate(&data).unwrap().status,
        "Entregado"
    );
}
#[test]
fn commercial_fields_cannot_be_changed_through_operational_products() {
    let source = source();
    for field in ["quantity", "price", "description", "client", "remove"] {
        let mut data = seed_quote_order(&source, date()).unwrap().data;
        match field {
            "quantity" => data.products[0].cantidad = "1".into(),
            "price" => data.products[0].purchase_price = "1".into(),
            "description" => data.products[0].description = "Otra".into(),
            "client" => data.client_name = "Otro".into(),
            _ => {
                data.products.pop();
            }
        }
        assert!(bastion_ordenes_compra::calculate(&data).is_err(), "{field}");
    }
}
#[test]
fn preserves_exemption_references_and_zero_tax() {
    let mut source = source();
    source.data.purchase_order_exempt_number = "EX-PRUEBA".into();
    source.calculation = calculate(&source.data).unwrap();
    let seed = seed_quote_order(&source, date()).unwrap();
    assert_eq!(seed.calculation.totals.tax, "0.00");
    assert_eq!(
        seed.calculation.totals.total,
        source.calculation.totals.total
    );
    assert_eq!(
        seed.data
            .approved_quote
            .unwrap()
            .purchase_order_exempt_number,
        "EX-PRUEBA"
    );
}
#[test]
fn requires_approved_status_and_consistent_stored_total() {
    let mut source = source();
    source.data.status = QuoteStatus::Borrador;
    assert!(seed_quote_order(&source, date()).is_err());
    source.data.status = QuoteStatus::Aprobada;
    source.calculation.totals.total = "1.00".into();
    assert!(seed_quote_order(&source, date()).is_err());
}
#[test]
fn long_descriptions_survive_and_long_number_requires_manual_completion() {
    let mut source = source();
    source.data.order_number = "X".repeat(100);
    source.data.items[0].description = "á".repeat(3000);
    source.calculation = calculate(&source.data).unwrap();
    let seed = seed_quote_order(&source, date()).unwrap();
    assert!(seed.data.n_orders.is_empty());
    assert_eq!(seed.data.products[0].description.chars().count(), 3000);
    assert_eq!(
        seed.data
            .approved_quote
            .unwrap()
            .order_number
            .chars()
            .count(),
        100
    );
}
#[test]
fn legacy_documents_keep_their_integer_quantity_and_calculation_rules() {
    let input: OrderInput = serde_json::from_value(json!({"n_orders":"OC","n_bill":"","order_date":"2026-10-23","received_date":null,"client_name":"Cliente","client_rtn":"","id_destination":1,"comments":"","products":[{"description":"Producto","cantidad":2,"sale_price":"50","purchase_price":"100","tracking":"","isv":true,"status":"Pendiente","coments":""}],"expenses":[]})).unwrap();
    let mut calculation = bastion_ordenes_compra::calculate(&input).unwrap();
    assert_eq!(calculation.totals.total, "230.00");
    assert!(calculation.totals.commercial.is_none());
    calculation.data.products[0].cantidad = "1.25".into();
    assert!(bastion_ordenes_compra::calculate(&calculation.data).is_err());
}
