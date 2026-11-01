use axum::{
    body::Body,
    http::{
        Request, StatusCode,
        header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE},
    },
};
use bastion_core::Database;
use bastion_mcp::{
    ActorQuota, AuthenticatedAccess, Credential, CredentialKind, ExchangePolicy, MysqlQuota,
    MysqlTokenExchange, ToolDescriptor, ToolQuota, ToolRegistry, ToolRisk,
};
use bastion_mcp_core::ReadToolError;
use bastion_mcp_http::{DraftMcpState, ReadToolExecutor, draft_mcp_router};
use serde_json::{Value, json};
use sqlx::{MySql, Transaction, mysql::MySqlPoolOptions};
use std::{collections::BTreeSet, future::Future, pin::Pin, sync::Arc, time::Duration};
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
struct Slow;
impl ReadToolExecutor for Slow {
    fn call<'a>(
        &'a self,
        _: &'a AuthenticatedAccess,
        _: &'a str,
        _: Value,
    ) -> Pin<Box<dyn Future<Output = Result<Value, ReadToolError>> + Send + 'a>> {
        Box::pin(async {
            tokio::time::sleep(Duration::from_secs(10)).await;
            Ok(json!({}))
        })
    }
}

#[tokio::test]
async fn mysql57_global_and_tool_timeouts_leave_terminal_audit() {
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
    let role = sqlx::query("INSERT INTO roles (name) VALUES ('MCP timeout role')")
        .execute(&pool)
        .await
        .unwrap()
        .last_insert_id();
    let person = sqlx::query("INSERT INTO persons (full_name) VALUES ('MCP timeout user')")
        .execute(&pool)
        .await
        .unwrap()
        .last_insert_id();
    let user=sqlx::query("INSERT INTO users (person_id,role_id,email,status) VALUES (?,?,'mcp-timeout@example.invalid','active')").bind(person).bind(role).execute(&pool).await.unwrap().last_insert_id();
    sqlx::query("INSERT INTO module_registry (module_id,display_name,included_in_release,environment_allowed,enabled) VALUES ('mcp_timeout_fixture','fixture',1,1,1)").execute(&pool).await.unwrap();
    let client=sqlx::query("INSERT INTO oauth_clients (client_id,client_id_hash,display_name,metadata_hash,approved_metadata_hash,enabled,token_auth_method,created_at,updated_at) VALUES ('mcp-timeout-client',UNHEX(SHA2('mcp-timeout-client',256)),'fixture',UNHEX(SHA2('metadata',256)),UNHEX(SHA2('metadata',256)),1,'none',UTC_TIMESTAMP(6),UTC_TIMESTAMP(6))").execute(&pool).await.unwrap().last_insert_id();
    sqlx::query("INSERT INTO oauth_client_scopes (client_id,scope_name) VALUES (?,'fixture:read')")
        .bind(client)
        .execute(&pool)
        .await
        .unwrap();
    let grant=sqlx::query("INSERT INTO oauth_grants (user_id,client_id,resource_uri,scopes_json,issued_at,expires_at) VALUES (?,?,'https://example.invalid/mcp',JSON_ARRAY('fixture:read'),UTC_TIMESTAMP(6)-INTERVAL 1 SECOND,UTC_TIMESTAMP(6)+INTERVAL 1 DAY)").bind(user).bind(client).execute(&pool).await.unwrap().last_insert_id();
    let access = Credential::generate().unwrap();
    sqlx::query("INSERT INTO oauth_access_tokens (grant_id,token_hash,scopes_json,issued_at,expires_at) VALUES (?,?,JSON_ARRAY('fixture:read'),UTC_TIMESTAMP(6)-INTERVAL 1 SECOND,UTC_TIMESTAMP(6)+INTERVAL 10 MINUTE)").bind(grant).bind(access.digest(CredentialKind::AccessToken).as_slice()).execute(&pool).await.unwrap();
    let create = |name: &str, tool_timeout: u64, server_timeout: Duration| {
        let descriptor = ToolDescriptor {
            name: name.into(),
            module_id: "mcp_timeout_fixture".into(),
            description: "Fixture".into(),
            required_scope: "fixture:read".into(),
            input_schema: json!({"type":"object"}),
            output_schema: None,
            risk: ToolRisk::ReadOnly,
            idempotent: true,
            max_response_bytes: 1024,
            max_duration_ms: tool_timeout,
        };
        let registry = Arc::new(
            ToolRegistry::new(vec![descriptor], &BTreeSet::from(["fixture:read".into()])).unwrap(),
        );
        let quota = MysqlQuota::new(
            pool.clone(),
            ActorQuota {
                window_seconds: 60,
                user_total: 20,
                client_total: 20,
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
                user_tool: 10,
                client_tool: 10,
            },
            4,
            server_timeout,
        )
        .unwrap()
        .with_read_executor(Arc::new(Slow));
        draft_mcp_router(state)
    };
    let call = |name: &str| {
        Request::builder().method("POST").uri("/mcp").header(CONTENT_TYPE,"application/json").header(ACCEPT,"application/json, text/event-stream").header(AUTHORIZATION,format!("Bearer {}",access.expose())).header("mcp-protocol-version","2026-07-28").header("mcp-method","tools/call").header("mcp-name",name).body(Body::from(json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":name,"arguments":{},"_meta":{"io.modelcontextprotocol/protocolVersion":"2026-07-28","io.modelcontextprotocol/clientCapabilities":{}}}}).to_string())).unwrap()
    };
    let global = create("fixture.global", 10000, Duration::from_secs(2))
        .oneshot(call("fixture.global"))
        .await
        .unwrap();
    assert_eq!(global.status(), StatusCode::SERVICE_UNAVAILABLE);
    let inner = create("fixture.inner", 50, Duration::from_secs(5))
        .oneshot(call("fixture.inner"))
        .await
        .unwrap();
    assert_eq!(inner.status(), StatusCode::BAD_REQUEST);
    for tool in ["fixture.global", "fixture.inner"] {
        let rows:Vec<String>=sqlx::query_scalar("SELECT result FROM audit_events WHERE actor_id=? AND action='mcp.tool.call' AND target_id=? ORDER BY id").bind(user).bind(tool).fetch_all(&pool).await.unwrap();
        assert_eq!(rows, vec!["started", "timeout"]);
    }
}
