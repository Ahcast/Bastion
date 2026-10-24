use bastion_core::{CoreError, CoreService, Database};
use bastion_cotizaciones::{QuoteDraft, QuoteInput, QuoteService};
use bastion_document_flows::{ConversionService, OrderConversionInput};
use bastion_facturas::InvoiceService;
use bastion_ordenes_compra::{DestinationInput, OrderService, quote_order_input};
use sqlx::mysql::MySqlPoolOptions;

fn quote_input() -> QuoteInput {
    serde_json::from_value(serde_json::json!({"client_name":"Cliente sintético","order_number":"REF-1","quote_date":"2026-10-23","valid_days":30,"notes":"Nota privada","terms":"Términos históricos","discount_type":"amount","discount_value":"0","items":[{"description":"Equipo","quantity":"1","unit_price":"100","discount_type":"amount","discount_value":"0","tax_type":"isv15"}]})).unwrap()
}
fn draft(input: QuoteInput) -> QuoteDraft {
    let calculation = bastion_cotizaciones::calculate(&input).unwrap();
    QuoteDraft {
        id: 1,
        version: 1,
        status: "Borrador".into(),
        currency: "HNL".into(),
        quote_number: Some("COT-000001".into()),
        data: calculation.data.clone(),
        calculation,
    }
}
#[test]
fn order_conversion_never_coerces_quantities_discounts_tax_or_descriptions() {
    assert!(quote_order_input(&draft(quote_input())).is_ok());
    let mut value = quote_input();
    value.items[0].quantity = "1.5".into();
    assert!(
        quote_order_input(&draft(value))
            .unwrap_err()
            .contains("enteras")
    );
    let mut value = quote_input();
    value.items[0].tax_type = bastion_cotizaciones::TaxType::Isv18;
    assert!(
        quote_order_input(&draft(value))
            .unwrap_err()
            .contains("18%")
    );
    let mut value = quote_input();
    value.discount_value = "1".into();
    assert!(
        quote_order_input(&draft(value))
            .unwrap_err()
            .contains("descuento general")
    );
    let mut value = quote_input();
    value.items[0].discount_value = "1".into();
    assert!(
        quote_order_input(&draft(value))
            .unwrap_err()
            .contains("descuentos")
    );
    let mut value = quote_input();
    value.items[0].description = "x".repeat(2001);
    assert!(
        quote_order_input(&draft(value))
            .unwrap_err()
            .contains("truncará")
    );
    let mut value = draft(quote_input());
    value.calculation.totals.total = "115.01".into();
    assert!(quote_order_input(&value).unwrap_err().contains("importes"));
}
#[tokio::test]
async fn mysql57_conversions_are_atomic_authorized_lossless_and_idempotent() {
    let Ok(url) = std::env::var("BASTION_CONVERSIONS_TEST_URL") else {
        eprintln!("Requires isolated conversion database; CI supplies it.");
        return;
    };
    let pool = MySqlPoolOptions::new()
        .max_connections(10)
        .connect(&url)
        .await
        .unwrap();
    let db = Database::from_test_pool(pool.clone());
    db.migrate().await.unwrap();
    QuoteService::migrate(&db).await.unwrap();
    InvoiceService::migrate(&db).await.unwrap();
    OrderService::migrate(&db).await.unwrap();
    ConversionService::migrate(&db).await.unwrap();
    sqlx::query("INSERT INTO persons (id,full_name) VALUES (1,'Conversion administrator'),(2,'Source editor')").execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO roles (id,name) VALUES (2,'Source editor')")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO users (id,person_id,role_id,email,status) VALUES (1,1,1,'admin@example.test','active'),(2,2,2,'source@example.test','active')").execute(&pool).await.unwrap();
    for capability in ["ver", "editar"] {
        sqlx::query("INSERT INTO role_permissions (role_id,module_id,scope_hash,scope_key,capability) VALUES (2,'cotizaciones',UNHEX(SHA2('documentos',256)),'documentos',?)").bind(capability).execute(&pool).await.unwrap();
    }
    let core = CoreService::new(db.clone(), "x".repeat(32), [1; 32]).unwrap();
    let quotes = QuoteService::new(db.clone(), core.admin());
    let invoices = InvoiceService::new(db.clone(), core.admin());
    let orders = OrderService::new(db.clone(), core.admin());
    let flows = ConversionService::new(&core);
    quotes.synchronize_release(true).await.unwrap();
    invoices.synchronize_release(true).await.unwrap();
    orders.synchronize_release(true).await.unwrap();
    for module in ["cotizaciones", "ordenes_compra"] {
        core.admin()
            .set_module_enabled(1, module, true)
            .await
            .unwrap();
    }
    let source = quotes
        .create(1, "source-conversion-create-01", quote_input())
        .await
        .unwrap();
    assert!(matches!(
        flows
            .invoice(1, source.id, 1, "conversion-disabled-01")
            .await,
        Err(CoreError::NotFound)
    ));
    core.admin()
        .set_module_enabled(1, "facturas", true)
        .await
        .unwrap();
    assert!(matches!(
        flows
            .invoice(2, source.id, 1, "conversion-forbidden-01")
            .await,
        Err(CoreError::Forbidden)
    ));
    assert!(matches!(
        flows.invoice(1, source.id, 99, "conversion-stale-01").await,
        Err(CoreError::PreconditionFailed)
    ));
    let invalid = OrderConversionInput {
        n_orders: "ORDER-1".into(),
        id_destination: 999,
    };
    assert!(matches!(
        flows
            .order(1, source.id, 1, "conversion-destination-01", invalid)
            .await,
        Err(CoreError::Invalid)
    ));
    assert_eq!(quotes.detail(1, source.id).await.unwrap().version, 1);
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM document_quote_conversions")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
    let destination = orders
        .save_destination(
            1,
            None,
            None,
            DestinationInput {
                destination: "Destino sintético".into(),
            },
        )
        .await
        .unwrap();
    let (first, parallel) = tokio::join!(
        flows.invoice(1, source.id, 1, "conversion-invoice-01"),
        flows.invoice(1, source.id, 1, "conversion-invoice-02")
    );
    let first = first.unwrap();
    let parallel = parallel.unwrap();
    assert_eq!(first.target_id, parallel.target_id);
    assert_ne!(first.reused, parallel.reused);
    let repeated = flows
        .invoice(1, source.id, 1, "conversion-invoice-01")
        .await
        .unwrap();
    assert_eq!(repeated.target_id, first.target_id);
    let saved = quotes.detail(1, source.id).await.unwrap();
    assert_eq!(saved.status, "Convertido");
    assert_eq!(saved.version, 2);
    let invoice = invoices.detail(1, first.target_id).await.unwrap();
    assert_eq!(invoice.invoice_number, None);
    assert_eq!(invoice.status, "Borrador");
    assert_eq!(invoice.data.credit_days, 30);
    assert!(matches!(
        invoice.data.payment_term_type,
        bastion_facturas::PaymentTerm::Credito
    ));
    assert_eq!(invoice.source_quote.as_ref().unwrap().id, source.id);
    assert_eq!(invoice.data.client_name, source.data.client_name);
    assert_eq!(invoice.data.notes, source.data.notes);
    assert_eq!(
        serde_json::to_value(&invoice.calculation.totals).unwrap(),
        serde_json::to_value(&source.calculation.totals).unwrap()
    );
    let mut modified = invoice.data;
    modified.items[0].cost_price = "30".into();
    let edited = invoices
        .replace(1, first.target_id, 1, modified)
        .await
        .unwrap();
    assert_eq!(edited.source_quote.as_ref().unwrap().version, 1);
    assert_eq!(
        invoices
            .detail(1, first.target_id)
            .await
            .unwrap()
            .source_quote
            .unwrap()
            .number,
        source.quote_number
    );
    let order = flows
        .order(
            1,
            source.id,
            2,
            "conversion-order-01",
            OrderConversionInput {
                n_orders: "ORDER-1".into(),
                id_destination: destination.id,
            },
        )
        .await
        .unwrap();
    let retry = flows
        .order(
            1,
            source.id,
            2,
            "conversion-order-01",
            OrderConversionInput {
                n_orders: "ORDER-1".into(),
                id_destination: destination.id,
            },
        )
        .await
        .unwrap();
    assert_eq!(order.target_id, retry.target_id);
    assert!(matches!(
        flows
            .order(
                1,
                source.id,
                2,
                "conversion-order-01",
                OrderConversionInput {
                    n_orders: "DIFFERENT".into(),
                    id_destination: destination.id
                }
            )
            .await,
        Err(CoreError::Conflict)
    ));
    let created = orders.detail(1, order.target_id).await.unwrap();
    assert_eq!(
        created.calculation.totals.total,
        source.calculation.totals.total
    );
    assert_eq!(created.source_quote.as_ref().unwrap().version, 2);
    let edited = orders
        .replace(1, created.id, 1, created.data)
        .await
        .unwrap();
    assert_eq!(edited.source_quote.unwrap().id, source.id);
    assert!(
        orders
            .detail(1, created.id)
            .await
            .unwrap()
            .source_quote
            .is_some()
    );
    let mut complex = quote_input();
    complex.items[0].quantity = "1.5".into();
    complex.items[0].tax_type = bastion_cotizaciones::TaxType::Isv18;
    complex.items[0].discount_value = "5".into();
    complex.discount_value = "2".into();
    let complex = quotes
        .create(1, "source-conversion-complex", complex)
        .await
        .unwrap();
    assert!(
        flows
            .order_preview(1, complex.id, 1)
            .await
            .unwrap()
            .reason
            .is_some()
    );
    assert!(
        flows
            .order(
                1,
                complex.id,
                1,
                "conversion-unsupported-01",
                OrderConversionInput {
                    n_orders: "ORDER-2".into(),
                    id_destination: destination.id
                }
            )
            .await
            .is_err()
    );
    assert_eq!(quotes.detail(1, complex.id).await.unwrap().version, 1);
    let from_complex = flows
        .invoice(1, complex.id, 1, "conversion-complex-invoice")
        .await
        .unwrap();
    assert_eq!(
        serde_json::to_value(
            invoices
                .detail(1, from_complex.target_id)
                .await
                .unwrap()
                .calculation
                .totals
        )
        .unwrap(),
        serde_json::to_value(&complex.calculation.totals).unwrap()
    );
    assert!(matches!(
        flows
            .invoice(1, complex.id, 1, "conversion-invoice-01")
            .await,
        Err(CoreError::Conflict)
    ));
    core.admin()
        .set_module_enabled(1, "facturas", false)
        .await
        .unwrap();
    assert!(matches!(
        flows
            .invoice(1, source.id, 1, "conversion-invoice-01")
            .await,
        Err(CoreError::NotFound)
    ));
    core.admin()
        .set_module_enabled(1, "facturas", true)
        .await
        .unwrap();
    quotes.delete(1, source.id, 3).await.unwrap();
    assert!(
        invoices
            .detail(1, first.target_id)
            .await
            .unwrap()
            .source_quote
            .is_some()
    );
    let historic: serde_json::Value = sqlx::query_scalar(
        "SELECT snapshot_json FROM document_quote_conversions WHERE invoice_id=?",
    )
    .bind(first.target_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(historic["data"]["terms"], "Términos históricos");
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM facturas_drafts")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 2);
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM ordenes_orders")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
}
