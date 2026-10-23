use bastion_core::{CoreError, CoreService, DashboardPeriod, Database};
use bastion_cotizaciones::{
    DiscountType, QuoteFilter, QuoteInput, QuoteItem, QuoteMailService, QuoteService, QuoteStatus,
    TaxType,
};
use sqlx::mysql::MySqlPoolOptions;

fn input() -> QuoteInput {
    QuoteInput {
        id_client: None,
        client_name: "Cliente sintético".into(),
        client_rtn: String::new(),
        client_email: String::new(),
        order_number: "ORD-PRUEBA".into(),
        quote_date: "2026-10-22".parse().unwrap(),
        valid_days: 30,
        status: QuoteStatus::Borrador,
        purchase_order_exempt_number: String::new(),
        registration_certificate_number: String::new(),
        sag_registration_number: String::new(),
        contact_email: String::new(),
        prepared_by_name: String::new(),
        notes: String::new(),
        terms: String::new(),
        discount_type: DiscountType::Amount,
        discount_value: "0".into(),
        items: vec![QuoteItem {
            description: "Producto sintético % _".into(),
            quantity: "0.125".into(),
            unit_price: "10.1234".into(),
            discount_type: DiscountType::Percent,
            discount_value: "5".into(),
            tax_type: TaxType::Isv15,
        }],
    }
}

#[tokio::test]
async fn mysql57_drafts_permissions_idempotency_versions_and_disable() {
    let Ok(url) = std::env::var("BASTION_COTIZACIONES_TEST_URL") else {
        eprintln!("Requires isolated BASTION_COTIZACIONES_TEST_URL; CI supplies it.");
        return;
    };
    // Dedicated disposable database only, independent from the Procesos fixtures.
    let pool = MySqlPoolOptions::new()
        .max_connections(4)
        .connect(&url)
        .await
        .unwrap();
    let database = Database::from_test_pool(pool.clone());
    database.migrate().await.unwrap();
    QuoteService::migrate(&database).await.unwrap();
    QuoteService::migrate(&database).await.unwrap();
    sqlx::query("INSERT INTO persons (id,full_name) VALUES (1,'Admin test'),(2,'Viewer test')")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO roles (id,name) VALUES (2,'Viewer')")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO users (id,person_id,role_id,email,status) VALUES (1,1,1,'admin@example.test','active'),(2,2,2,'viewer@example.test','active')").execute(&pool).await.unwrap();
    let core = CoreService::new(database.clone(), "x".repeat(32), [1; 32]).unwrap();
    let service = QuoteService::new(database.clone(), core.admin());
    service.synchronize_release(true).await.unwrap();
    assert!(matches!(
        service.list(1, &QuoteFilter::default()).await,
        Err(CoreError::NotFound)
    ));
    core.admin()
        .set_module_enabled(1, "cotizaciones", true)
        .await
        .unwrap();
    assert!(matches!(
        service.list(2, &QuoteFilter::default()).await,
        Err(CoreError::Forbidden)
    ));
    assert!(matches!(
        service.dashboard(2, DashboardPeriod::All).await,
        Err(CoreError::Forbidden)
    ));
    assert_eq!(
        service.dashboard(1, DashboardPeriod::All).await.unwrap()["metrics"]["total"],
        0
    );
    sqlx::query("INSERT INTO role_permissions (role_id,module_id,scope_hash,scope_key,capability) VALUES (2,'cotizaciones',UNHEX(SHA2('documentos',256)),'documentos','ver')").execute(&pool).await.unwrap();
    service
        .list(2, &QuoteFilter::default())
        .await
        .expect("A role with document read permission must access the enabled module");
    assert!(matches!(
        service
            .create(2, "synthetic-create-key-0001", input())
            .await,
        Err(CoreError::Forbidden)
    ));
    let (one, two) = tokio::join!(
        service.create(1, "synthetic-create-key-0001", input()),
        service.create(1, "synthetic-create-key-0001", input())
    );
    let one = one.unwrap();
    assert_eq!(one.id, two.unwrap().id);
    assert_eq!(one.data.items[0].unit_price, "10.12");
    assert_eq!(one.quote_number.as_deref(), Some("COT-000001"));
    assert_eq!(one.data.prepared_by_name, "Admin test");
    assert_eq!(one.data.contact_email, "admin@example.test");
    assert_eq!(one.calculation.valid_until, "2026-11-21");
    let summary = service.dashboard(2, DashboardPeriod::All).await.unwrap();
    assert_eq!(summary["metrics"]["total"], 1);
    assert_eq!(summary["metrics"]["amount"], one.calculation.totals.total);
    assert_eq!(summary["distribution"][0]["label"], "Borrador");
    assert_eq!(summary["series"][0]["bucket"], "2026-10");
    assert_eq!(summary["recent"][0]["quote_number"], "COT-000001");
    assert!(summary["recent"][0].get("notes").is_none());
    sqlx::query("UPDATE cotizaciones_drafts SET calculation_json=JSON_REMOVE(calculation_json,'$.totals.total') WHERE id=?").bind(one.id).execute(&pool).await.unwrap();
    let unpriced = service.dashboard(2, DashboardPeriod::All).await.unwrap();
    assert_eq!(unpriced["metrics"]["total"], 1);
    assert_eq!(unpriced["metrics"]["without_total"], 1);
    assert_eq!(unpriced["metrics"]["amount"], "0.00");
    assert!(unpriced["recent"][0]["total"].is_null());
    sqlx::query("UPDATE cotizaciones_drafts SET calculation_json=JSON_SET(calculation_json,'$.totals.total',?) WHERE id=?").bind(&one.calculation.totals.total).bind(one.id).execute(&pool).await.unwrap();
    // A date outside the selected Honduras month cannot affect its summary.
    sqlx::query("UPDATE cotizaciones_drafts SET data_json=JSON_SET(data_json,'$.quote_date','2000-01-01') WHERE id=?").bind(one.id).execute(&pool).await.unwrap();
    let month = service.dashboard(1, DashboardPeriod::Month).await.unwrap();
    assert_eq!(month["metrics"]["total"], 0);
    assert_eq!(month["metrics"]["amount"], "0.00");
    assert!(
        month["series"]
            .as_array()
            .unwrap()
            .iter()
            .all(|point| point["count"] == 0 && point["amount"] == "0.00")
    );
    sqlx::query(
        "UPDATE cotizaciones_drafts SET data_json=JSON_SET(data_json,'$.quote_date',?) WHERE id=?",
    )
    .bind(one.data.quote_date.to_string())
    .bind(one.id)
    .execute(&pool)
    .await
    .unwrap();
    let mut changed = input();
    changed.notes = "Cambio".into();
    changed.status = QuoteStatus::Aprobada;
    changed.prepared_by_name = "Falso".into();
    changed.contact_email = "forged@example.test".into();
    assert!(matches!(
        service
            .create(1, "synthetic-create-key-0001", changed.clone())
            .await,
        Err(CoreError::Conflict)
    ));
    let (first, stale) = tokio::join!(
        service.replace(1, one.id, 1, changed.clone()),
        service.replace(1, one.id, 1, changed)
    );
    assert!(first.is_ok() ^ stale.is_ok());
    let failure = if first.is_err() {
        first.unwrap_err()
    } else {
        stale.unwrap_err()
    };
    assert!(matches!(failure, CoreError::PreconditionFailed));
    let edited = service.detail(2, one.id).await.unwrap();
    assert_eq!(edited.version, 2);
    assert_eq!(edited.quote_number, one.quote_number);
    assert_eq!(edited.status, "Aprobada");
    assert_eq!(edited.data.prepared_by_name, "Admin test");
    assert_eq!(edited.data.contact_email, "admin@example.test");
    let preview = service.preview(1, &edited.data).await.unwrap();
    assert_eq!(preview.totals.total, edited.calculation.totals.total);
    assert!(matches!(
        service.replace(2, one.id, 2, input()).await,
        Err(CoreError::Forbidden)
    ));
    // Retrying creation returns its original snapshot, even after editing.
    assert_eq!(
        service
            .create(1, "synthetic-create-key-0001", input())
            .await
            .unwrap()
            .version,
        1
    );
    let filter = QuoteFilter {
        q: Some("% _".into()),
        ..Default::default()
    };
    assert_eq!(service.list(2, &filter).await.unwrap().items.len(), 1);
    let none = QuoteFilter {
        q: Some("' OR 1=1 --".into()),
        ..Default::default()
    };
    assert!(service.list(2, &none).await.unwrap().items.is_empty());
    let events: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM audit_events WHERE module_id='cotizaciones'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(events, 2);
    assert!(matches!(
        service
            .change_status(2, one.id, 2, QuoteStatus::Enviada)
            .await,
        Err(CoreError::Forbidden)
    ));
    assert!(matches!(
        service
            .clone_quote(2, one.id, 2, "synthetic-clone-key-0001")
            .await,
        Err(CoreError::Forbidden)
    ));
    assert!(matches!(
        service.delete(2, one.id, 2).await,
        Err(CoreError::Forbidden)
    ));
    let sent = service
        .change_status(1, one.id, 2, QuoteStatus::Enviada)
        .await
        .unwrap();
    assert_eq!(sent.version, 3);
    assert_eq!(sent.data.status.as_str(), "Enviada");
    assert_eq!(sent.calculation.data.status.as_str(), "Enviada");
    assert_eq!(
        sent.calculation.totals.total,
        edited.calculation.totals.total
    );
    assert!(matches!(
        service
            .change_status(1, one.id, 2, QuoteStatus::Aprobada)
            .await,
        Err(CoreError::PreconditionFailed)
    ));
    let approved = service
        .change_status(1, one.id, 3, QuoteStatus::Aprobada)
        .await
        .unwrap();
    sqlx::query("INSERT INTO role_permissions (role_id,module_id,scope_hash,scope_key,capability) VALUES (2,'cotizaciones',UNHEX(SHA2('documentos',256)),'documentos','editar')").execute(&pool).await.unwrap();
    assert!(matches!(
        service
            .clone_quote(2, one.id, 3, "synthetic-clone-key-0001")
            .await,
        Err(CoreError::PreconditionFailed)
    ));
    let (cloned, replay) = tokio::join!(
        service.clone_quote(2, one.id, approved.version, "synthetic-clone-key-0001"),
        service.clone_quote(2, one.id, approved.version, "synthetic-clone-key-0001")
    );
    let cloned = cloned.unwrap();
    assert_eq!(cloned.id, replay.unwrap().id);
    assert_eq!(cloned.quote_number.as_deref(), Some("COT-000002"));
    assert_eq!(cloned.data.client_name, "");
    assert_eq!(cloned.data.prepared_by_name, "Viewer test");
    assert_eq!(cloned.data.contact_email, "viewer@example.test");
    assert_eq!(cloned.status, "Borrador");
    assert_eq!(
        cloned.data.quote_date,
        chrono::Utc::now()
            .with_timezone(&chrono::FixedOffset::west_opt(21600).unwrap())
            .date_naive()
    );
    assert_eq!(
        cloned.data.items[0].description,
        approved.data.items[0].description
    );
    assert_eq!(cloned.data.notes, approved.data.notes);
    assert_eq!(
        cloned.calculation.totals.total,
        approved.calculation.totals.total
    );
    assert_eq!(
        service.detail(1, one.id).await.unwrap().version,
        approved.version
    );
    assert!(matches!(
        service
            .clone_quote(2, one.id, 3, "synthetic-clone-key-0001")
            .await,
        Err(CoreError::Conflict)
    ));
    assert!(matches!(
        service
            .clone_quote(2, one.id, approved.version, "bad")
            .await,
        Err(CoreError::Invalid)
    ));
    assert!(matches!(
        service.delete(2, cloned.id, 1).await,
        Err(CoreError::Forbidden)
    ));
    assert!(matches!(
        service.delete(1, cloned.id, 2).await,
        Err(CoreError::PreconditionFailed)
    ));
    let document_key = format!("cotizaciones/{}/synthetic.pdf", cloned.id);
    sqlx::query("INSERT INTO cotizaciones_documents (object_key,quote_id,quote_version,kind,filename,mime,sha256,byte_size,created_at) VALUES (?,?,1,'pdf','synthetic.pdf','application/pdf',UNHEX(SHA2('synthetic',256)),10,UTC_TIMESTAMP(6))").bind(&document_key).bind(cloned.id).execute(&pool).await.unwrap();
    let pending=sqlx::query("INSERT INTO cotizaciones_email_jobs (quote_id,quote_version,actor_id,key_hash,request_hash,payload_ciphertext,delivery_status,scheduled_at,created_at) VALUES (?,1,2,UNHEX(SHA2('synthetic-job',256)),UNHEX(SHA2('payload',256)),X'01','delivering',UTC_TIMESTAMP(6),UTC_TIMESTAMP(6))").bind(cloned.id).execute(&pool).await.unwrap().last_insert_id();
    assert!(matches!(
        service.delete(1, cloned.id, 1).await,
        Err(CoreError::Conflict)
    ));
    sqlx::query("UPDATE cotizaciones_email_jobs SET delivery_status='pending' WHERE id=?")
        .bind(pending)
        .execute(&pool)
        .await
        .unwrap();
    service.delete(1, cloned.id, 1).await.unwrap();
    let cancelled: (String, Vec<u8>) = sqlx::query_as(
        "SELECT delivery_status,payload_ciphertext FROM cotizaciones_email_jobs WHERE id=?",
    )
    .bind(pending)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(cancelled.0, "cancelled");
    assert!(cancelled.1.is_empty());
    let cleanup: String =
        sqlx::query_scalar("SELECT object_key FROM cotizaciones_document_deletions")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(cleanup, document_key);
    assert!(matches!(
        service.detail(1, cloned.id).await,
        Err(CoreError::NotFound)
    ));
    let audit: Vec<String> = sqlx::query_scalar(
        "SELECT action FROM audit_events WHERE module_id='cotizaciones' ORDER BY id",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        audit,
        [
            "quote.create",
            "quote.update",
            "quote.status",
            "quote.status",
            "quote.clone",
            "quote.delete"
        ]
    );
    sqlx::query("DELETE FROM role_permissions WHERE role_id=2 AND module_id='cotizaciones' AND capability='editar'").execute(&pool).await.unwrap();
    let revoked=sqlx::query("INSERT INTO cotizaciones_email_jobs (quote_id,quote_version,actor_id,key_hash,request_hash,payload_ciphertext,scheduled_at,created_at) VALUES (?,?,2,UNHEX(SHA2('revoked-job',256)),UNHEX(SHA2('payload',256)),X'01',UTC_TIMESTAMP(6),UTC_TIMESTAMP(6))").bind(one.id).bind(approved.version).execute(&pool).await.unwrap().last_insert_id();
    assert!(
        QuoteMailService::new(core.clone())
            .deliver_one()
            .await
            .unwrap()
    );
    let failed: (String, Option<String>) = sqlx::query_as(
        "SELECT delivery_status,last_error_code FROM cotizaciones_email_jobs WHERE id=?",
    )
    .bind(revoked)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(failed.0, "failed");
    assert_eq!(failed.1.as_deref(), Some("authorization_changed"));
    core.admin()
        .set_module_enabled(1, "cotizaciones", false)
        .await
        .unwrap();
    assert!(matches!(
        service.dashboard(1, DashboardPeriod::All).await,
        Err(CoreError::NotFound)
    ));
    assert!(matches!(
        service.detail(1, one.id).await,
        Err(CoreError::NotFound)
    ));
    assert!(matches!(
        service
            .create(1, "synthetic-create-key-0002", input())
            .await,
        Err(CoreError::NotFound)
    ));
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM cotizaciones_drafts")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
    core.admin()
        .set_module_enabled(1, "cotizaciones", true)
        .await
        .unwrap();
    verify_client_selection(&core, &database, &service).await;
    service.synchronize_release(false).await.unwrap();
    assert!(matches!(
        service.detail(1, one.id).await,
        Err(CoreError::NotFound)
    ));
}

async fn verify_client_selection(core: &CoreService, database: &Database, quotes: &QuoteService) {
    use bastion_clientes::{ClientInput, ClientService};
    ClientService::migrate(database).await.unwrap();
    let clients = ClientService::new(database.clone(), core.admin());
    clients.synchronize_release(true).await.unwrap();
    core.admin()
        .set_module_enabled(1, "clientes", true)
        .await
        .unwrap();
    let client = clients
        .create(
            1,
            "synthetic-client-link-01",
            ClientInput {
                cliente_name: "Cliente de catálogo".into(),
                rtn: "08011900000000".into(),
                email: "catalog@example.test".into(),
                ..Default::default()
            },
        )
        .await
        .unwrap()
        .client;
    let mut request = input();
    request.id_client = Some(client.id);
    request.client_name = "Nombre falsificado".into();
    request.client_rtn = "falso".into();
    request.client_email = "forged@example.test".into();
    let created = quotes
        .create(1, "synthetic-selected-quote-01", request.clone())
        .await
        .unwrap();
    assert_eq!(created.data.id_client, Some(client.id));
    assert_eq!(created.data.client_name, client.data.cliente_name);
    assert_eq!(created.data.client_rtn, client.data.rtn);
    assert_eq!(created.data.client_email, client.data.email);
    assert_eq!(created.calculation.data.client_rtn, client.data.rtn);
    let mut changed_client = client.data.clone();
    changed_client.email = "updated@example.test".into();
    clients
        .replace(1, client.id, client.version, changed_client)
        .await
        .unwrap();
    let replay = quotes
        .create(1, "synthetic-selected-quote-01", request.clone())
        .await
        .unwrap();
    assert_eq!(replay.id, created.id);
    assert_eq!(replay.data.client_email, "catalog@example.test");
    let mut invalid = request.clone();
    invalid.id_client = Some(u64::MAX);
    assert!(matches!(
        quotes
            .create(1, "synthetic-missing-client-01", invalid)
            .await,
        Err(CoreError::NotFound)
    ));
    sqlx::query("INSERT INTO role_permissions (role_id,module_id,scope_hash,scope_key,capability) VALUES (2,'cotizaciones',UNHEX(SHA2('documentos',256)),'documentos','editar')").execute(database.pool()).await.unwrap();
    assert!(matches!(
        quotes
            .create(2, "synthetic-denied-client-01", request.clone())
            .await,
        Err(CoreError::Forbidden)
    ));
    sqlx::query("INSERT INTO role_permissions (role_id,module_id,scope_hash,scope_key,capability) VALUES (2,'clientes',UNHEX(SHA2('catalogo',256)),'catalogo','ver')").execute(database.pool()).await.unwrap();
    let permitted = quotes
        .create(2, "synthetic-allowed-client-01", request.clone())
        .await
        .unwrap();
    assert_eq!(permitted.data.client_email, "updated@example.test");
    core.admin()
        .set_module_enabled(1, "clientes", false)
        .await
        .unwrap();
    assert!(matches!(
        quotes
            .create(1, "synthetic-disabled-client-01", request.clone())
            .await,
        Err(CoreError::NotFound)
    ));
    request.notes = "Editar sin reemplazar cliente".into();
    let edited = quotes
        .replace(1, created.id, created.version, request.clone())
        .await
        .unwrap();
    assert_eq!(edited.data.client_name, "Cliente de catálogo");
    assert_eq!(edited.data.client_email, "catalog@example.test");
    assert_eq!(
        quotes.detail(1, created.id).await.unwrap().data.id_client,
        Some(client.id)
    );
    let cloned = quotes
        .clone_quote(1, edited.id, edited.version, "synthetic-client-clone-01")
        .await
        .unwrap();
    assert_eq!(cloned.data.id_client, None);
    assert!(cloned.data.client_name.is_empty());
    assert!(cloned.data.client_rtn.is_empty());
    assert!(cloned.data.client_email.is_empty());
    request.id_client = None;
    request.client_name = "Cliente manual".into();
    let manual = quotes
        .replace(1, edited.id, edited.version, request)
        .await
        .unwrap();
    assert_eq!(manual.data.client_name, "Cliente manual");
    assert!(manual.data.client_rtn.is_empty());
    assert!(manual.data.client_email.is_empty());
}
