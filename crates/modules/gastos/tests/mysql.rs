use bastion_core::{CoreError, CoreService, Database};
use bastion_gastos::{ExpenseFilter, ExpenseInput, ExpenseService};
use sqlx::mysql::MySqlPoolOptions;
fn input(invoice: &str, taxable: &str, exempt: &str) -> ExpenseInput {
    ExpenseInput {
        expense_date: chrono::NaiveDate::from_ymd_opt(2026, 10, 24).unwrap(),
        invoice_number: invoice.into(),
        provider: "Proveedor literal %_".into(),
        currency: "HNL".into(),
        taxable_amount: taxable.into(),
        exempt_amount: exempt.into(),
    }
}
#[tokio::test]
async fn mysql57_expenses_activation_crud_filters_idempotency_audit_and_soft_delete() {
    let Ok(url) = std::env::var("BASTION_GASTOS_TEST_URL") else {
        eprintln!("Requires isolated BASTION_GASTOS_TEST_URL; CI supplies it.");
        return;
    };
    let pool = MySqlPoolOptions::new()
        .max_connections(5)
        .connect(&url)
        .await
        .unwrap();
    let db_name: String = sqlx::query_scalar("SELECT DATABASE()")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(db_name, "bastion_gastos_test");
    let db = Database::from_test_pool(pool.clone());
    db.migrate().await.unwrap();
    sqlx::query(
        "INSERT INTO persons (id,full_name) VALUES (1,'Synthetic admin'),(2,'Synthetic viewer')",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO roles (id,name) VALUES (2,'Synthetic viewer')")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO users (id,person_id,role_id,email,status) VALUES (1,1,1,'admin@example.test','active'),(2,2,2,'viewer@example.test','active')").execute(&pool).await.unwrap();
    let core = CoreService::new(db.clone(), "x".repeat(32), [1; 32]).unwrap();
    let svc = ExpenseService::new(db.clone(), core.admin());
    svc.synchronize_release(true).await.unwrap();
    assert!(matches!(
        core.admin().set_module_enabled(1, "gastos", true).await,
        Err(CoreError::Forbidden)
    ));
    ExpenseService::migrate(&db).await.unwrap();
    ExpenseService::migrate(&db).await.unwrap();
    let checksum: Vec<u8> =
        sqlx::query_scalar("SELECT checksum FROM _gastos_migrations WHERE version=2")
            .fetch_one(&pool)
            .await
            .unwrap();
    sqlx::query("UPDATE _gastos_migrations SET checksum=REPEAT('x',48) WHERE version=2")
        .execute(&pool)
        .await
        .unwrap();
    svc.synchronize_release(true).await.unwrap();
    assert!(matches!(
        core.admin().set_module_enabled(1, "gastos", true).await,
        Err(CoreError::Forbidden)
    ));
    sqlx::query("UPDATE _gastos_migrations SET checksum=? WHERE version=2")
        .bind(checksum)
        .execute(&pool)
        .await
        .unwrap();
    svc.synchronize_release(true).await.unwrap();
    assert!(matches!(
        svc.list(1, &ExpenseFilter::default()).await,
        Err(CoreError::NotFound)
    ));
    core.admin()
        .set_module_enabled(1, "gastos", true)
        .await
        .unwrap();
    assert!(matches!(
        svc.list(2, &ExpenseFilter::default()).await,
        Err(CoreError::Forbidden)
    ));
    sqlx::query("INSERT INTO role_permissions (role_id,module_id,scope_hash,scope_key,capability) VALUES (2,'gastos',UNHEX(SHA2('registros',256)),'registros','ver')").execute(&pool).await.unwrap();
    assert!(
        svc.list(2, &ExpenseFilter::default())
            .await
            .unwrap()
            .items
            .is_empty()
    );
    assert!(matches!(
        svc.create(2, "viewer-cannot-create", input("NO", "1", "0"))
            .await,
        Err(CoreError::Forbidden)
    ));
    assert!(matches!(
        svc.preview(2, &input("NO", "1", "0")).await,
        Err(CoreError::Forbidden)
    ));
    let payload = input("FACT-001", "100.005", "20.005");
    let preview = svc.preview(1, &payload).await.unwrap();
    assert_eq!(preview.total, "135.02");
    let (a, b) = tokio::join!(
        svc.create(1, "expense-retry-key-1", payload.clone()),
        svc.create(1, "expense-retry-key-1", payload)
    );
    let a = a.unwrap();
    let b = b.unwrap();
    assert_eq!(a.id, b.id);
    assert_eq!(a.total, "135.02");
    assert!(matches!(
        svc.create(1, "expense-retry-key-1", input("OTHER", "1", "0"))
            .await,
        Err(CoreError::Conflict)
    ));
    let mut second = input("literal%_", "0", "12.34");
    second.provider = "Otro proveedor".into();
    second.expense_date = chrono::NaiveDate::from_ymd_opt(2026, 10, 25).unwrap();
    svc.create(1, "expense-create-key-2", second).await.unwrap();
    let mut third = input("null", "2", "0");
    third.provider = "Tercero".into();
    svc.create(1, "expense-create-key-3", third).await.unwrap();
    let page = svc
        .list(
            2,
            &ExpenseFilter {
                page_size: Some(1),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(page.items.len(), 1);
    assert!(page.has_more);
    assert_eq!(page.summary.count, 3);
    assert_eq!(page.summary.total, "149.66");
    assert_eq!(
        svc.list(
            1,
            &ExpenseFilter {
                search: Some("null".into()),
                ..Default::default()
            }
        )
        .await
        .unwrap()
        .summary
        .count,
        1
    );
    assert_eq!(
        svc.list(
            1,
            &ExpenseFilter {
                search: Some("' OR 1=1 --".into()),
                ..Default::default()
            }
        )
        .await
        .unwrap()
        .summary
        .count,
        0
    );
    assert_eq!(
        svc.list(
            1,
            &ExpenseFilter {
                search: Some("literal%_".into()),
                ..Default::default()
            }
        )
        .await
        .unwrap()
        .summary
        .count,
        1
    );
    let day = chrono::NaiveDate::from_ymd_opt(2026, 10, 24).unwrap();
    let filtered = svc
        .list(
            1,
            &ExpenseFilter {
                date_from: Some(day),
                date_to: Some(day),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(filtered.summary.count, 2);
    assert_eq!(filtered.summary.total, "137.32");
    assert!(matches!(
        svc.list(
            1,
            &ExpenseFilter {
                page: Some(0),
                ..Default::default()
            }
        )
        .await,
        Err(CoreError::Invalid)
    ));
    let changed = input("FACT-001", "200", "10");
    assert!(matches!(
        svc.replace(2, a.id, 1, changed.clone()).await,
        Err(CoreError::Forbidden)
    ));
    let (u, v) = tokio::join!(
        svc.replace(1, a.id, 1, changed.clone()),
        svc.replace(1, a.id, 1, changed)
    );
    assert!(u.is_ok() ^ v.is_ok());
    let stale = if let Err(e) = u { e } else { v.unwrap_err() };
    assert!(matches!(stale, CoreError::PreconditionFailed));
    assert_eq!(svc.detail(2, a.id).await.unwrap().total, "240.00");
    assert!(matches!(
        svc.delete(2, a.id).await,
        Err(CoreError::Forbidden)
    ));
    sqlx::query("INSERT INTO role_permissions (role_id,module_id,scope_hash,scope_key,capability) VALUES (2,'gastos',UNHEX(SHA2('registros',256)),'registros','editar')").execute(&pool).await.unwrap();
    assert!(matches!(
        svc.delete(2, a.id).await,
        Err(CoreError::Forbidden)
    ));
    svc.delete(1, a.id).await.unwrap();
    assert!(matches!(
        svc.detail(1, a.id).await,
        Err(CoreError::NotFound)
    ));
    assert!(matches!(
        svc.replace(1, a.id, 2, input("NO", "1", "0")).await,
        Err(CoreError::NotFound)
    ));
    assert!(matches!(
        svc.delete(1, a.id).await,
        Err(CoreError::NotFound)
    ));
    let retained:u64=sqlx::query_scalar("SELECT COUNT(*) FROM gastos_records WHERE id=? AND deleted_at IS NOT NULL AND total=240.00").bind(a.id).fetch_one(&pool).await.unwrap();
    assert_eq!(retained, 1);
    assert_eq!(
        svc.list(1, &ExpenseFilter::default())
            .await
            .unwrap()
            .summary
            .count,
        2
    );
    let events:Vec<(String,u64)>=sqlx::query_as("SELECT action,COUNT(*) FROM audit_events WHERE module_id='gastos' GROUP BY action ORDER BY action").fetch_all(&pool).await.unwrap();
    assert_eq!(
        events,
        vec![
            ("expense.create".into(), 3),
            ("expense.delete".into(), 1),
            ("expense.update".into(), 1)
        ]
    );
    for _ in 0..120 {
        svc.consume_request(2).await.unwrap();
    }
    assert!(matches!(
        svc.consume_request(2).await,
        Err(CoreError::Overloaded)
    ));
    sqlx::query("UPDATE gastos_actor_limits SET window_start=UTC_TIMESTAMP(6)-INTERVAL 2 MINUTE WHERE actor_id=2").execute(&pool).await.unwrap();
    svc.consume_request(2).await.unwrap();
    core.admin()
        .set_module_enabled(1, "gastos", false)
        .await
        .unwrap();
    assert!(matches!(
        svc.list(1, &ExpenseFilter::default()).await,
        Err(CoreError::NotFound)
    ));
    svc.synchronize_release(false).await.unwrap();
    assert!(matches!(
        core.admin().set_module_enabled(1, "gastos", true).await,
        Err(CoreError::Forbidden)
    ));
}
