use bastion_clientes::{ClientInput, ClientService};
use bastion_core::{CoreError, CoreService, Database};
use bastion_cotizaciones::QuoteService;
use bastion_document_flows::ConversionService;
use bastion_facturas::{InvoiceInput, InvoiceService};
use bastion_licitaciones::{
    AwardInput, AwardSelection, GlobalVariable, GlobalVariablesInput, LotInput, LotItem,
    TenderService,
};
use bastion_ordenes_compra::OrderService;
use sqlx::mysql::MySqlPoolOptions;

fn selected(quantity: &str) -> AwardInput {
    AwardInput {
        items: vec![AwardSelection {
            lot_number: 1,
            item_number: 1,
            quantity: quantity.into(),
        }],
    }
}

fn invoice(client: u64, quantity: &str) -> InvoiceInput {
    serde_json::from_value(serde_json::json!({
        "id_client":client,"client_name":"Untrusted client name","order_number":"OC-SYNTHETIC",
        "invoice_date":"2026-11-06","payment_term_type":"credito","credit_days":30,
        "discount_type":"amount","discount_value":"0","notes":"","public_notes":"",
        "items":[{"description":"Reviewed item","quantity":quantity,"unit_price":"40",
          "cost_price":"0","discount_type":"amount","discount_value":"0","tax_type":"isv15"}],
        "expenses":[]
    }))
    .unwrap()
}

#[tokio::test]
async fn mysql57_partial_awards_and_reviewed_invoices_preserve_permissions_and_uniqueness() {
    let Ok(url) = std::env::var("BASTION_TENDER_AWARDS_TEST_URL") else {
        eprintln!("Requires isolated BASTION_TENDER_AWARDS_TEST_URL; CI supplies it.");
        return;
    };
    let pool = MySqlPoolOptions::new()
        .max_connections(12)
        .connect(&url)
        .await
        .unwrap();
    let db = Database::from_test_pool(pool.clone());
    db.migrate().await.unwrap();
    ClientService::migrate(&db).await.unwrap();
    QuoteService::migrate(&db).await.unwrap();
    OrderService::migrate(&db).await.unwrap();
    InvoiceService::migrate(&db).await.unwrap();
    TenderService::migrate(&db).await.unwrap();
    ConversionService::migrate(&db).await.unwrap();
    TenderService::migrate(&db).await.unwrap();
    InvoiceService::migrate(&db).await.unwrap();
    ConversionService::migrate(&db).await.unwrap();
    sqlx::query("INSERT INTO persons(id,full_name) VALUES(1,'Admin'),(2,'Reader')")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO roles(id,name) VALUES(2,'Reader')")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO users(id,person_id,role_id,email,status) VALUES(1,1,1,'admin@example.test','active'),(2,2,2,'reader@example.test','active')").execute(&pool).await.unwrap();
    let core = CoreService::new(db.clone(), "x".repeat(32), [1; 32]).unwrap();
    let clients = ClientService::new(db.clone(), core.admin());
    let tenders = TenderService::new(db.clone(), core.admin());
    let invoices = InvoiceService::new(db.clone(), core.admin());
    let flows = ConversionService::new(&core);
    clients.synchronize_release(true).await.unwrap();
    tenders.synchronize_release(true).await.unwrap();
    invoices.synchronize_release(true).await.unwrap();
    for module in ["clientes", "licitaciones", "facturas"] {
        core.admin()
            .set_module_enabled(1, module, true)
            .await
            .unwrap();
    }
    let client = clients
        .create(
            1,
            "award-client-key-00001",
            ClientInput {
                cliente_name: "Catalog client".into(),
                rtn: "08011900000000".into(),
                ..Default::default()
            },
        )
        .await
        .unwrap()
        .client;
    let exp = tenders.create(1, "award-expedient-00001").await.unwrap();
    assert!(matches!(
        tenders
            .register_award(
                1,
                exp.id,
                exp.version,
                "award-before-sent-01",
                selected("1")
            )
            .await,
        Err(CoreError::Conflict)
    ));
    let exp = tenders
        .save_global_variables(
            1,
            exp.id,
            exp.version,
            GlobalVariablesInput {
                variables: vec![
                    GlobalVariable {
                        key: "lotes".into(),
                        label: "Lotes".into(),
                        kind: bastion_colaboracion::VariableKind::Integer,
                        value: Some(serde_json::json!("1")),
                    },
                    GlobalVariable {
                        key: "stylo".into(),
                        label: "Estilo".into(),
                        kind: bastion_colaboracion::VariableKind::Text,
                        value: Some(serde_json::json!("std")),
                    },
                ],
            },
            false,
        )
        .await
        .unwrap();
    let exp = tenders
        .save_lot(
            1,
            exp.id,
            1,
            exp.version,
            LotInput {
                items: vec![LotItem {
                    descripcion: "Offered item".into(),
                    pais_origen: String::new(),
                    fecha_entrega: "30 días".into(),
                    cantidad: "5".into(),
                    unidad: "und".into(),
                    transporte: "0.00".into(),
                    precio_unitario: "100.00".into(),
                }],
            },
        )
        .await
        .unwrap();
    let exp = tenders.mark_sent(1, exp.id, exp.version).await.unwrap();
    let (first, retry) = tokio::join!(
        tenders.register_award(
            1,
            exp.id,
            exp.version,
            "award-shared-key-0001",
            selected("2")
        ),
        tenders.register_award(
            1,
            exp.id,
            exp.version,
            "award-shared-key-0001",
            selected("2")
        ),
    );
    let award = first.unwrap();
    assert_eq!(award.id, retry.unwrap().id);
    assert_eq!(
        tenders
            .available_award_lot(1, exp.id, 1)
            .await
            .unwrap()
            .items[0]
            .remaining_quantity,
        "3"
    );
    assert!(matches!(
        tenders
            .register_award(
                1,
                exp.id,
                exp.version,
                "award-excess-key-0001",
                selected("4")
            )
            .await,
        Err(CoreError::Conflict)
    ));
    assert!(matches!(
        tenders
            .register_award(
                2,
                exp.id,
                exp.version,
                "award-denied-key-0001",
                selected("1")
            )
            .await,
        Err(CoreError::Forbidden)
    ));
    assert!(matches!(
        flows
            .tender_invoice(
                1,
                exp.id,
                award.id,
                2,
                "invoice-wrong-version",
                invoice(client.id, "2")
            )
            .await,
        Err(CoreError::PreconditionFailed)
    ));
    assert!(matches!(
        flows
            .tender_invoice(
                1,
                exp.id,
                award.id,
                1,
                "invoice-wrong-qty-01",
                invoice(client.id, "1")
            )
            .await,
        Err(CoreError::Invalid)
    ));
    let (one, two) = tokio::join!(
        flows.tender_invoice(
            1,
            exp.id,
            award.id,
            1,
            "invoice-concurrent-1",
            invoice(client.id, "2")
        ),
        flows.tender_invoice(
            1,
            exp.id,
            award.id,
            1,
            "invoice-concurrent-2",
            invoice(client.id, "2")
        ),
    );
    let one = one.unwrap();
    let two = two.unwrap();
    assert_eq!(one.target_id, two.target_id);
    assert_ne!(one.reused, two.reused);
    let target = invoices.detail(1, one.target_id).await.unwrap();
    assert_eq!(target.status, "Borrador");
    assert!(target.invoice_number.is_none() && target.payments.is_empty());
    assert_eq!(target.source_tender.as_ref().unwrap().award_id, award.id);
    assert_eq!(target.data.client_name, "Catalog client");
    assert_eq!(target.calculation.totals.total, "92.00");
    let mut changed = invoice(client.id, "2");
    changed.items[0].unit_price = "50".into();
    assert!(matches!(
        flows
            .tender_invoice(1, exp.id, award.id, 1, "invoice-concurrent-1", changed)
            .await,
        Err(CoreError::Conflict)
    ));
    for (module, scope) in [("licitaciones", "expedientes"), ("facturas", "documentos")] {
        sqlx::query("INSERT INTO role_permissions(role_id,module_id,scope_hash,scope_key,capability) VALUES(2,?,UNHEX(SHA2(?,256)),?,'ver')")
            .bind(module).bind(scope).bind(scope).execute(&pool).await.unwrap();
    }
    assert_eq!(
        flows
            .tender_invoice_link(2, exp.id, award.id)
            .await
            .unwrap()
            .unwrap()
            .target_id,
        one.target_id
    );
    assert!(matches!(
        flows
            .tender_invoice(
                2,
                exp.id,
                award.id,
                1,
                "invoice-reader-key-01",
                invoice(client.id, "2")
            )
            .await,
        Err(CoreError::Forbidden)
    ));
    let second = tenders
        .register_award(
            1,
            exp.id,
            exp.version,
            "award-second-key-001",
            selected("1"),
        )
        .await
        .unwrap();
    let mut missing_client = invoice(client.id, "1");
    missing_client.id_client = None;
    assert!(matches!(
        flows
            .tender_invoice(
                1,
                exp.id,
                second.id,
                1,
                "invoice-no-client-01",
                missing_client
            )
            .await,
        Err(CoreError::Invalid)
    ));
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM facturas_drafts")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
    core.admin()
        .set_module_enabled(1, "facturas", false)
        .await
        .unwrap();
    assert!(matches!(
        flows
            .tender_invoice(
                1,
                exp.id,
                second.id,
                1,
                "invoice-disabled-01",
                invoice(client.id, "1")
            )
            .await,
        Err(CoreError::NotFound)
    ));
}
