use bastion_core::{CoreError, CoreService, DashboardPeriod, Database};
use bastion_facturas::{InvoiceFilter, InvoiceInput, InvoiceService};
use sqlx::mysql::MySqlPoolOptions;

fn input() -> InvoiceInput {
    serde_json::from_value(serde_json::json!({
        "client_name":"Cliente sintético % _", "order_number":"ORD-1", "invoice_date":"2026-10-22",
        "payment_term_type":"credito", "credit_days":30, "discount_type":"amount", "discount_value":"0",
        "notes":"Privado", "public_notes":"Público", "expenses":[{"description":"Flete", "amount":"5"}],
        "items":[{"description":"Equipo", "quantity":"2", "unit_price":"100", "cost_price":"60", "discount_type":"percent", "discount_value":"10", "tax_type":"isv15"}]
    })).unwrap()
}

#[tokio::test]
async fn mysql57_drafts_atomic_receipts_permissions_versions_and_disable() {
    let Ok(url) = std::env::var("BASTION_FACTURAS_TEST_URL") else {
        eprintln!("Requires isolated BASTION_FACTURAS_TEST_URL; CI supplies it.");
        return;
    };
    let pool = MySqlPoolOptions::new()
        .max_connections(5)
        .connect(&url)
        .await
        .unwrap();
    let database = Database::from_test_pool(pool.clone());
    database.migrate().await.unwrap();
    let core = CoreService::new(database.clone(), "x".repeat(32), [1; 32]).unwrap();
    let service = InvoiceService::new(database.clone(), core.admin());
    service.synchronize_release(true).await.unwrap();
    let allowed: bool = sqlx::query_scalar(
        "SELECT environment_allowed FROM module_registry WHERE module_id='facturas'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(!allowed, "unmigrated modules cannot activate");
    InvoiceService::migrate(&database).await.unwrap();
    InvoiceService::migrate(&database).await.unwrap();
    sqlx::query("INSERT INTO persons (id,full_name) VALUES (1,'Admin test'),(2,'Viewer test')")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO roles (id,name) VALUES (2,'Viewer')")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO users (id,person_id,role_id,email,status) VALUES (1,1,1,'admin@example.test','active'),(2,2,2,'viewer@example.test','active')").execute(&pool).await.unwrap();
    service.synchronize_release(true).await.unwrap();
    assert!(matches!(
        service.list(1, &InvoiceFilter::default()).await,
        Err(CoreError::NotFound)
    ));
    core.admin()
        .set_module_enabled(1, "facturas", true)
        .await
        .unwrap();
    assert!(matches!(
        service.list(2, &InvoiceFilter::default()).await,
        Err(CoreError::Forbidden)
    ));
    sqlx::query("INSERT INTO role_permissions (role_id,module_id,scope_hash,scope_key,capability) VALUES (2,'facturas',UNHEX(SHA2('documentos',256)),'documentos','ver')").execute(&pool).await.unwrap();
    assert!(service.list(2, &InvoiceFilter::default()).await.is_ok());
    assert!(matches!(
        service.create(2, "synthetic-key-0001", input()).await,
        Err(CoreError::Forbidden)
    ));
    let (a, b) = tokio::join!(
        service.create(1, "synthetic-key-0001", input()),
        service.create(1, "synthetic-key-0001", input())
    );
    let a = a.unwrap();
    let b = b.unwrap();
    assert_eq!(a.id, b.id);
    assert_eq!(a.invoice_number, None);
    assert_eq!(a.status, "Borrador");
    assert_eq!(a.calculation.totals.total, "207.00");
    assert_eq!(a.calculation.profit, "55.00");
    assert_eq!(a.calculation.balance, "207.00");
    let dashboard = service.dashboard(1, DashboardPeriod::All).await.unwrap();
    assert_eq!(dashboard["metrics"]["total"], 1);
    assert_eq!(dashboard["metrics"]["amount"], "207.00");
    assert_eq!(dashboard["recent"][0]["status"], "Borrador");
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM facturas_drafts")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
    assert_eq!(
        service.detail(2, a.id).await.unwrap().data.client_name,
        input().client_name
    );
    let mut changed = input();
    changed.notes = "Actualizado".into();
    assert!(matches!(
        service
            .create(1, "synthetic-key-0001", changed.clone())
            .await,
        Err(CoreError::Conflict)
    ));
    let (first, second) = tokio::join!(
        service.replace(1, a.id, 1, changed.clone()),
        service.replace(1, a.id, 1, input())
    );
    assert_eq!(usize::from(first.is_ok()) + usize::from(second.is_ok()), 1);
    let error = if first.is_err() {
        first.unwrap_err()
    } else {
        second.unwrap_err()
    };
    assert!(matches!(error, CoreError::PreconditionFailed));
    let replay = service
        .create(1, "synthetic-key-0001", input())
        .await
        .unwrap();
    assert_eq!(
        replay.version, 1,
        "receipt remains the original response after edits"
    );
    assert_eq!(service.detail(1, a.id).await.unwrap().version, 2);
    for (search, expected) in [("%", 1), ("_", 1), ("' OR 1=1 --", 0)] {
        let page = service
            .list(
                1,
                &InvoiceFilter {
                    search: Some(search.into()),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        assert_eq!(page.items.len(), expected);
    }
    let mut cash = input();
    cash.payment_term_type = bastion_facturas::PaymentTerm::Contado;
    assert!(matches!(
        service.create(1, "synthetic-key-cash", cash).await,
        Err(CoreError::Invalid)
    ));
    assert!(matches!(
        service.detail(1, u64::MAX).await,
        Err(CoreError::NotFound)
    ));
    let events: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM audit_events WHERE module_id='facturas'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(events, 2, "one create and one successful edit");
    let details: Vec<serde_json::Value> =
        sqlx::query_scalar("SELECT details_json FROM audit_events WHERE module_id='facturas'")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert!(
        details
            .iter()
            .all(|v| v.as_object().unwrap().len() == 1 && v.get("version").is_some())
    );
    core.admin()
        .set_module_enabled(1, "facturas", false)
        .await
        .unwrap();
    assert!(matches!(
        service.detail(1, a.id).await,
        Err(CoreError::NotFound)
    ));
    assert!(matches!(
        service.create(1, "synthetic-key-0001", input()).await,
        Err(CoreError::NotFound)
    ));
    core.admin()
        .set_module_enabled(1, "facturas", true)
        .await
        .unwrap();
    assert_eq!(service.detail(1, a.id).await.unwrap().version, 2);
    service.synchronize_release(false).await.unwrap();
    assert!(matches!(
        service.list(1, &InvoiceFilter::default()).await,
        Err(CoreError::NotFound)
    ));
    service.synchronize_release(true).await.unwrap();
    for _ in 0..120 {
        service.consume_request(1).await.unwrap();
    }
    assert!(matches!(
        service.consume_request(1).await,
        Err(CoreError::Locked)
    ));

    // Catalog selection uses the Clientes contract; snapshots survive disable
    // and do not depend on mutable copies sent by the browser.
    bastion_clientes::ClientService::migrate(&core.database())
        .await
        .unwrap();
    let clients = bastion_clientes::ClientService::new(core.database(), core.admin());
    clients.synchronize_release(true).await.unwrap();
    core.admin()
        .set_module_enabled(1, "clientes", true)
        .await
        .unwrap();
    let client = clients
        .create(
            1,
            "invoice-catalog-client-01",
            bastion_clientes::ClientInput {
                cliente_name: "Cliente de catálogo".into(),
                rtn: "08011900000000".into(),
                email: "client@example.test".into(),
                ..Default::default()
            },
        )
        .await
        .unwrap()
        .client;
    let mut selected = input();
    selected.id_client = Some(client.id);
    selected.client_name = "Untrusted override".into();
    selected.client_rtn = "forged".into();
    selected.client_email = "forged@example.test".into();
    sqlx::query("INSERT INTO role_permissions (role_id,module_id,scope_hash,scope_key,capability) VALUES (2,'facturas',UNHEX(SHA2('documentos',256)),'documentos','editar')").execute(&pool).await.unwrap();
    assert!(matches!(
        service
            .create(2, "invoice-forbidden-catalog", selected.clone())
            .await,
        Err(CoreError::Forbidden)
    ));
    let linked = service
        .create(1, "invoice-linked-client-01", selected.clone())
        .await
        .unwrap();
    assert_eq!(linked.data.client_name, client.data.cliente_name);
    assert_eq!(linked.data.client_rtn, client.data.rtn);
    assert_eq!(linked.data.client_email, client.data.email);
    core.admin()
        .set_module_enabled(1, "clientes", false)
        .await
        .unwrap();
    assert_eq!(
        service
            .create(1, "invoice-linked-client-01", selected.clone())
            .await
            .unwrap()
            .id,
        linked.id
    );
    let kept = service
        .replace(1, linked.id, linked.version, selected.clone())
        .await
        .unwrap();
    assert_eq!(kept.data.client_name, client.data.cliente_name);
    assert_eq!(kept.data.client_rtn, client.data.rtn);
    selected.id_client = Some(client.id + 1);
    assert!(matches!(
        service
            .replace(1, kept.id, kept.version, selected.clone())
            .await,
        Err(CoreError::NotFound)
    ));
    selected.id_client = None;
    let manual = service
        .replace(1, kept.id, kept.version, selected)
        .await
        .unwrap();
    assert!(manual.data.client_rtn.is_empty());
    assert!(manual.data.client_email.is_empty());
}
