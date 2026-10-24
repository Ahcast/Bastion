use super::*;
use axum::body::{Body, to_bytes};
use bastion_core::Database;
use bastion_cotizaciones::{QuoteInput, QuoteService};
use bastion_ordenes_compra::OrderService;
use sqlx::mysql::MySqlPoolOptions;
use tower::ServiceExt;
fn request(
    method: &str,
    path: &str,
    token: Option<&str>,
    csrf: bool,
    origin: &str,
    body: &str,
    version: Option<u64>,
) -> axum::http::Request<Body> {
    let mut builder = axum::http::Request::builder()
        .method(method)
        .uri(path)
        .header("origin", origin)
        .header("content-type", "application/json");
    if let Some(token) = token {
        builder = builder.header(
            "cookie",
            format!("__Host-bastion_session={token}; __Host-bastion_csrf=synthetic-csrf"),
        );
    }
    if csrf {
        builder = builder.header("x-csrf-token", "synthetic-csrf");
    }
    if let Some(version) = version {
        builder = builder.header("if-match", format!("\"{version}\""));
    }
    builder = builder.header("idempotency-key", "synthetic-http-convert-0001");
    let mut req = builder.body(Body::from(body.to_owned())).unwrap();
    req.extensions_mut()
        .insert(ConnectInfo("127.0.0.1:5010".parse::<SocketAddr>().unwrap()));
    req
}
#[tokio::test]
async fn mysql57_conversion_http_auth_csrf_origin_permissions_version_and_save() {
    let Ok(url) = std::env::var("BASTION_CONVERSION_HTTP_TEST_URL") else {
        eprintln!(
            "Requires disposable BASTION_CONVERSION_HTTP_TEST_URL; not executed locally without it."
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
    assert_eq!(dbname, "bastion_conversion_http_test");
    let database = Database::from_test_pool(pool.clone());
    database.migrate().await.unwrap();
    QuoteService::migrate(&database).await.unwrap();
    OrderService::migrate(&database).await.unwrap();
    sqlx::query("INSERT INTO persons (id,full_name) VALUES (1,'HTTP Admin'),(2,'HTTP Viewer')")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO roles (id,name) VALUES (2,'HTTP Viewer')")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO users (id,person_id,role_id,email,status) VALUES (1,1,1,'admin@example.test','active'),(2,2,2,'viewer@example.test','active')").execute(&pool).await.unwrap();
    for (id, token) in [(1_u64, "synthetic-admin"), (2, "synthetic-viewer")] {
        sqlx::query("INSERT INTO sessions (user_id,token_hash,csrf_hash,ip_address,started_at,last_activity_at,last_totp_at,expires_at) VALUES (?,UNHEX(SHA2(?,256)),UNHEX(SHA2('synthetic-csrf',256)),'127.0.0.1',UTC_TIMESTAMP(6),UTC_TIMESTAMP(6),UTC_TIMESTAMP(6),UTC_TIMESTAMP(6)+INTERVAL 1 HOUR)").bind(id).bind(token).execute(&pool).await.unwrap();
    }
    let core = CoreService::new(database.clone(), "x".repeat(32), [1; 32]).unwrap();
    let quotes = QuoteService::new(database.clone(), core.admin());
    let orders = OrderService::new(database, core.admin());
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
    sqlx::query("INSERT INTO ordenes_destinations (id,destination,name_key) VALUES (1,'Destino sintético','destino sintético')").execute(&pool).await.unwrap();
    let input:QuoteInput=serde_json::from_value(serde_json::json!({"client_name":"HTTP Cliente","order_number":"OC-HTTP","quote_date":"2026-10-23","valid_days":30,"status":"Aprobada","notes":"","terms":"","discount_type":"amount","discount_value":"1","items":[{"description":"Equipo","quantity":"1.25","unit_price":"100","discount_type":"amount","discount_value":"0","tax_type":"isv18"}]})).unwrap();
    let source = quotes
        .create(1, "synthetic-http-source-0001", input)
        .await
        .unwrap();
    let app = router(
        core.clone(),
        "https://example.test".into(),
        vec![],
        std::path::PathBuf::from("unused.csv"),
    )
    .unwrap();
    let prep = format!("/api/v1/cotizaciones/{}/order-preparation", source.id);
    let save = format!("/api/v1/cotizaciones/{}/order", source.id);
    for (token, status) in [
        (None, StatusCode::UNAUTHORIZED),
        (Some("synthetic-viewer"), StatusCode::FORBIDDEN),
    ] {
        assert_eq!(
            app.clone()
                .oneshot(request(
                    "GET",
                    &prep,
                    token,
                    false,
                    "https://example.test",
                    "",
                    Some(source.version)
                ))
                .await
                .unwrap()
                .status(),
            status
        );
    }
    assert_eq!(
        app.clone()
            .oneshot(request(
                "GET",
                &prep,
                Some("synthetic-admin"),
                false,
                "https://example.test",
                "",
                None
            ))
            .await
            .unwrap()
            .status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        app.clone()
            .oneshot(request(
                "GET",
                &prep,
                Some("synthetic-admin"),
                false,
                "https://example.test",
                "",
                Some(source.version + 1)
            ))
            .await
            .unwrap()
            .status(),
        StatusCode::PRECONDITION_FAILED
    );
    let response = app
        .clone()
        .oneshot(request(
            "GET",
            &prep,
            Some("synthetic-admin"),
            false,
            "https://example.test",
            "",
            Some(source.version),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let seed: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 1024 * 1024).await.unwrap())
            .unwrap();
    let mut data = seed["data"].clone();
    data["id_destination"] = serde_json::json!(1);
    let payload = serde_json::to_string(&data).unwrap();
    assert_eq!(
        app.clone()
            .oneshot(request(
                "POST",
                &save,
                Some("synthetic-admin"),
                false,
                "https://example.test",
                &payload,
                Some(source.version)
            ))
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        app.clone()
            .oneshot(request(
                "POST",
                &save,
                Some("synthetic-admin"),
                true,
                "https://untrusted.example.test",
                &payload,
                Some(source.version)
            ))
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
    let response = app
        .clone()
        .oneshot(request(
            "POST",
            &save,
            Some("synthetic-admin"),
            true,
            "https://example.test",
            &payload,
            Some(source.version),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let order: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 1024 * 1024).await.unwrap())
            .unwrap();
    assert_eq!(order["data"]["products"][0]["cantidad"], "1.25");
    assert_eq!(order["source_quote"]["version"], source.version);
    let retry = app
        .clone()
        .oneshot(request(
            "POST",
            &save,
            Some("synthetic-admin"),
            true,
            "https://example.test",
            &payload,
            Some(source.version),
        ))
        .await
        .unwrap();
    assert_eq!(retry.status(), StatusCode::OK);
    let retried: serde_json::Value =
        serde_json::from_slice(&to_bytes(retry.into_body(), 1024 * 1024).await.unwrap()).unwrap();
    assert_eq!(retried, order);

    // A valid quote can exceed 1 MiB once products and approved terms are copied.
    let mut large = source.data.clone();
    large.order_number = "OC-HTTP-GRANDE".into();
    large.items = vec![large.items[0].clone(); 200];
    for row in &mut large.items {
        row.description = "X".repeat(2800);
    }
    let large = quotes
        .create(1, "synthetic-http-source-large-0001", large)
        .await
        .unwrap();
    let large_prep = format!("/api/v1/cotizaciones/{}/order-preparation", large.id);
    let response = app
        .clone()
        .oneshot(request(
            "GET",
            &large_prep,
            Some("synthetic-admin"),
            false,
            "https://example.test",
            "",
            Some(large.version),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let mut large_seed: serde_json::Value = serde_json::from_slice(
        &to_bytes(response.into_body(), 3 * 1024 * 1024)
            .await
            .unwrap(),
    )
    .unwrap();
    large_seed["data"]["id_destination"] = serde_json::json!(1);
    let payload = serde_json::to_string(&large_seed["data"]).unwrap();
    assert!(payload.len() > 1024 * 1024);
    let large_save = format!("/api/v1/cotizaciones/{}/order", large.id);
    let mut req = request(
        "POST",
        &large_save,
        Some("synthetic-admin"),
        true,
        "https://example.test",
        &payload,
        Some(large.version),
    );
    req.headers_mut().insert(
        "idempotency-key",
        HeaderValue::from_static("synthetic-http-large-convert-0001"),
    );
    let response = app.clone().oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let converted: serde_json::Value = serde_json::from_slice(
        &to_bytes(response.into_body(), 3 * 1024 * 1024)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(converted["data"]["products"].as_array().unwrap().len(), 200);
    assert_eq!(
        converted["calculation"]["totals"]["total"],
        large.calculation.totals.total
    );
    core.admin()
        .set_module_enabled(1, "ordenes_compra", false)
        .await
        .unwrap();
    assert_eq!(
        app.oneshot(request(
            "POST",
            &save,
            Some("synthetic-admin"),
            true,
            "https://example.test",
            &payload,
            Some(source.version)
        ))
        .await
        .unwrap()
        .status(),
        StatusCode::NOT_FOUND
    );
}
