use bastion_contabilidad::*;
use bastion_core::{CoreError, CoreService, Database};
use bastion_gastos::{ExpenseInput, ExpenseService};
use sqlx::mysql::MySqlPoolOptions;

fn entry(date: &str, debit: &str, credit: &str, amount: &str) -> EntryInput {
    serde_json::from_value(serde_json::json!({"date":date,"description":"Registro sintético","reference":"TEST","kind":"manual","source":null,"lines":[{"account":debit,"debit":amount,"credit":"0"},{"account":credit,"debit":"0","credit":amount}]})).unwrap()
}
fn expense(currency: &str) -> ExpenseInput {
    serde_json::from_value(serde_json::json!({"expense_date":"2026-11-16","invoice_number":"TEST-1","provider":"Sintético","currency":currency,"taxable_amount":"100","exempt_amount":"0"})).unwrap()
}
#[tokio::test]
async fn mysql57_accounting_integrity_periods_permissions_and_sources() {
    let Ok(url) = std::env::var("BASTION_CONTABILIDAD_TEST_URL") else {
        eprintln!("Requires isolated BASTION_CONTABILIDAD_TEST_URL; CI supplies it.");
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
    assert_eq!(db_name, "bastion_contabilidad_test");
    let db = Database::from_test_pool(pool.clone());
    db.migrate().await.unwrap();
    sqlx::query(
        "INSERT INTO persons(id,full_name) VALUES(1,'Synthetic admin'),(2,'Synthetic viewer')",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO roles(id,name) VALUES(2,'Synthetic viewer')")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO users(id,person_id,role_id,email,status) VALUES(1,1,1,'admin@example.test','active'),(2,2,2,'viewer@example.test','active')").execute(&pool).await.unwrap();
    let core = CoreService::new(db.clone(), "x".repeat(32), [1; 32]).unwrap();
    let svc = AccountingService::new(db.clone(), core.admin());
    svc.synchronize_release(true).await.unwrap();
    assert!(matches!(
        core.admin()
            .set_module_enabled(1, "contabilidad", true)
            .await,
        Err(CoreError::Forbidden)
    ));
    AccountingService::migrate(&db).await.unwrap();
    AccountingService::migrate(&db).await.unwrap();
    let checksum: Vec<u8> =
        sqlx::query_scalar("SELECT checksum FROM _contabilidad_migrations WHERE version=1")
            .fetch_one(&pool)
            .await
            .unwrap();
    sqlx::query("UPDATE _contabilidad_migrations SET checksum=REPEAT('x',48) WHERE version=1")
        .execute(&pool)
        .await
        .unwrap();
    svc.synchronize_release(true).await.unwrap();
    assert!(matches!(
        core.admin()
            .set_module_enabled(1, "contabilidad", true)
            .await,
        Err(CoreError::Forbidden)
    ));
    sqlx::query("UPDATE _contabilidad_migrations SET checksum=? WHERE version=1")
        .bind(checksum)
        .execute(&pool)
        .await
        .unwrap();
    svc.synchronize_release(true).await.unwrap();
    assert!(matches!(svc.accounts(1).await, Err(CoreError::NotFound)));
    core.admin()
        .set_module_enabled(1, "contabilidad", true)
        .await
        .unwrap();
    assert!(matches!(svc.accounts(2).await, Err(CoreError::Forbidden)));
    sqlx::query("INSERT INTO role_permissions(role_id,module_id,scope_hash,scope_key,capability) VALUES(2,'contabilidad',UNHEX(SHA2('libros',256)),'libros','ver')").execute(&pool).await.unwrap();
    assert!(
        svc.accounts(2)
            .await
            .unwrap()
            .iter()
            .any(|a| a.code == "1201")
    );
    assert!(matches!(
        svc.open_period(2, "2026-11").await,
        Err(CoreError::Forbidden)
    ));
    let opening = entry("2026-11-01", "1102", "3101", "1000.01");
    assert!(matches!(
        svc.create(1, "closed", opening.clone()).await,
        Err(CoreError::Conflict)
    ));
    svc.open_period(1, "2026-11").await.unwrap();
    assert!(matches!(
        svc.create(1, "group", entry("2026-11-01", "11", "3101", "1"))
            .await,
        Err(CoreError::Invalid)
    ));
    assert!(matches!(
        svc.create(1, "reversal-1", opening.clone()).await,
        Err(CoreError::Invalid)
    ));
    let (a, b) = tokio::join!(
        svc.create(1, "opening", opening.clone()),
        svc.create(1, "opening", opening.clone())
    );
    let a = a.unwrap();
    assert_eq!(a.id, b.unwrap().id);
    assert!(matches!(
        svc.create(1, "opening", entry("2026-11-01", "1102", "3101", "1"))
            .await,
        Err(CoreError::Conflict)
    ));
    assert!(matches!(
        svc.replace(1, a.id, 99, opening.clone()).await,
        Err(CoreError::PreconditionFailed)
    ));
    assert!(matches!(
        svc.update_period(
            1,
            "2026-11",
            1,
            PeriodUpdate {
                closed: true,
                reason: "Test".into()
            }
        )
        .await,
        Err(CoreError::Conflict)
    ));
    let a = svc
        .act(1, a.id, a.version, "contabilizar", EntryAction::default())
        .await
        .unwrap();
    assert!(matches!(
        svc.replace(1, a.id, a.version, opening.clone()).await,
        Err(CoreError::Conflict)
    ));
    svc.update_period(
        1,
        "2026-11",
        1,
        PeriodUpdate {
            closed: true,
            reason: "Cierre sintético".into(),
        },
    )
    .await
    .unwrap();
    assert!(matches!(
        svc.create(1, "closed2", opening.clone()).await,
        Err(CoreError::Conflict)
    ));
    svc.open_period(1, "2026-12").await.unwrap();
    // An immutable entry in a closed period can be corrected in the next open period.
    svc.update_account(
        1,
        "1102",
        1,
        AccountUpdate {
            name: "Banco de prueba".into(),
            active: false,
        },
    )
    .await
    .unwrap();
    let reverse = svc
        .act(
            1,
            a.id,
            a.version,
            "reversar",
            EntryAction {
                date: Some("2026-12-01".parse().unwrap()),
                reason: Some("Corrección".into()),
            },
        )
        .await
        .unwrap();
    assert_eq!(reverse.reversal_of, Some(a.id));
    assert_eq!(reverse.status, "contabilizado");
    let report = svc
        .balance(
            1,
            &BalanceFilter {
                date_from: "2026-12-01".parse().unwrap(),
                date_to: "2026-12-31".parse().unwrap(),
            },
        )
        .await
        .unwrap();
    let bank = report.items.iter().find(|r| r.code == "1102").unwrap();
    assert_eq!(bank.opening, "1000.01");
    assert_eq!(bank.credit, "1000.01");
    assert_eq!(bank.balance, "0.00");
    assert!(report.balanced);
    assert!(matches!(
        svc.act(
            1,
            reverse.id,
            reverse.version,
            "reversar",
            EntryAction::default()
        )
        .await,
        Err(CoreError::Conflict)
    ));
    assert!(matches!(
        svc.create(1, "inactive", entry("2026-12-01", "1102", "3101", "1"))
            .await,
        Err(CoreError::Invalid)
    ));
    svc.update_period(
        1,
        "2026-11",
        2,
        PeriodUpdate {
            closed: false,
            reason: "Prueba de integración".into(),
        },
    )
    .await
    .unwrap();
    ExpenseService::migrate(&db).await.unwrap();
    let gastos = ExpenseService::new(db.clone(), core.admin());
    gastos.synchronize_release(true).await.unwrap();
    core.admin()
        .set_module_enabled(1, "gastos", true)
        .await
        .unwrap();
    let g = gastos
        .create(1, "accounting-gasto-001", expense("HNL"))
        .await
        .unwrap();
    assert!(matches!(
        svc.proposal(2, "gastos", g.id).await,
        Err(CoreError::Forbidden)
    ));
    let usd = gastos
        .create(1, "accounting-gasto-usd", expense("HNL"))
        .await
        .unwrap();
    // Legacy/imported currency must never be treated as HNL, even if normal CRUD rejects it.
    sqlx::query("UPDATE gastos_records SET currency='USD' WHERE id=?")
        .bind(usd.id)
        .execute(&pool)
        .await
        .unwrap();
    assert!(matches!(
        svc.proposal(1, "gastos", usd.id).await,
        Err(CoreError::Conflict)
    ));
    let proposed = svc.proposal(1, "gastos", g.id).await.unwrap();
    assert_eq!(proposed.entry.validate().unwrap(), 11500);
    let draft = svc
        .create(1, "import-gasto", proposed.entry.clone())
        .await
        .unwrap();
    assert!(matches!(
        svc.create(1, "duplicate", proposed.entry.clone()).await,
        Err(CoreError::Conflict)
    ));
    let mut changed = expense("HNL");
    changed.exempt_amount = "20".into();
    gastos.replace(1, g.id, g.version, changed).await.unwrap();
    assert_eq!(
        svc.detail(1, draft.id).await.unwrap().source_status,
        "modificado"
    );
    assert!(matches!(
        svc.act(
            1,
            draft.id,
            draft.version,
            "contabilizar",
            EntryAction::default()
        )
        .await,
        Err(CoreError::PreconditionFailed)
    ));
    svc.act(
        1,
        draft.id,
        draft.version,
        "cancelar",
        EntryAction::default(),
    )
    .await
    .unwrap();
    let fresh = svc.proposal(1, "gastos", g.id).await.unwrap();
    let draft = svc.create(1, "fresh", fresh.entry).await.unwrap();
    svc.act(
        1,
        draft.id,
        draft.version,
        "contabilizar",
        EntryAction::default(),
    )
    .await
    .unwrap();
    // Source ports preserve invoice issuance and order cost rules.
    use bastion_facturas::{InvoiceInput, InvoiceService};
    InvoiceService::migrate(&db).await.unwrap();
    let invoices = InvoiceService::new(db.clone(), core.admin());
    invoices.synchronize_release(true).await.unwrap();
    core.admin()
        .set_module_enabled(1, "facturas", true)
        .await
        .unwrap();
    core.admin().set_core_setting(1,"company.profile",serde_json::json!({"name":"Empresa sintética","rtn":"RTN","email":"test@example.test","phone":"123","address":"TEST"})).await.unwrap();
    let sar = core.admin().get_sar_settings(1).await.unwrap();
    core.admin()
        .set_sar_settings(
            1,
            bastion_core::SarSettings {
                limit_range: "000-001-01-00000001 / 000-001-01-00000005".into(),
                current_invoice: "000-001-01-00000000".into(),
                limit_date: Some("2099-12-31".parse().unwrap()),
                sar_text: "CAI sintético".into(),
            },
            &sar.version,
        )
        .await
        .unwrap();
    let invoice:InvoiceInput=serde_json::from_value(serde_json::json!({"client_name":"Cliente sintético","order_number":"TEST","invoice_date":"2026-11-16","payment_term_type":"credito","credit_days":30,"discount_type":"amount","discount_value":"0","notes":"","public_notes":"","expenses":[],"items":[{"description":"Equipo","quantity":"1","unit_price":"100","cost_price":"60","discount_type":"amount","discount_value":"0","tax_type":"isv15"}]})).unwrap();
    let inv = invoices
        .create(1, "accounting-invoice-test", invoice)
        .await
        .unwrap();
    assert!(matches!(
        svc.proposal(1, "facturas", inv.id).await,
        Err(CoreError::Conflict)
    ));
    invoices
        .mark_sent(1, inv.id, inv.version, "accounting-issue-test")
        .await
        .unwrap();
    let sale = svc.proposal(1, "facturas", inv.id).await.unwrap();
    assert_eq!(sale.entry.lines[0].debit, "115.00");
    assert_eq!(sale.entry.lines[1].credit, "100.00");
    assert_eq!(sale.entry.lines[2].credit, "15.00");
    let sale = svc.create(1, "sale", sale.entry).await.unwrap();
    svc.act(
        1,
        sale.id,
        sale.version,
        "contabilizar",
        EntryAction::default(),
    )
    .await
    .unwrap();
    use bastion_ordenes_compra::{DestinationInput, OrderInput, OrderService};
    OrderService::migrate(&db).await.unwrap();
    let orders = OrderService::new(db.clone(), core.admin());
    orders.synchronize_release(true).await.unwrap();
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
                destination: "Oficina sintética".into(),
            },
        )
        .await
        .unwrap();
    let order:OrderInput=serde_json::from_value(serde_json::json!({"n_orders":"OC-TEST","n_bill":"TEST","order_date":"2026-11-15","received_date":"2026-11-16","client_name":"Cliente","client_rtn":"","id_destination":destination.id,"comments":"","products":[{"description":"Equipo","cantidad":2,"sale_price":"200","purchase_price":"100","tracking":"","isv":true,"status":"Comprado","coments":""}],"expenses":[{"description":"Casillero","amount":"5"}]})).unwrap();
    let order = orders
        .create(1, "accounting-order-test", order)
        .await
        .unwrap();
    let purchase = svc.proposal(1, "ordenes_compra", order.id).await.unwrap();
    assert_eq!(purchase.entry.validate().unwrap(), 20500);
    assert_eq!(purchase.entry.lines[0].account, "1105");
    let purchase = svc.create(1, "purchase", purchase.entry).await.unwrap();
    svc.act(
        1,
        purchase.id,
        purchase.version,
        "contabilizar",
        EntryAction::default(),
    )
    .await
    .unwrap();
    let events: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM audit_events WHERE module_id='contabilidad'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(events > 10);
    core.admin()
        .set_module_enabled(1, "contabilidad", false)
        .await
        .unwrap();
    assert!(matches!(
        svc.detail(1, a.id).await,
        Err(CoreError::NotFound)
    ));
}
