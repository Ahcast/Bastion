use bastion_core::{CoreError, CoreService, Database};
use bastion_cotizaciones::{QuoteInput, QuoteService, QuoteStatus};
use bastion_ordenes_compra::{DestinationInput, OrderService};
use bastion_workflows::{convert_quote_order, prepare_quote_order};
use sqlx::mysql::MySqlPoolOptions;
fn date() -> chrono::NaiveDate {
    "2026-10-23".parse().unwrap()
}
fn input() -> QuoteInput {
    serde_json::from_value(serde_json::json!({
        "client_name":"Cliente sintético","order_number":"OC-SINTETICA","quote_date":"2026-10-23","valid_days":30,
        "status":"Aprobada","notes":"No enviar","terms":"Prueba","discount_type":"percent","discount_value":"5",
        "items":[{"description":"Producto","quantity":"1.25","unit_price":"100","discount_type":"amount","discount_value":"2","tax_type":"isv18"}]
    })).unwrap()
}
#[tokio::test]
async fn mysql57_atomic_quote_order_conversion_permissions_retries_and_immutable_snapshot() {
    let Ok(url) = std::env::var("BASTION_CONVERSION_TEST_URL") else {
        eprintln!(
            "Requires disposable BASTION_CONVERSION_TEST_URL; not executed locally without it."
        );
        return;
    };
    let pool = MySqlPoolOptions::new()
        .max_connections(8)
        .connect(&url)
        .await
        .unwrap();
    let dbname: String = sqlx::query_scalar("SELECT DATABASE()")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        dbname, "bastion_conversion_test",
        "Only the disposable conversion database is allowed"
    );
    let database = Database::from_test_pool(pool.clone());
    database.migrate().await.unwrap();
    QuoteService::migrate(&database).await.unwrap();
    OrderService::migrate(&database).await.unwrap();
    QuoteService::migrate(&database).await.unwrap();
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
    let core = CoreService::new(database.clone(), "x".repeat(32), [1; 32]).unwrap();
    let quotes = QuoteService::new(database.clone(), core.admin());
    let orders = OrderService::new(database.clone(), core.admin());
    quotes.synchronize_release(true).await.unwrap();
    orders.synchronize_release(true).await.unwrap();
    core.admin()
        .set_module_enabled(1, "cotizaciones", true)
        .await
        .unwrap();
    core.admin()
        .set_module_enabled(1, "ordenes_compra", true)
        .await
        .unwrap();
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
    let source = quotes
        .create(1, "synthetic-source-create-0001", input())
        .await
        .unwrap();
    assert!(matches!(
        prepare_quote_order(&quotes, &orders, 2, source.id, source.version, date()).await,
        Err(CoreError::Forbidden)
    ));
    assert!(matches!(
        prepare_quote_order(&quotes, &orders, 1, source.id, source.version + 1, date()).await,
        Err(CoreError::PreconditionFailed)
    ));
    let seed = prepare_quote_order(&quotes, &orders, 1, source.id, source.version, date())
        .await
        .unwrap();
    assert_eq!(
        quotes.detail(1, source.id).await.unwrap().version,
        source.version
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM ordenes_orders")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    let mut data = seed.data;
    data.id_destination = Some(999999);
    assert!(
        convert_quote_order(
            &quotes,
            &orders,
            1,
            source.id,
            source.version,
            "synthetic-convert-invalid-0001",
            data.clone()
        )
        .await
        .is_err()
    );
    assert_eq!(
        quotes.detail(1, source.id).await.unwrap().status,
        "Aprobada"
    );
    data.id_destination = Some(destination.id);
    let mut tampered = data.clone();
    tampered.approved_quote.as_mut().unwrap().discount_value = "0".into();
    assert!(matches!(
        convert_quote_order(
            &quotes,
            &orders,
            1,
            source.id,
            source.version,
            "synthetic-convert-tamper-0001",
            tampered
        )
        .await,
        Err(CoreError::Conflict)
    ));
    assert!(
        orders
            .create(1, "synthetic-bypass-create-0001", data.clone())
            .await
            .is_err()
    );
    // Force failure after the order/receipt insert to prove transaction rollback.
    sqlx::query("CREATE TRIGGER synthetic_conversion_failure BEFORE INSERT ON cotizaciones_order_conversions FOR EACH ROW SIGNAL SQLSTATE '45000' SET MESSAGE_TEXT='synthetic rollback test'").execute(&pool).await.unwrap();
    assert!(
        convert_quote_order(
            &quotes,
            &orders,
            1,
            source.id,
            source.version,
            "synthetic-convert-rollback-0001",
            data.clone()
        )
        .await
        .is_err()
    );
    sqlx::query("DROP TRIGGER synthetic_conversion_failure")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM ordenes_orders")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM ordenes_conversion_receipts")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        quotes.detail(1, source.id).await.unwrap().status,
        "Aprobada"
    );
    core.admin()
        .set_module_enabled(1, "ordenes_compra", false)
        .await
        .unwrap();
    assert!(matches!(
        convert_quote_order(
            &quotes,
            &orders,
            1,
            source.id,
            source.version,
            "synthetic-convert-success-0001",
            data.clone()
        )
        .await,
        Err(CoreError::NotFound)
    ));
    core.admin()
        .set_module_enabled(1, "ordenes_compra", true)
        .await
        .unwrap();
    let (one, two) = tokio::join!(
        convert_quote_order(
            &quotes,
            &orders,
            1,
            source.id,
            source.version,
            "synthetic-convert-success-0001",
            data.clone()
        ),
        convert_quote_order(
            &quotes,
            &orders,
            1,
            source.id,
            source.version,
            "synthetic-convert-success-0001",
            data.clone()
        )
    );
    let order = one.unwrap();
    assert_eq!(order.id, two.unwrap().id);
    assert_eq!(order.source_quote.as_ref().unwrap().id, source.id);
    assert_eq!(order.source_quote.as_ref().unwrap().version, source.version);
    assert_eq!(
        order.calculation.totals.total,
        source.calculation.totals.total
    );
    let converted = quotes.detail(1, source.id).await.unwrap();
    assert_eq!(converted.status, "Convertido");
    assert_eq!(converted.version, source.version + 1);
    assert!(matches!(
        quotes.delete(1, source.id, converted.version).await,
        Err(CoreError::Conflict)
    ));
    assert!(
        convert_quote_order(
            &quotes,
            &orders,
            1,
            source.id,
            source.version,
            "synthetic-other-key-0000001",
            data.clone()
        )
        .await
        .is_err()
    );
    let mut changed = data.clone();
    changed.comments = "Otro cuerpo".into();
    assert!(matches!(
        convert_quote_order(
            &quotes,
            &orders,
            1,
            source.id,
            source.version,
            "synthetic-convert-success-0001",
            changed
        )
        .await,
        Err(CoreError::Conflict)
    ));
    let mut operational = order.data.clone();
    operational.products[0].sale_price = "10".into();
    operational.products[0].tracking = "Operación".into();
    let updated = orders
        .replace(1, order.id, order.version, operational.clone())
        .await
        .unwrap();
    assert_eq!(
        updated.calculation.totals.total,
        order.calculation.totals.total
    );
    assert_eq!(updated.source_quote.unwrap().id, source.id);
    operational.approved_quote.as_mut().unwrap().discount_value = "0".into();
    assert!(matches!(
        orders
            .replace(1, order.id, updated.version, operational)
            .await,
        Err(CoreError::Conflict)
    ));
    // Both edit permissions are required even for recovering a prior receipt.
    sqlx::query("INSERT INTO role_permissions (role_id,module_id,scope_hash,scope_key,capability) VALUES (2,'cotizaciones',UNHEX(SHA2('documentos',256)),'documentos','editar'),(2,'cotizaciones',UNHEX(SHA2('documentos',256)),'documentos','ver')").execute(&pool).await.unwrap();
    assert!(matches!(
        convert_quote_order(
            &quotes,
            &orders,
            2,
            source.id,
            source.version,
            "synthetic-convert-success-0001",
            data
        )
        .await,
        Err(CoreError::Forbidden)
    ));
    let draft = quotes
        .create(
            1,
            "synthetic-source-create-0002",
            QuoteInput {
                status: QuoteStatus::Borrador,
                ..input()
            },
        )
        .await
        .unwrap();
    assert!(matches!(
        prepare_quote_order(&quotes, &orders, 1, draft.id, draft.version, date()).await,
        Err(CoreError::Conflict)
    ));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM ordenes_orders")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM cotizaciones_order_conversions")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
    assert_eq!(sqlx::query_scalar::<_,i64>("SELECT COUNT(*) FROM audit_events WHERE action IN ('quote.to_order','order.from_quote')").fetch_one(&pool).await.unwrap(),2);
}
