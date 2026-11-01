use bastion_core::{CoreService, Database};
use bastion_cotizaciones::QuoteService;
use bastion_facturas::InvoiceService;
use bastion_mcp::{
    ActorQuota, Credential, CredentialKind, ExchangePolicy, MysqlQuota, MysqlTokenExchange,
    ToolQuota,
};
use bastion_mcp_core::{DraftReadExecutor, draft_read_tool_registry};
use bastion_mcp_http::{DraftMcpState, draft_mcp_router};
use bastion_ordenes_compra::OrderService;
use sqlx::{MySql, Transaction, mysql::MySqlPoolOptions};
use std::{collections::BTreeSet, sync::Arc, time::Duration};

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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mysql57_real_sdk_initializes_and_reads_quotes() {
    if std::env::var("BASTION_MCP_SDK_TEST").as_deref() != Ok("1") {
        return;
    }
    let Ok(url) = std::env::var("BASTION_MCP_TEST_URL") else {
        return;
    };
    let pool = MySqlPoolOptions::new()
        .max_connections(8)
        .connect(&url)
        .await
        .unwrap();
    let db = Database::from_test_pool(pool.clone());
    db.migrate().await.unwrap();
    QuoteService::migrate(&db).await.unwrap();
    InvoiceService::migrate(&db).await.unwrap();
    OrderService::migrate(&db).await.unwrap();
    let core = CoreService::new(
        db.clone(),
        Credential::generate().unwrap().expose(),
        [19; 32],
    )
    .unwrap();
    let quotes = QuoteService::new(db.clone(), core.admin());
    let invoices = InvoiceService::new(db.clone(), core.admin());
    let orders = OrderService::new(db, core.admin());
    quotes.synchronize_release(true).await.unwrap();
    invoices.synchronize_release(true).await.unwrap();
    orders.synchronize_release(true).await.unwrap();
    sqlx::query("UPDATE module_registry SET enabled=1 WHERE module_id IN ('cotizaciones','facturas','ordenes_compra')").execute(&pool).await.unwrap();
    let role = sqlx::query("INSERT INTO roles (name) VALUES ('MCP SDK read role')")
        .execute(&pool)
        .await
        .unwrap()
        .last_insert_id();
    let person = sqlx::query("INSERT INTO persons (full_name) VALUES ('MCP SDK read user')")
        .execute(&pool)
        .await
        .unwrap()
        .last_insert_id();
    let user=sqlx::query("INSERT INTO users (person_id,role_id,email,status) VALUES (?,?,'mcp-sdk-read@example.invalid','active')").bind(person).bind(role).execute(&pool).await.unwrap().last_insert_id();
    sqlx::query("INSERT INTO role_permissions (role_id,module_id,scope_key,scope_hash,capability) VALUES (?,'cotizaciones','documentos',UNHEX(SHA2('documentos',256)),'ver')").bind(role).execute(&pool).await.unwrap();
    let client=sqlx::query("INSERT INTO oauth_clients (client_id,client_id_hash,display_name,metadata_hash,approved_metadata_hash,enabled,token_auth_method,created_at,updated_at) VALUES ('mcp-sdk-read-client',UNHEX(SHA2('mcp-sdk-read-client',256)),'fixture',UNHEX(SHA2('metadata',256)),UNHEX(SHA2('metadata',256)),1,'none',UTC_TIMESTAMP(6),UTC_TIMESTAMP(6))").execute(&pool).await.unwrap().last_insert_id();
    sqlx::query(
        "INSERT INTO oauth_client_scopes (client_id,scope_name) VALUES (?,'cotizaciones:read')",
    )
    .bind(client)
    .execute(&pool)
    .await
    .unwrap();
    let grant=sqlx::query("INSERT INTO oauth_grants (user_id,client_id,resource_uri,scopes_json,issued_at,expires_at) VALUES (?,?,'https://example.invalid/mcp',JSON_ARRAY('cotizaciones:read'),UTC_TIMESTAMP(6)-INTERVAL 1 SECOND,UTC_TIMESTAMP(6)+INTERVAL 1 DAY)").bind(user).bind(client).execute(&pool).await.unwrap().last_insert_id();
    let access = Credential::generate().unwrap();
    sqlx::query("INSERT INTO oauth_access_tokens (grant_id,token_hash,scopes_json,issued_at,expires_at) VALUES (?,?,JSON_ARRAY('cotizaciones:read'),UTC_TIMESTAMP(6)-INTERVAL 1 SECOND,UTC_TIMESTAMP(6)+INTERVAL 10 MINUTE)").bind(grant).bind(access.digest(CredentialKind::AccessToken).as_slice()).execute(&pool).await.unwrap();
    let quota = MysqlQuota::new(
        pool.clone(),
        ActorQuota {
            window_seconds: 60,
            user_total: 10,
            client_total: 10,
        },
    )
    .unwrap();
    let state = DraftMcpState::new(
        pool.clone(),
        MysqlTokenExchange::new(pool.clone(), FixturePolicy),
        "https://example.invalid/mcp".into(),
        "https://example.invalid/.well-known/oauth-protected-resource/mcp".into(),
        BTreeSet::new(),
        Arc::new(draft_read_tool_registry().unwrap()),
        quota,
        ToolQuota {
            user_tool: 10,
            client_tool: 10,
        },
        4,
        Duration::from_secs(10),
    )
    .unwrap()
    .with_read_executor(Arc::new(DraftReadExecutor::new(quotes, invoices, orders)));
    let router = draft_mcp_router(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    let access_value = access.expose();
    let script =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/mcp-client/client.mjs");
    let child = tokio::task::spawn_blocking(move || {
        std::process::Command::new("node")
            .arg(script)
            .env("MCP_FIXTURE_ENDPOINT", format!("http://{address}/mcp"))
            .env("MCP_FIXTURE_ACCESS", access_value)
            .output()
    })
    .await
    .unwrap()
    .unwrap();
    server.abort();
    assert!(
        child.status.success(),
        "SDK failed: {}",
        String::from_utf8_lossy(&child.stderr)
    );
    let rows: Vec<String> = sqlx::query_scalar(
        "SELECT result FROM audit_events WHERE actor_id=? AND action='mcp.tool.call' ORDER BY id",
    )
    .bind(user)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(rows, vec!["started", "success"]);
}
