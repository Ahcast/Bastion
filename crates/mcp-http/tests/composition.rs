use axum::{
    body::Body,
    http::{
        Request, StatusCode,
        header::{
            ACCESS_CONTROL_ALLOW_CREDENTIALS, ACCESS_CONTROL_ALLOW_ORIGIN,
            ACCESS_CONTROL_REQUEST_HEADERS, ACCESS_CONTROL_REQUEST_METHOD, ORIGIN,
        },
    },
};
use bastion_core::{CoreService, Database};
use bastion_mcp::{ActorQuota, ClientMetadata, Credential, ToolQuota};
use bastion_mcp_http::{
    ClientMetadataSource, DraftCompositionConfig, MetadataFetchError, compose_draft_router,
};
use sqlx::mysql::MySqlPoolOptions;
use std::{collections::BTreeSet, time::Duration};
use tower::ServiceExt;

#[derive(Clone)]
struct NoFetch;
impl ClientMetadataSource for NoFetch {
    async fn fetch(&self, _: &str) -> Result<ClientMetadata, MetadataFetchError> {
        Err(MetadataFetchError::Unavailable)
    }
}

#[tokio::test]
async fn mysql57_composed_draft_separates_browser_and_client_cors() {
    let Ok(url) = std::env::var("BASTION_MCP_TEST_URL") else {
        return;
    };
    let pool = MySqlPoolOptions::new()
        .max_connections(2)
        .connect(&url)
        .await
        .unwrap();
    let db = Database::from_test_pool(pool.clone());
    db.migrate().await.unwrap();
    let core = CoreService::new(db, Credential::generate().unwrap().expose(), [31; 32]).unwrap();
    let config = DraftCompositionConfig {
        resource: "https://api.example.invalid/mcp".into(),
        issuer: "https://api.example.invalid/oauth".into(),
        authorization_endpoint: "https://api.example.invalid/oauth/authorize".into(),
        token_endpoint: "https://api.example.invalid/oauth/token".into(),
        frontend_origin: "https://app.example.invalid".into(),
        client_origins: BTreeSet::from(["https://client.example.invalid".into()]),
        actor_quota: ActorQuota {
            window_seconds: 60,
            user_total: 20,
            client_total: 20,
        },
        tool_quota: ToolQuota {
            user_tool: 10,
            client_tool: 10,
        },
        max_concurrent: 4,
        max_metadata_fetches: 2,
        request_timeout: Duration::from_secs(10),
        perimeter: bastion_mcp_http::DraftPerimeter::new(vec![], Duration::from_secs(60), 100, 100)
            .unwrap()
            .with_shared(pool.clone(), [37; 32], 60, 100)
            .unwrap(),
    };
    let router = compose_draft_router(core, config, NoFetch)
        .unwrap()
        .layer(axum::Extension(axum::extract::ConnectInfo(
            "192.0.2.1:1234".parse::<std::net::SocketAddr>().unwrap(),
        )));
    let metadata = Request::builder()
        .uri("/.well-known/oauth-protected-resource/mcp")
        .header(ORIGIN, "https://other.example.invalid")
        .body(Body::empty())
        .unwrap();
    let metadata = router.clone().oneshot(metadata).await.unwrap();
    assert_eq!(metadata.status(), StatusCode::OK);
    assert_eq!(metadata.headers()[ACCESS_CONTROL_ALLOW_ORIGIN], "*");
    let preflight = |path: &str, origin: &str, headers: &str| {
        Request::builder()
            .method("OPTIONS")
            .uri(path)
            .header(ORIGIN, origin)
            .header(ACCESS_CONTROL_REQUEST_METHOD, "POST")
            .header(ACCESS_CONTROL_REQUEST_HEADERS, headers)
            .body(Body::empty())
            .unwrap()
    };
    let browser = router
        .clone()
        .oneshot(preflight(
            "/oauth/clients/mine",
            "https://app.example.invalid",
            "content-type,x-csrf-token",
        ))
        .await
        .unwrap();
    assert_eq!(
        browser.headers()[ACCESS_CONTROL_ALLOW_ORIGIN],
        "https://app.example.invalid"
    );
    assert_eq!(browser.headers()[ACCESS_CONTROL_ALLOW_CREDENTIALS], "true");
    let evil = router
        .clone()
        .oneshot(preflight(
            "/oauth/clients/mine",
            "https://evil.invalid",
            "content-type,x-csrf-token",
        ))
        .await
        .unwrap();
    assert!(!evil.headers().contains_key(ACCESS_CONTROL_ALLOW_ORIGIN));
    let client = router
        .oneshot(preflight(
            "/mcp",
            "https://client.example.invalid",
            "authorization,content-type,mcp-method",
        ))
        .await
        .unwrap();
    assert_eq!(
        client.headers()[ACCESS_CONTROL_ALLOW_ORIGIN],
        "https://client.example.invalid"
    );
    assert!(
        !client
            .headers()
            .contains_key(ACCESS_CONTROL_ALLOW_CREDENTIALS)
    );
}
