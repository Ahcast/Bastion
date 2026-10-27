use super::*;
use axum::body::{Body, to_bytes};
use bastion_core::Database;
use sqlx::mysql::MySqlPoolOptions;
use tower::ServiceExt;

async fn fetch_report(app: &Router, path: &str, session: Option<&str>) -> Response {
    let mut builder = axum::http::Request::builder()
        .uri(path)
        .header("origin", "https://example.test");
    if let Some(session) = session {
        builder = builder.header("cookie", format!("__Host-bastion_session={session}"));
    }
    let mut request = builder.body(Body::empty()).unwrap();
    request
        .extensions_mut()
        .insert(ConnectInfo("127.0.0.1:5010".parse::<SocketAddr>().unwrap()));
    app.clone().oneshot(request).await.unwrap()
}

async fn json(response: Response) -> serde_json::Value {
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = to_bytes(response.into_body(), 2 * 1024 * 1024)
        .await
        .unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

#[tokio::test]
async fn mysql57_reports_http_permissions_totals_soft_delete_export_audit_and_limits() {
    let Ok(url) = std::env::var("BASTION_REPORTES_HTTP_TEST_URL") else {
        eprintln!("Requires isolated BASTION_REPORTES_HTTP_TEST_URL; CI supplies it.");
        return;
    };
    let pool = MySqlPoolOptions::new()
        .max_connections(5)
        .connect(&url)
        .await
        .unwrap();
    let name: String = sqlx::query_scalar("SELECT DATABASE()")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(name, "bastion_reportes_http_test");
    let db = Database::from_test_pool(pool.clone());
    db.migrate().await.unwrap();
    bastion_ordenes_compra::OrderService::migrate(&db)
        .await
        .unwrap();
    bastion_facturas::InvoiceService::migrate(&db)
        .await
        .unwrap();
    bastion_gastos::ExpenseService::migrate(&db).await.unwrap();
    bastion_reportes::ReportService::migrate(&db).await.unwrap();
    bastion_reportes::ReportService::migrate(&db).await.unwrap();
    sqlx::query("INSERT INTO persons (id,full_name) VALUES (1,'Report admin'),(2,'Report viewer')")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO roles (id,name) VALUES (2,'Report viewer')")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO users (id,person_id,role_id,email,status) VALUES (1,1,1,'admin@example.test','active'),(2,2,2,'viewer@example.test','active')").execute(&pool).await.unwrap();
    for (user, token) in [
        (1_u64, "synthetic-report-admin"),
        (2, "synthetic-report-viewer"),
    ] {
        sqlx::query("INSERT INTO sessions (user_id,token_hash,csrf_hash,ip_address,started_at,last_activity_at,last_totp_at,expires_at) VALUES (?,UNHEX(SHA2(?,256)),UNHEX(SHA2('report-csrf',256)),'127.0.0.1',UTC_TIMESTAMP(6),UTC_TIMESTAMP(6),UTC_TIMESTAMP(6),UTC_TIMESTAMP(6)+INTERVAL 1 HOUR)")
            .bind(user).bind(token).execute(&pool).await.unwrap();
    }
    let core = CoreService::new(db.clone(), "x".repeat(32), [1; 32]).unwrap();
    bastion_ordenes_compra::OrderService::new(db.clone(), core.admin())
        .synchronize_release(true)
        .await
        .unwrap();
    bastion_facturas::InvoiceService::new(db.clone(), core.admin())
        .synchronize_release(true)
        .await
        .unwrap();
    bastion_gastos::ExpenseService::new(db.clone(), core.admin())
        .synchronize_release(true)
        .await
        .unwrap();
    bastion_reportes::ReportService::new(db.clone(), core.admin())
        .synchronize_release(true)
        .await
        .unwrap();
    for module in ["ordenes_compra", "facturas", "gastos"] {
        core.admin()
            .set_module_enabled(1, module, true)
            .await
            .unwrap();
    }
    let app = crate::router(
        core.clone(),
        "https://example.test".into(),
        Vec::new(),
        "/unused".into(),
    )
    .unwrap();
    let admin = Some("synthetic-report-admin");
    let viewer = Some("synthetic-report-viewer");
    let dates = "date_from=2026-10-01&date_to=2026-10-31";
    let orders = format!("/api/v1/reportes/ordenes?{dates}");
    let expenses = format!("/api/v1/reportes/gastos?{dates}");
    let invoices = format!("/api/v1/reportes/facturas?{dates}");
    assert_eq!(
        fetch_report(&app, &orders, None).await.status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        fetch_report(&app, &orders, admin).await.status(),
        StatusCode::NOT_FOUND
    );
    core.admin()
        .set_module_enabled(1, "reportes", true)
        .await
        .unwrap();
    assert_eq!(
        fetch_report(&app, &expenses, viewer).await.status(),
        StatusCode::FORBIDDEN
    );
    sqlx::query("INSERT INTO role_permissions (role_id,module_id,scope_hash,scope_key,capability) VALUES (2,'reportes',UNHEX(SHA2('gastos',256)),'gastos','ver')").execute(&pool).await.unwrap();
    // Report permission alone cannot reveal the source domain.
    assert_eq!(
        fetch_report(&app, &expenses, viewer).await.status(),
        StatusCode::FORBIDDEN
    );
    sqlx::query("INSERT INTO role_permissions (role_id,module_id,scope_hash,scope_key,capability) VALUES (2,'gastos',UNHEX(SHA2('registros',256)),'registros','ver')").execute(&pool).await.unwrap();
    assert!(
        json(fetch_report(&app, &expenses, viewer).await).await["items"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        fetch_report(&app, &orders, viewer).await.status(),
        StatusCode::FORBIDDEN
    );

    sqlx::query("INSERT INTO ordenes_destinations (id,destination,name_key) VALUES (1,'Synthetic','synthetic')").execute(&pool).await.unwrap();
    let calculation = serde_json::json!({"totals":{"subtotal":"100.00","tax":"15.00","total":"115.00","products_investment":"50.00","other_expenses":"5.00","investment_total":"55.00","profit":"60.00"}});
    for (date, status) in [
        ("2026-10-01", "Pendiente"),
        ("2026-10-31", "Listo"),
        ("2026-11-01", "Pendiente"),
    ] {
        sqlx::query("INSERT INTO ordenes_orders (status,n_orders,n_bill,client_name,order_date,id_destination,total,data_json,calculation_json,created_by,updated_by,created_at,updated_at) VALUES (?,'OC-001','','=1+1 <script>',?,1,115.00,'{}',?,1,1,UTC_TIMESTAMP(6),UTC_TIMESTAMP(6))")
            .bind(status).bind(date).bind(&calculation).execute(&pool).await.unwrap();
    }
    let report = json(fetch_report(&app, &format!("{orders}&page_size=1"), admin).await).await;
    assert_eq!(report["totals"]["records"], 2);
    assert_eq!(report["totals"]["total"], "230.00");
    assert_eq!(report["items"].as_array().unwrap().len(), 1);
    assert_eq!(report["has_more"], true);
    assert_eq!(
        json(fetch_report(&app, &format!("{orders}&status=Pendiente"), admin).await).await["totals"]
            ["records"],
        1
    );
    assert_eq!(
        json(fetch_report(&app, &format!("{orders}&search=absent"), admin).await).await["totals"]["records"],
        0
    );
    let invoice_calc =
        serde_json::json!({"totals":{"total":"115.00"},"paid_total":"0.00","balance":"115.00"});
    sqlx::query("INSERT INTO facturas_drafts (client_name,order_number,invoice_date,data_json,calculation_json,created_by,updated_by,created_at,updated_at) VALUES ('Synthetic','OC-001','2026-10-31','{}',?,1,1,UTC_TIMESTAMP(6),UTC_TIMESTAMP(6))")
        .bind(invoice_calc).execute(&pool).await.unwrap();
    let invoice_report = json(fetch_report(&app, &invoices, admin).await).await;
    assert_eq!(invoice_report["totals"]["records"], 1);
    assert_eq!(invoice_report["totals"]["balance"], "115.00");

    for (number, deleted) in [("LIVE", false), ("DELETED", true)] {
        sqlx::query("INSERT INTO gastos_records (expense_date,invoice_number,provider,currency,taxable_amount,exempt_amount,isv_15,total,created_by,updated_by,created_at,updated_at,deleted_at) VALUES ('2026-10-31',?,'Supplier','HNL',100.00,20.00,15.00,135.00,1,1,UTC_TIMESTAMP(6),UTC_TIMESTAMP(6),IF(?,UTC_TIMESTAMP(6),NULL))")
            .bind(number).bind(deleted).execute(&pool).await.unwrap();
    }
    let expense_report = json(fetch_report(&app, &expenses, admin).await).await;
    assert_eq!(expense_report["totals"][0]["records"], 1);
    assert_eq!(expense_report["totals"][0]["total"], "135.00");
    assert_eq!(expense_report["items"][0]["invoice_number"], "LIVE");
    for kind in ["ordenes", "facturas", "gastos"] {
        let response = fetch_report(
            &app,
            &format!("/api/v1/reportes/{kind}/export?{dates}"),
            admin,
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        assert!(
            response.headers()["content-disposition"]
                .to_str()
                .unwrap()
                .contains(".xls")
        );
        assert!(
            response.headers()["content-type"]
                .to_str()
                .unwrap()
                .contains("application/vnd.ms-excel")
        );
        let xml = String::from_utf8(
            to_bytes(response.into_body(), 2 * 1024 * 1024)
                .await
                .unwrap()
                .to_vec(),
        )
        .unwrap();
        assert!(xml.contains("ss:Name=\"Resumen\"") && xml.contains("ss:Name=\"Detalle\""));
        assert!(
            !xml.contains("ss:Formula") && !xml.contains("<script>") && !xml.contains("DELETED")
        );
    }
    for path in [
        "/api/v1/reportes/ordenes?date_from=2025-01-01&date_to=2026-10-31",
        "/api/v1/reportes/facturas?date_from=2026-10-31&date_to=2026-10-01",
        "/api/v1/reportes/ordenes?date_from=2026-10-01&date_to=2026-10-31&status=unknown",
    ] {
        assert_eq!(
            fetch_report(&app, path, admin).await.status(),
            StatusCode::BAD_REQUEST
        );
    }
    for _ in 0..10 {
        sqlx::query("INSERT INTO gastos_records (expense_date,invoice_number,provider,currency,taxable_amount,exempt_amount,isv_15,total,created_by,updated_by,created_at,updated_at) SELECT expense_date,invoice_number,provider,currency,taxable_amount,exempt_amount,isv_15,total,created_by,updated_by,created_at,updated_at FROM gastos_records WHERE deleted_at IS NULL")
            .execute(&pool).await.unwrap();
    }
    assert_eq!(
        fetch_report(
            &app,
            &format!("/api/v1/reportes/gastos/export?{dates}"),
            admin
        )
        .await
        .status(),
        StatusCode::PAYLOAD_TOO_LARGE
    );
    let audit: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM audit_events WHERE module_id='reportes' AND action='report.export'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(audit, 3);
    sqlx::query("UPDATE reportes_actor_limits SET requests=30 WHERE actor_id=1")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        fetch_report(&app, &orders, admin).await.status(),
        StatusCode::TOO_MANY_REQUESTS
    );
    core.admin()
        .set_module_enabled(1, "reportes", false)
        .await
        .unwrap();
    assert_eq!(
        fetch_report(&app, &orders, admin).await.status(),
        StatusCode::NOT_FOUND
    );
}
