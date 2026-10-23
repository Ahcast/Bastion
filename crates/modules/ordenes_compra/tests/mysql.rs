use bastion_core::{CoreError, CoreService, Database};
use bastion_ordenes_compra::{DestinationInput, OrderFilter, OrderInput, OrderService};
use sqlx::mysql::MySqlPoolOptions;

fn input() -> OrderInput {
    serde_json::from_value(serde_json::json!({"n_orders":"OC-1","n_bill":"","order_date":"2026-10-23","received_date":null,"client_name":"Cliente % _","client_rtn":"","id_destination":1,"comments":"Privado","products":[{"description":"Equipo","cantidad":2,"sale_price":"60","purchase_price":"100","tracking":"","isv":true,"status":"Pendiente","coments":""}],"expenses":[{"description":"Flete","amount":"5"}]})).unwrap()
}
#[tokio::test]
async fn mysql57_orders_destinations_atomicity_permissions_and_conflicts() {
    let Ok(url) = std::env::var("BASTION_ORDENES_TEST_URL") else {
        eprintln!("Requires isolated BASTION_ORDENES_TEST_URL; CI supplies it.");
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
    let service = OrderService::new(database.clone(), core.admin());
    service.synchronize_release(true).await.unwrap();
    let allowed: bool = sqlx::query_scalar(
        "SELECT environment_allowed FROM module_registry WHERE module_id='ordenes_compra'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(!allowed);
    OrderService::migrate(&database).await.unwrap();
    OrderService::migrate(&database).await.unwrap();
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
        service.list(1, &OrderFilter::default()).await,
        Err(CoreError::NotFound)
    ));
    core.admin()
        .set_module_enabled(1, "ordenes_compra", true)
        .await
        .unwrap();
    assert!(
        matches!(
            service.create(1, "synthetic-key-0001", input()).await,
            Err(CoreError::Invalid)
        ),
        "destination must exist"
    );
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM ordenes_orders")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
    let a = service
        .save_destination(
            1,
            None,
            None,
            DestinationInput {
                destination: "  Oficina   central ".into(),
            },
        )
        .await
        .unwrap();
    assert_eq!(a.destination, "Oficina central");
    let b = service
        .save_destination(
            1,
            None,
            None,
            DestinationInput {
                destination: "OFICINA CENTRAL".into(),
            },
        )
        .await
        .unwrap();
    assert_eq!(a.id, b.id);
    service
        .save_destination(
            1,
            Some(a.id),
            Some(1),
            DestinationInput {
                destination: "Centro".into(),
            },
        )
        .await
        .unwrap();
    assert!(matches!(
        service
            .save_destination(
                1,
                Some(a.id),
                Some(1),
                DestinationInput {
                    destination: "Obsoleto".into()
                }
            )
            .await,
        Err(CoreError::PreconditionFailed)
    ));
    assert!(matches!(
        service.destinations(2).await,
        Err(CoreError::Forbidden)
    ));
    sqlx::query("INSERT INTO role_permissions (role_id,module_id,scope_hash,scope_key,capability) VALUES (2,'ordenes_compra',UNHEX(SHA2('documentos',256)),'documentos','ver')").execute(&pool).await.unwrap();
    assert!(service.destinations(2).await.is_ok());
    assert!(matches!(
        service
            .save_destination(
                2,
                None,
                None,
                DestinationInput {
                    destination: "Prohibido".into()
                }
            )
            .await,
        Err(CoreError::Forbidden)
    ));
    assert!(matches!(
        service.create(2, "synthetic-key-0001", input()).await,
        Err(CoreError::Forbidden)
    ));
    let (a, b) = tokio::join!(
        service.create(1, "synthetic-key-0001", input()),
        service.create(1, "synthetic-key-0001", input())
    );
    let a = a.unwrap();
    assert_eq!(a.id, b.unwrap().id);
    assert_eq!(a.calculation.totals.total, "230.00");
    assert_eq!(a.calculation.totals.investment_total, "125.00");
    let mut changed = input();
    changed.comments = "Actualizado".into();
    assert!(matches!(
        service
            .create(1, "synthetic-key-0001", changed.clone())
            .await,
        Err(CoreError::Conflict)
    ));
    let (first, second) = tokio::join!(
        service.replace(1, a.id, 1, changed),
        service.replace(1, a.id, 1, input())
    );
    assert_eq!(usize::from(first.is_ok()) + usize::from(second.is_ok()), 1);
    assert!(matches!(
        if first.is_err() {
            first.unwrap_err()
        } else {
            second.unwrap_err()
        },
        CoreError::PreconditionFailed
    ));
    assert_eq!(
        service
            .create(1, "synthetic-key-0001", input())
            .await
            .unwrap()
            .version,
        1
    );
    assert_eq!(service.detail(2, a.id).await.unwrap().version, 2);
    for (search, expected) in [("%", 1), ("_", 1), ("' OR 1=1 --", 0)] {
        assert_eq!(
            service
                .list(
                    2,
                    &OrderFilter {
                        search: Some(search.into()),
                        ..Default::default()
                    }
                )
                .await
                .unwrap()
                .items
                .len(),
            expected
        );
    }
    assert!(
        service
            .list(
                2,
                &OrderFilter {
                    min_total: Some("231".into()),
                    ..Default::default()
                }
            )
            .await
            .unwrap()
            .items
            .is_empty()
    );
    let events:i64=sqlx::query_scalar("SELECT COUNT(*) FROM audit_events WHERE module_id='ordenes_compra' AND action='order.create'").fetch_one(&pool).await.unwrap();
    assert_eq!(events, 1);
    core.admin()
        .set_module_enabled(1, "ordenes_compra", false)
        .await
        .unwrap();
    assert!(matches!(
        service.detail(1, a.id).await,
        Err(CoreError::NotFound)
    ));
    assert!(matches!(
        service.destinations(1).await,
        Err(CoreError::NotFound)
    ));
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM ordenes_orders")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
}
