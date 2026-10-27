use base64::{Engine, engine::general_purpose::STANDARD};
use bastion_core::{CoreError, CoreService, Database};
use bastion_cotizaciones::QuoteService;
use bastion_document_flows::ConversionService;
use bastion_facturas::InvoiceService;
use bastion_ordenes_compra::{OrderDocumentInput, OrderInput, OrderService};
use sqlx::mysql::MySqlPoolOptions;
fn input() -> OrderInput {
    serde_json::from_value(serde_json::json!({"n_orders":"OC-ACCIONES","n_bill":"REF-ANTERIOR","order_date":"2026-10-23","received_date":"2026-10-24","client_name":"Cliente","client_rtn":"RTN-1","id_destination":1,"comments":"Privado","products":[{"description":"Equipo","cantidad":2,"sale_price":"60","purchase_price":"100","tracking":"tracking","isv":true,"status":"Comprado","coments":"Comentario"}],"expenses":[{"description":"Flete","amount":"5"}]})).unwrap()
}
fn file(name: &str, bytes: &[u8]) -> OrderDocumentInput {
    OrderDocumentInput {
        filename: name.into(),
        content_base64: STANDARD.encode(bytes),
    }
}
#[tokio::test]
async fn mysql57_order_actions_private_files_clone_and_invoice_are_atomic() {
    let Ok(url) = std::env::var("BASTION_ORDER_ACTIONS_TEST_URL") else {
        eprintln!("Requires isolated order actions DB and synthetic storage; CI supplies them.");
        return;
    };
    assert!(
        std::env::var("BASTION_AWS_CLI")
            .unwrap()
            .ends_with("/tests/fixtures/private_storage.py")
    );
    let pool = MySqlPoolOptions::new()
        .max_connections(12)
        .connect(&url)
        .await
        .unwrap();
    let db = Database::from_test_pool(pool.clone());
    db.migrate().await.unwrap();
    QuoteService::migrate(&db).await.unwrap();
    OrderService::migrate(&db).await.unwrap();
    InvoiceService::migrate(&db).await.unwrap();
    ConversionService::migrate(&db).await.unwrap();
    sqlx::query("INSERT INTO persons(id,full_name) VALUES(1,'Admin'),(2,'Editor')")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO roles(id,name) VALUES(2,'Editor')")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO users(id,person_id,role_id,email,status) VALUES(1,1,1,'admin@example.test','active'),(2,2,2,'editor@example.test','active')").execute(&pool).await.unwrap();
    for cap in ["ver", "editar"] {
        sqlx::query("INSERT INTO role_permissions(role_id,module_id,scope_hash,scope_key,capability) VALUES(2,'ordenes_compra',UNHEX(SHA2('documentos',256)),'documentos',?)").bind(cap).execute(&pool).await.unwrap();
    }
    let core = CoreService::new(db.clone(), "x".repeat(32), [1; 32]).unwrap();
    let orders = OrderService::new(db.clone(), core.admin());
    let invoices = InvoiceService::new(db.clone(), core.admin());
    let flows = ConversionService::new(&core);
    orders.synchronize_release(true).await.unwrap();
    invoices.synchronize_release(true).await.unwrap();
    for module in ["ordenes_compra", "facturas"] {
        core.admin()
            .set_module_enabled(1, module, true)
            .await
            .unwrap();
    }
    sqlx::query(
        "INSERT INTO ordenes_destinations(id,destination,name_key) VALUES(1,'Oficina','oficina')",
    )
    .execute(&pool)
    .await
    .unwrap();
    let order = orders
        .create(1, "order-actions-source-01", input())
        .await
        .unwrap();
    assert_eq!(order.status, "Entregado");
    assert_eq!(order.data.products[0].line_id.len(), 48);
    let line = &order.data.products[0].line_id;
    let first = orders
        .upload_document(
            2,
            order.id,
            line,
            1,
            "order-actions-upload-01",
            file("factura.pdf", b"%PDF-private"),
        )
        .await
        .unwrap();
    assert_eq!(first.order_version, 2);
    let replay = orders
        .upload_document(
            2,
            order.id,
            line,
            1,
            "order-actions-upload-01",
            file("factura.pdf", b"%PDF-private"),
        )
        .await
        .unwrap();
    assert_eq!(first.attachment.id, replay.attachment.id);
    assert!(
        matches!(
            orders
                .upload_document(
                    1,
                    order.id,
                    line,
                    2,
                    "order-actions-upload-01",
                    file("other", b"other")
                )
                .await,
            Ok(_)
        ),
        "keys are scoped to actor"
    );
    // Admin substitution incremented the order version and queued the original object.
    assert!(matches!(
        orders
            .upload_document(
                2,
                order.id,
                line,
                3,
                "order-actions-replace-01",
                file("replacement", b"replacement")
            )
            .await,
        Err(CoreError::Forbidden)
    ));
    let current = orders.detail(1, order.id).await.unwrap();
    assert_eq!(current.version, 3);
    assert!(matches!(
        orders
            .remove_document(2, order.id, &first.attachment.id, 3)
            .await,
        Err(CoreError::Forbidden)
    ));
    let mut removed = current.data.clone();
    removed.products.clear();
    assert!(matches!(
        orders.replace(1, order.id, 3, removed).await,
        Err(CoreError::Conflict)
    ));
    let clone = orders
        .clone_order(1, order.id, 3, "order-actions-clone-01")
        .await
        .unwrap();
    assert_eq!(clone.data.order_date, current.data.order_date);
    assert_eq!(clone.data.expenses[0].amount, "5.00");
    assert!(clone.data.n_orders.contains("-COPIA-"));
    assert!(clone.data.n_orders.chars().count() <= 50);
    assert_eq!(clone.status, "Pendiente");
    assert_eq!(clone.data.n_bill, "");
    assert!(clone.data.received_date.is_none());
    assert_ne!(clone.data.products[0].line_id, *line);
    assert!(clone.source_quote.is_none());
    assert!(
        orders
            .documents(1, clone.id)
            .await
            .unwrap()
            .items
            .is_empty()
    );
    assert_eq!(
        orders
            .clone_order(1, order.id, 3, "order-actions-clone-01")
            .await
            .unwrap()
            .id,
        clone.id
    );
    assert!(matches!(
        orders
            .clone_order(1, order.id, 2, "order-actions-stale-01")
            .await,
        Err(CoreError::PreconditionFailed)
    ));
    assert!(matches!(
        orders
            .clone_order(1, clone.id, 1, "order-actions-clone-01")
            .await,
        Err(CoreError::Conflict)
    ));
    let docs = orders.documents(1, order.id).await.unwrap();
    let doc = &docs.items[0];
    let content = orders.document_content(2, order.id, &doc.id).await.unwrap();
    assert_eq!(STANDARD.decode(content.content_base64).unwrap(), b"other");
    assert_eq!(content.mime, "application/octet-stream");
    assert!(matches!(
        orders.document_content(1, clone.id, &doc.id).await,
        Err(CoreError::NotFound)
    ));
    assert!(orders.clean_one_document().await.unwrap());
    assert!(matches!(
        orders
            .document_content(1, order.id, &first.attachment.id)
            .await,
        Err(CoreError::NotFound)
    ));
    assert!(matches!(
        flows
            .order_invoice(2, order.id, 3, "order-actions-invoice-01")
            .await,
        Err(CoreError::Forbidden)
    ));
    let (a, b) = tokio::join!(
        flows.order_invoice(1, order.id, 3, "order-actions-invoice-01"),
        flows.order_invoice(1, order.id, 3, "order-actions-invoice-02")
    );
    let a = a.unwrap();
    let b = b.unwrap();
    assert_eq!(a.target_id, b.target_id);
    assert_ne!(a.reused, b.reused);
    let invoice = invoices.detail(1, a.target_id).await.unwrap();
    assert_eq!(invoice.source_order.as_ref().unwrap().id, order.id);
    assert_eq!(invoice.source_order.as_ref().unwrap().version, 3);
    assert!(invoice.invoice_number.is_none());
    assert_eq!(
        invoice.calculation.totals.total,
        current.calculation.totals.total
    );
    assert_eq!(
        invoice.calculation.investment_total,
        current.calculation.totals.investment_total
    );
    assert_eq!(invoice.data.items[0].cost_price, "60.00");
    assert_eq!(invoice.data.credit_days, 30);
    assert!(invoice.data.notes.is_empty());
    assert_eq!(invoice.data.expenses.len(), 1);
    let updated = invoices
        .replace(1, invoice.id, 1, invoice.data.clone())
        .await
        .unwrap();
    assert_eq!(updated.source_order.unwrap().id, order.id);
    assert_eq!(
        orders.detail(1, order.id).await.unwrap().data.n_bill,
        "REF-ANTERIOR"
    );
    assert!(matches!(
        flows
            .order_invoice(1, clone.id, 1, "order-actions-invoice-01")
            .await,
        Err(CoreError::Conflict)
    ));
    let latest = orders
        .remove_document(1, order.id, &doc.id, 3)
        .await
        .unwrap();
    assert_eq!(latest, 4);
    assert!(orders.clean_one_document().await.unwrap());
    let mut changed = orders.detail(1, order.id).await.unwrap().data;
    changed.products[0].purchase_price = "100.1234".into();
    let changed = orders.replace(1, order.id, 4, changed).await.unwrap();
    let reused = flows
        .order_invoice(1, order.id, 5, "order-actions-existing-01")
        .await
        .unwrap();
    assert_eq!(reused.target_id, a.target_id);
    assert!(reused.reused);
    let precision = orders
        .clone_order(1, order.id, changed.version, "order-actions-precision-01")
        .await
        .unwrap();
    let before: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM facturas_drafts")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(matches!(
        flows
            .order_invoice(1, precision.id, 1, "order-actions-precision-02")
            .await,
        Err(CoreError::Invalid)
    ));
    assert_eq!(
        before,
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM facturas_drafts")
            .fetch_one(&pool)
            .await
            .unwrap()
    );
    core.admin()
        .set_module_enabled(1, "facturas", false)
        .await
        .unwrap();
    assert!(matches!(
        flows
            .order_invoice(1, order.id, 5, "order-actions-disabled-01")
            .await,
        Err(CoreError::NotFound)
    ));
}
