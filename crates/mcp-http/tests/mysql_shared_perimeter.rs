use axum::{
    Router,
    body::Body,
    extract::ConnectInfo,
    http::{Request, StatusCode},
    middleware,
    routing::get,
};
use bastion_core::Database;
use bastion_mcp_http::{DraftPerimeter, perimeter_guard};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use sqlx::mysql::MySqlPoolOptions;
use std::{net::SocketAddr, time::Duration};
use tower::ServiceExt;

#[tokio::test]
async fn mysql57_ip_limit_is_shared_across_instances() {
    let Ok(url) = std::env::var("BASTION_MCP_TEST_URL") else {
        return;
    };
    let pool = MySqlPoolOptions::new()
        .max_connections(5)
        .connect(&url)
        .await
        .unwrap();
    Database::from_test_pool(pool.clone())
        .migrate()
        .await
        .unwrap();
    let make_router = || {
        let guard = DraftPerimeter::new(vec![], Duration::from_secs(60), 20, 100)
            .unwrap()
            .with_shared(pool.clone(), [42; 32], 60, 2)
            .unwrap();
        Router::new()
            .route("/", get(|| async { "ok" }))
            .layer(middleware::from_fn_with_state(guard, perimeter_guard))
    };
    let make_request = |ip: &str| {
        let mut request = Request::new(Body::empty());
        request
            .extensions_mut()
            .insert(ConnectInfo(SocketAddr::new(ip.parse().unwrap(), 1234)));
        request
    };
    let a = make_router();
    let b = make_router();
    assert_eq!(
        a.clone()
            .oneshot(make_request("192.0.2.70"))
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        b.clone()
            .oneshot(make_request("::ffff:192.0.2.70"))
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        a.clone()
            .oneshot(make_request("192.0.2.70"))
            .await
            .unwrap()
            .status(),
        StatusCode::TOO_MANY_REQUESTS
    );
    assert_eq!(
        b.clone()
            .oneshot(make_request("192.0.2.71"))
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    let mut mac = Hmac::<Sha256>::new_from_slice(&[42; 32]).unwrap();
    mac.update(b"bastion/mcp/perimeter-ip/v1\0");
    mac.update(b"192.0.2.70");
    let hash = mac.finalize().into_bytes();
    sqlx::query("UPDATE oauth_perimeter_quotas SET window_start=UTC_TIMESTAMP(6)-INTERVAL 61 SECOND WHERE client_hash=?")
        .bind(hash.as_slice()).execute(&pool).await.unwrap();
    let (one, two, three) = tokio::join!(
        a.clone().oneshot(make_request("192.0.2.72")),
        b.clone().oneshot(make_request("192.0.2.72")),
        b.clone().oneshot(make_request("192.0.2.72"))
    );
    let mut statuses = [
        one.unwrap().status().as_u16(),
        two.unwrap().status().as_u16(),
        three.unwrap().status().as_u16(),
    ];
    statuses.sort();
    assert_eq!(statuses, [200, 200, 429]);
    assert_eq!(
        a.clone()
            .oneshot(make_request("192.0.2.70"))
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    pool.close().await;
    assert_eq!(
        a.oneshot(make_request("192.0.2.73"))
            .await
            .unwrap()
            .status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
}
