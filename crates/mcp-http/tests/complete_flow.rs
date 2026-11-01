use axum::{
    body::{Body, to_bytes},
    http::{
        Request, StatusCode,
        header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE, COOKIE, LOCATION, ORIGIN},
    },
};
use bastion_core::{CoreService, Database};
use bastion_cotizaciones::QuoteService;
use bastion_mcp::{
    ActorQuota, ApprovedClientStore, ClientMetadata, Credential, ToolQuota,
    validate_client_metadata,
};
use bastion_mcp_http::{
    ClientMetadataSource, DraftCompositionConfig, MetadataFetchError, compose_draft_router,
};
use serde_json::json;
use sqlx::mysql::MySqlPoolOptions;
use std::{collections::BTreeSet, time::Duration};
use tower::ServiceExt;

#[derive(Clone)]
struct FakeFetcher;
impl ClientMetadataSource for FakeFetcher {
    async fn fetch(&self, id: &str) -> Result<ClientMetadata, MetadataFetchError> {
        let raw=json!({"client_id":id,"client_name":"End-to-end fixture","redirect_uris":["https://complete-client.example.invalid/cb"],"token_endpoint_auth_method":"none"}).to_string();
        validate_client_metadata(id, id, 200, raw.as_bytes())
            .map_err(|_| MetadataFetchError::InvalidResponse)
    }
}

#[tokio::test]
async fn mysql57_full_draft_oauth_to_tool_rechecks_permissions_and_replay() {
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
    let core = CoreService::new(
        db.clone(),
        Credential::generate().unwrap().expose(),
        [37; 32],
    )
    .unwrap();
    QuoteService::new(db, core.admin())
        .synchronize_release(true)
        .await
        .unwrap();
    sqlx::query("UPDATE module_registry SET enabled=1 WHERE module_id='cotizaciones'")
        .execute(&pool)
        .await
        .unwrap();
    let admin_role: u64 =
        sqlx::query_scalar("SELECT id FROM roles WHERE system_key='system_administrator'")
            .fetch_one(&pool)
            .await
            .unwrap();
    let role = sqlx::query("INSERT INTO roles (name) VALUES ('MCP full flow role')")
        .execute(&pool)
        .await
        .unwrap()
        .last_insert_id();
    let admin_person =
        sqlx::query("INSERT INTO persons (full_name) VALUES ('MCP full flow admin')")
            .execute(&pool)
            .await
            .unwrap()
            .last_insert_id();
    let admin=sqlx::query("INSERT INTO users (person_id,role_id,email,status) VALUES (?,?,'mcp-full-admin@example.invalid','active')").bind(admin_person).bind(admin_role).execute(&pool).await.unwrap().last_insert_id();
    let person = sqlx::query("INSERT INTO persons (full_name) VALUES ('MCP full flow user')")
        .execute(&pool)
        .await
        .unwrap()
        .last_insert_id();
    let user=sqlx::query("INSERT INTO users (person_id,role_id,email,status) VALUES (?,?,'mcp-full-user@example.invalid','active')").bind(person).bind(role).execute(&pool).await.unwrap().last_insert_id();
    sqlx::query("INSERT INTO role_permissions (role_id,module_id,scope_key,scope_hash,capability) VALUES (?,'cotizaciones','documentos',UNHEX(SHA2('documentos',256)),'ver')").bind(role).execute(&pool).await.unwrap();
    let session = Credential::generate().unwrap().expose();
    let csrf = Credential::generate().unwrap().expose();
    sqlx::query("INSERT INTO sessions (user_id,token_hash,csrf_hash,ip_address,started_at,last_activity_at,last_totp_at,expires_at) VALUES (?,UNHEX(SHA2(?,256)),UNHEX(SHA2(?,256)),'127.0.0.1',UTC_TIMESTAMP(6),UTC_TIMESTAMP(6),UTC_TIMESTAMP(6),UTC_TIMESTAMP(6)+INTERVAL 1 HOUR)").bind(user).bind(&session).bind(&csrf).execute(&pool).await.unwrap();
    let client = "https://complete-client.example.invalid/metadata.json";
    let redirect = "https://complete-client.example.invalid/cb";
    let resource = "https://api.example.invalid/mcp";
    let metadata = FakeFetcher.fetch(client).await.unwrap();
    let scopes = BTreeSet::from(["cotizaciones:read".to_owned()]);
    ApprovedClientStore::new(pool.clone())
        .approve_new_client(admin, &metadata, &scopes, &scopes)
        .await
        .unwrap();
    let config = DraftCompositionConfig {
        resource: resource.into(),
        issuer: "https://api.example.invalid/oauth".into(),
        authorization_endpoint: "https://api.example.invalid/oauth/authorize".into(),
        token_endpoint: "https://api.example.invalid/oauth/token".into(),
        frontend_origin: "https://app.example.invalid".into(),
        client_origins: BTreeSet::new(),
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
    };
    let router = compose_draft_router(core, config, FakeFetcher).unwrap();
    let mut form = url::form_urlencoded::Serializer::new(String::new());
    form.append_pair("response_type", "code")
        .append_pair("client_id", client)
        .append_pair("redirect_uri", redirect)
        .append_pair("resource", resource)
        .append_pair("scope", "cotizaciones:read")
        .append_pair("state", "state-fixture")
        .append_pair("code_challenge_method", "S256")
        .append_pair(
            "code_challenge",
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM",
        );
    let query = form.finish();
    let start = router
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/oauth/authorize?{query}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(start.status(), StatusCode::SEE_OTHER);
    assert!(
        start.headers()[LOCATION]
            .to_str()
            .unwrap()
            .starts_with("https://app.example.invalid/oauth/consent?")
    );
    let browser = |path: &str, body: serde_json::Value| {
        Request::builder()
            .method("POST")
            .uri(path)
            .header(CONTENT_TYPE, "application/json")
            .header(ORIGIN, "https://app.example.invalid")
            .header(
                COOKIE,
                format!("__Host-bastion_session={session}; __Host-bastion_csrf={csrf}"),
            )
            .header("x-csrf-token", &csrf)
            .body(Body::from(body.to_string()))
            .unwrap()
    };
    let pending = router
        .clone()
        .oneshot(browser(
            "/oauth/consent/begin",
            json!({"authorization_query":query}),
        ))
        .await
        .unwrap();
    assert_eq!(pending.status(), StatusCode::OK);
    let pending: serde_json::Value =
        serde_json::from_slice(&to_bytes(pending.into_body(), 8192).await.unwrap()).unwrap();
    let consent = router
        .clone()
        .oneshot(browser(
            "/oauth/consent/decision",
            json!({"handle":pending["handle"],"decision":"approve"}),
        ))
        .await
        .unwrap();
    assert_eq!(consent.status(), StatusCode::OK);
    let consent: serde_json::Value =
        serde_json::from_slice(&to_bytes(consent.into_body(), 8192).await.unwrap()).unwrap();
    let callback = url::Url::parse(consent["redirect_to"].as_str().unwrap()).unwrap();
    let pairs: std::collections::BTreeMap<_, _> = callback.query_pairs().into_owned().collect();
    assert_eq!(pairs.get("state").unwrap(), "state-fixture");
    assert_eq!(
        pairs.get("iss").unwrap(),
        "https://api.example.invalid/oauth"
    );
    let code = pairs.get("code").unwrap();
    let mut form = url::form_urlencoded::Serializer::new(String::new());
    form.append_pair("grant_type", "authorization_code")
        .append_pair("client_id", client)
        .append_pair("redirect_uri", redirect)
        .append_pair("resource", resource)
        .append_pair("code", code)
        .append_pair(
            "code_verifier",
            "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk",
        );
    let token_body = form.finish();
    let token_request = || {
        Request::builder()
            .method("POST")
            .uri("/oauth/token")
            .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(Body::from(token_body.clone()))
            .unwrap()
    };
    let token = router.clone().oneshot(token_request()).await.unwrap();
    assert_eq!(token.status(), StatusCode::OK);
    let token: serde_json::Value =
        serde_json::from_slice(&to_bytes(token.into_body(), 8192).await.unwrap()).unwrap();
    let access = token["access_token"].as_str().unwrap();
    let call = || {
        Request::builder().method("POST").uri("/mcp").header(CONTENT_TYPE,"application/json").header(ACCEPT,"application/json, text/event-stream").header(AUTHORIZATION,format!("Bearer {access}")).header("mcp-protocol-version","2026-07-28").header("mcp-method","tools/call").header("mcp-name","cotizaciones.listar").body(Body::from(json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"cotizaciones.listar","arguments":{"page_size":5},"_meta":{"io.modelcontextprotocol/protocolVersion":"2026-07-28","io.modelcontextprotocol/clientCapabilities":{}}}}).to_string())).unwrap()
    };
    let result = router.clone().oneshot(call()).await.unwrap();
    assert_eq!(result.status(), StatusCode::OK);
    let result: serde_json::Value =
        serde_json::from_slice(&to_bytes(result.into_body(), 262144).await.unwrap()).unwrap();
    assert_eq!(result["result"]["structuredContent"]["page_size"], 5);
    let audited:i64=sqlx::query_scalar("SELECT COUNT(*) FROM audit_events WHERE actor_id=? AND action='mcp.tool.call' AND target_id='cotizaciones.listar' AND result='success'").bind(user).fetch_one(&pool).await.unwrap();
    assert_eq!(audited, 1);
    sqlx::query("DELETE FROM role_permissions WHERE role_id=? AND module_id='cotizaciones'")
        .bind(role)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        router.clone().oneshot(call()).await.unwrap().status(),
        StatusCode::UNAUTHORIZED
    );
    sqlx::query("INSERT INTO role_permissions (role_id,module_id,scope_key,scope_hash,capability) VALUES (?,'cotizaciones','documentos',UNHEX(SHA2('documentos',256)),'ver')").bind(role).execute(&pool).await.unwrap();
    assert_eq!(
        router
            .clone()
            .oneshot(token_request())
            .await
            .unwrap()
            .status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        router.oneshot(call()).await.unwrap().status(),
        StatusCode::UNAUTHORIZED
    );
}
