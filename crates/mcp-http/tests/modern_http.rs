use std::{collections::BTreeSet, sync::Arc, time::Duration};

use axum::{
    body::{Body, to_bytes},
    http::{
        Request, StatusCode,
        header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE, ORIGIN, RETRY_AFTER},
    },
};
use bastion_core::Database;
use bastion_mcp::{
    ActorQuota, Credential, CredentialKind, ExchangePolicy, MysqlQuota, MysqlTokenExchange,
    ToolDescriptor, ToolQuota, ToolRegistry, ToolRisk,
};
use bastion_mcp_http::{DraftMcpState, draft_mcp_router};
use serde_json::json;
use sqlx::{MySql, Transaction, mysql::MySqlPoolOptions};
use tower::ServiceExt;

#[derive(Clone)]
struct FixturePolicy;
impl ExchangePolicy for FixturePolicy {
    async fn authorize(
        &self,
        _: &mut Transaction<'_, MySql>,
        _: u64,
        _: u64,
        _: &BTreeSet<String>,
    ) -> Result<bool, sqlx::Error> {
        Ok(true)
    }
}

#[tokio::test]
async fn mysql57_mcp_http_authenticates_filters_limits_and_audits() {
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
    let person = sqlx::query("INSERT INTO persons (full_name) VALUES ('MCP HTTP test')")
        .execute(&pool)
        .await
        .unwrap()
        .last_insert_id();
    let role = sqlx::query("INSERT INTO roles (name) VALUES ('MCP HTTP role')")
        .execute(&pool)
        .await
        .unwrap()
        .last_insert_id();
    let user=sqlx::query("INSERT INTO users (person_id,role_id,email,status) VALUES (?,?,'mcp-http@example.invalid','active')").bind(person).bind(role).execute(&pool).await.unwrap().last_insert_id();
    let client=sqlx::query("INSERT INTO oauth_clients (client_id,client_id_hash,display_name,metadata_hash,approved_metadata_hash,enabled,token_auth_method,created_at,updated_at) VALUES ('mcp-http-client',UNHEX(SHA2('mcp-http-client',256)),'MCP HTTP fixture',UNHEX(SHA2('metadata',256)),UNHEX(SHA2('metadata',256)),1,'none',UTC_TIMESTAMP(6),UTC_TIMESTAMP(6))").execute(&pool).await.unwrap().last_insert_id();
    sqlx::query("INSERT INTO oauth_client_scopes (client_id,scope_name) VALUES (?,'read')")
        .bind(client)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO module_registry (module_id,display_name,included_in_release,environment_allowed,enabled) VALUES ('mcp_http_fixture','HTTP fixture',1,1,1)").execute(&pool).await.unwrap();
    let grant=sqlx::query("INSERT INTO oauth_grants (user_id,client_id,resource_uri,scopes_json,issued_at,expires_at) VALUES (?,?,'https://example.invalid/mcp',JSON_ARRAY('read'),UTC_TIMESTAMP(6)-INTERVAL 1 SECOND,UTC_TIMESTAMP(6)+INTERVAL 1 DAY)").bind(user).bind(client).execute(&pool).await.unwrap().last_insert_id();
    let access = Credential::generate().unwrap();
    sqlx::query("INSERT INTO oauth_access_tokens (grant_id,token_hash,scopes_json,issued_at,expires_at) VALUES (?,?,JSON_ARRAY('read'),UTC_TIMESTAMP(6)-INTERVAL 1 SECOND,UTC_TIMESTAMP(6)+INTERVAL 10 MINUTE)").bind(grant).bind(access.digest(CredentialKind::AccessToken).as_slice()).execute(&pool).await.unwrap();
    let scope = BTreeSet::from(["read".to_owned()]);
    let tool = ToolDescriptor {
        name: "fixture.read".into(),
        module_id: "mcp_http_fixture".into(),
        description: "Fixture".into(),
        required_scope: "read".into(),
        input_schema: json!({"type":"object"}),
        output_schema: None,
        risk: ToolRisk::ReadOnly,
        idempotent: true,
        max_response_bytes: 1024,
        max_duration_ms: 1000,
    };
    let registry = Arc::new(ToolRegistry::new(vec![tool], &scope).unwrap());
    let quota = MysqlQuota::new(
        pool.clone(),
        ActorQuota {
            window_seconds: 60,
            user_total: 4,
            client_total: 4,
        },
    )
    .unwrap();
    let state = DraftMcpState::new(
        pool.clone(),
        MysqlTokenExchange::new(pool.clone(), FixturePolicy),
        "https://example.invalid/mcp".into(),
        "https://example.invalid/.well-known/oauth-protected-resource/mcp".into(),
        BTreeSet::new(),
        registry,
        quota,
        ToolQuota {
            user_tool: 2,
            client_tool: 2,
        },
        4,
        Duration::from_secs(10),
    )
    .unwrap();
    let router = draft_mcp_router(state);
    let body = |method: &str| {
        json!({"jsonrpc":"2.0","id":7,"method":method,"params":{"_meta":{"io.modelcontextprotocol/protocolVersion":"2026-07-28","io.modelcontextprotocol/clientCapabilities":{}}}}).to_string()
    };
    let request = |method: &str| {
        Request::builder()
            .method("POST")
            .uri("/mcp")
            .header(CONTENT_TYPE, "application/json")
            .header(ACCEPT, "application/json, text/event-stream")
            .header("mcp-protocol-version", "2026-07-28")
            .header("mcp-method", method)
            .header(AUTHORIZATION, format!("Bearer {}", access.expose()))
            .body(Body::from(body(method)))
            .unwrap()
    };
    let anonymous = Request::builder()
        .method("POST")
        .uri("/mcp")
        .body(Body::from(body("server/discover")))
        .unwrap();
    assert_eq!(
        router.clone().oneshot(anonymous).await.unwrap().status(),
        StatusCode::UNAUTHORIZED
    );
    let bad_origin = Request::builder()
        .method("POST")
        .uri("/mcp")
        .header(ORIGIN, "https://evil.example.invalid")
        .body(Body::from(body("server/discover")))
        .unwrap();
    assert_eq!(
        router.clone().oneshot(bad_origin).await.unwrap().status(),
        StatusCode::FORBIDDEN
    );
    let bad_header = Request::builder()
        .method("POST")
        .uri("/mcp")
        .header(CONTENT_TYPE, "application/json")
        .header(ACCEPT, "application/json, text/event-stream")
        .header("mcp-protocol-version", "2026-07-28")
        .header("mcp-method", "other")
        .header(AUTHORIZATION, format!("Bearer {}", access.expose()))
        .body(Body::from(body("server/discover")))
        .unwrap();
    let error = router.clone().oneshot(bad_header).await.unwrap();
    assert_eq!(error.status(), StatusCode::BAD_REQUEST);
    let json: serde_json::Value =
        serde_json::from_slice(&to_bytes(error.into_body(), 8192).await.unwrap()).unwrap();
    assert_eq!(json["id"], 7);
    assert_eq!(json["error"]["code"], -32020);
    let legacy_init = Request::builder().method("POST").uri("/mcp")
        .header(CONTENT_TYPE, "application/json").header(ACCEPT, "application/json, text/event-stream")
        .header(AUTHORIZATION, format!("Bearer {}", access.expose()))
        .body(Body::from(json!({"jsonrpc":"2.0","id":8,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"client","version":"1"}}}).to_string())).unwrap();
    let response = router.clone().oneshot(legacy_init).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let json: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 8192).await.unwrap()).unwrap();
    assert_eq!(json["result"]["protocolVersion"], "2025-11-25");
    let legacy_notification = Request::builder()
        .method("POST")
        .uri("/mcp")
        .header(CONTENT_TYPE, "application/json")
        .header(ACCEPT, "application/json, text/event-stream")
        .header("mcp-protocol-version", "2025-11-25")
        .header(AUTHORIZATION, format!("Bearer {}", access.expose()))
        .body(Body::from(
            json!({"jsonrpc":"2.0","method":"notifications/initialized"}).to_string(),
        ))
        .unwrap();
    let response = router.clone().oneshot(legacy_notification).await.unwrap();
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    assert!(
        to_bytes(response.into_body(), 8192)
            .await
            .unwrap()
            .is_empty()
    );
    let response = router
        .clone()
        .oneshot(request("server/discover"))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let response = router.clone().oneshot(request("tools/list")).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let json: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 8192).await.unwrap()).unwrap();
    assert_eq!(json["result"]["tools"][0]["name"], "fixture.read");
    let limited = router.oneshot(request("tools/list")).await.unwrap();
    assert_eq!(limited.status(), StatusCode::TOO_MANY_REQUESTS);
    assert!(limited.headers().contains_key(RETRY_AFTER));
    let audited:i64=sqlx::query_scalar("SELECT COUNT(*) FROM audit_events WHERE actor_id=? AND action IN ('mcp.discover','mcp.tools.list')").bind(user).fetch_one(&pool).await.unwrap();
    assert_eq!(audited, 3);
}
