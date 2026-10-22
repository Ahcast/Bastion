use bastion_clientes::{ClientFilter, ClientInput, ClientService};
use bastion_core::{CoreError, CoreService, Database};
use sqlx::mysql::MySqlPoolOptions;

fn input(name: &str, rtn: &str, email: &str) -> ClientInput {
    ClientInput {
        cliente_name: name.into(),
        rtn: rtn.into(),
        email: email.into(),
        ..Default::default()
    }
}

#[tokio::test]
async fn mysql57_clients_merge_catalog_permissions_concurrency_and_disable() {
    let Ok(url) = std::env::var("BASTION_CLIENTES_TEST_URL") else {
        eprintln!("Requires isolated BASTION_CLIENTES_TEST_URL; CI supplies it.");
        return;
    };
    let pool = MySqlPoolOptions::new()
        .max_connections(5)
        .connect(&url)
        .await
        .unwrap();
    let database = Database::from_test_pool(pool.clone());
    database.migrate().await.unwrap();
    ClientService::migrate(&database).await.unwrap();
    ClientService::migrate(&database).await.unwrap();
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
    let service = ClientService::new(database, core.admin());
    service.synchronize_release(true).await.unwrap();
    assert!(matches!(
        service.list(1, &ClientFilter::default()).await,
        Err(CoreError::NotFound)
    ));
    core.admin()
        .set_module_enabled(1, "clientes", true)
        .await
        .unwrap();
    assert!(matches!(
        service.catalog(2, &ClientFilter::default()).await,
        Err(CoreError::Forbidden)
    ));
    sqlx::query("INSERT INTO role_permissions (role_id,module_id,scope_hash,scope_key,capability) VALUES (2,'clientes',UNHEX(SHA2('catalogo',256)),'catalogo','ver')").execute(&pool).await.unwrap();
    assert!(service.list(2, &ClientFilter::default()).await.is_ok());
    assert!(matches!(
        service
            .create(2, "viewer-cannot-create", input("No", "", ""))
            .await,
        Err(CoreError::Forbidden)
    ));

    // Concurrent retries return exactly one saved response and one audit event.
    let payload = input("  Compañía José  ", "0801-1234", "old@example.test");
    let (a, b) = tokio::join!(
        service.create(1, "client-create-retry-1", payload.clone()),
        service.create(1, "client-create-retry-1", payload)
    );
    let a = a.unwrap();
    let b = b.unwrap();
    assert_eq!(a.client.id, b.client.id);
    assert_eq!(a.client.version, b.client.version);
    assert!(!a.merged);
    let id = a.client.id;
    assert!(matches!(
        service
            .create(1, "client-create-retry-1", input("Different", "", ""))
            .await,
        Err(CoreError::Conflict)
    ));

    // Name wins over a conflicting supplied RTN; merge never replaces RTN/email.
    let mut incoming = input("COMPANIA  JOSE", "999", "new@example.test");
    incoming.user_contact = "Contacto".into();
    let merged = service
        .create(1, "client-merge-by-name", incoming)
        .await
        .unwrap();
    assert!(merged.merged);
    assert_eq!(merged.client.id, id);
    assert_eq!(merged.client.data.rtn, "0801-1234");
    assert_eq!(merged.client.data.email, "old@example.test");
    assert_eq!(merged.client.data.user_contact, "Contacto");
    // Same RTN digits under another name updates that record's name.
    let merged = service
        .create(
            1,
            "client-merge-by-rtn",
            input("Nuevo nombre", "08011234", ""),
        )
        .await
        .unwrap();
    assert_eq!(merged.client.id, id);
    assert_eq!(merged.client.data.cliente_name, "Nuevo nombre");
    assert_eq!(merged.client.data.rtn, "0801-1234");
    assert!(matches!(
        service
            .replace(2, id, merged.client.version, input("No", "", ""))
            .await,
        Err(CoreError::Forbidden)
    ));
    let (a, b) = tokio::join!(
        service.replace(1, id, merged.client.version, input("Editado", "", "")),
        service.replace(1, id, merged.client.version, input("Editado", "", ""))
    );
    assert!(a.is_ok() ^ b.is_ok());
    assert!(matches!(
        if a.is_err() {
            a.unwrap_err()
        } else {
            b.unwrap_err()
        },
        CoreError::PreconditionFailed
    ));
    assert_eq!(service.detail(2, id).await.unwrap().data.email, ""); // Optional fields can be cleared on edit.
    assert!(matches!(
        service.detail(2, u64::MAX).await,
        Err(CoreError::NotFound)
    ));

    // Different keys racing the same canonical name still produce one client.
    let (a, b) = tokio::join!(
        service.create(1, "client-distinct-key-a", input("Árbol", "", "")),
        service.create(1, "client-distinct-key-b", input("ARBOL", "", ""))
    );
    let a = a.unwrap();
    let b = b.unwrap();
    assert_eq!(a.client.id, b.client.id);
    let second = service
        .create(
            1,
            "client-second-record",
            input("Original", "222", "best@example.test"),
        )
        .await
        .unwrap();
    // Legacy permits duplicate names on edit; catalog picks email, RTN, then lowest ID.
    service
        .replace(
            1,
            second.client.id,
            second.client.version,
            input("árbol", "222", "best@example.test"),
        )
        .await
        .unwrap();
    let catalog = service.catalog(2, &ClientFilter::default()).await.unwrap();
    assert_eq!(catalog.items.len(), 2);
    assert_eq!(catalog.items[0].id, second.client.id);
    let filter = ClientFilter {
        search: Some("best@".into()),
        ..Default::default()
    };
    assert_eq!(
        service.list(2, &filter).await.unwrap().items[0].id,
        second.client.id
    );
    for search in ["%", "_", "' OR 1=1 --"] {
        assert!(
            service
                .list(
                    2,
                    &ClientFilter {
                        search: Some(search.into()),
                        ..Default::default()
                    }
                )
                .await
                .unwrap()
                .items
                .is_empty()
        );
    }
    let first = service
        .list(
            2,
            &ClientFilter {
                page_size: Some(1),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert!(first.has_more);
    assert_eq!(first.items.len(), 1);
    let last = service
        .list(
            2,
            &ClientFilter {
                page_size: Some(1),
                page: Some(2),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert!(!last.has_more);
    assert_ne!(first.items[0].id, last.items[0].id);
    assert!(
        service
            .list(
                2,
                &ClientFilter {
                    page_size: Some(101),
                    ..Default::default()
                }
            )
            .await
            .is_err()
    );
    let events: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM audit_events WHERE module_id='clientes' AND action='client.create'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(events, 3);
    let leaked: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM audit_events WHERE module_id='clientes' AND CAST(details_json AS CHAR) LIKE '%example.test%'").fetch_one(&pool).await.unwrap();
    assert_eq!(leaked, 0);
    for _ in 0..120 {
        service.consume_request(2).await.unwrap();
    }
    assert!(matches!(
        service.consume_request(2).await,
        Err(CoreError::Locked)
    ));
    core.admin()
        .set_module_enabled(1, "clientes", false)
        .await
        .unwrap();
    assert!(matches!(
        service.detail(1, id).await,
        Err(CoreError::NotFound)
    ));
    assert!(matches!(
        service
            .create(1, "disabled-no-creation", input("Disabled", "", ""))
            .await,
        Err(CoreError::NotFound)
    ));
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM clientes_records")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 3);
    service.synchronize_release(false).await.unwrap();
    assert!(
        core.admin()
            .set_module_enabled(1, "clientes", true)
            .await
            .is_err()
    );
    // Migration checksum mismatch must make activation unavailable.
    sqlx::query("UPDATE _clientes_migrations SET checksum=UNHEX(REPEAT('00',48)) WHERE version=1")
        .execute(&pool)
        .await
        .unwrap();
    service.synchronize_release(true).await.unwrap();
    assert!(
        core.admin()
            .set_module_enabled(1, "clientes", true)
            .await
            .is_err()
    );
}
