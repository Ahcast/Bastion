use axum::{
    body::Body,
    http::{
        Request, StatusCode,
        header::{CACHE_CONTROL, LOCATION, ORIGIN},
    },
};
use bastion_core::Database;
use bastion_mcp::{ApprovedClientStore, ClientMetadata, validate_client_metadata};
use bastion_mcp_http::{
    ClientMetadataSource, DraftAuthorizationState, MetadataFetchError, draft_authorization_router,
};
use serde_json::json;
use sqlx::mysql::MySqlPoolOptions;
use std::{
    collections::BTreeSet,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};
use tower::ServiceExt;

#[derive(Clone)]
struct FakeFetcher {
    changed: Arc<AtomicBool>,
    hits: Arc<AtomicUsize>,
}
impl ClientMetadataSource for FakeFetcher {
    async fn fetch(&self, id: &str) -> Result<ClientMetadata, MetadataFetchError> {
        self.hits.fetch_add(1, Ordering::SeqCst);
        let mut body=json!({"client_id":id,"client_name":"fixture","redirect_uris":["https://auth-client.example.invalid/callback"],"token_endpoint_auth_method":"none"}).to_string();
        if self.changed.load(Ordering::SeqCst) {
            body.push(' ');
        }
        validate_client_metadata(id, id, 200, body.as_bytes())
            .map_err(|_| MetadataFetchError::InvalidResponse)
    }
}

#[tokio::test]
async fn mysql57_authorization_entry_refetches_approved_metadata_and_only_redirects_to_frontend() {
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
    let role: u64 =
        sqlx::query_scalar("SELECT id FROM roles WHERE system_key='system_administrator'")
            .fetch_one(&pool)
            .await
            .unwrap();
    let person = sqlx::query("INSERT INTO persons (full_name) VALUES ('OAuth entry admin')")
        .execute(&pool)
        .await
        .unwrap()
        .last_insert_id();
    let admin=sqlx::query("INSERT INTO users (person_id,role_id,email,status) VALUES (?,?,'oauth-entry-admin@example.invalid','active')").bind(person).bind(role).execute(&pool).await.unwrap().last_insert_id();
    let client_id = "https://auth-client.example.invalid/metadata.json";
    let callback = "https://auth-client.example.invalid/callback";
    let raw=json!({"client_id":client_id,"client_name":"fixture","redirect_uris":[callback],"token_endpoint_auth_method":"none"}).to_string();
    let metadata = validate_client_metadata(client_id, client_id, 200, raw.as_bytes()).unwrap();
    let scopes = BTreeSet::from(["cotizaciones:read".to_owned()]);
    ApprovedClientStore::new(pool.clone())
        .approve_new_client(admin, &metadata, &scopes, &scopes)
        .await
        .unwrap();
    let changed = Arc::new(AtomicBool::new(false));
    let hits = Arc::new(AtomicUsize::new(0));
    let state = DraftAuthorizationState::new(
        pool.clone(),
        "https://api.example.invalid/mcp".into(),
        "https://app.example.invalid".into(),
        BTreeSet::from(["https://client.example.invalid".into()]),
        FakeFetcher {
            changed: changed.clone(),
            hits: hits.clone(),
        },
        2,
    )
    .unwrap();
    let router = draft_authorization_router(state);
    let mut query = url::form_urlencoded::Serializer::new(String::new());
    query
        .append_pair("response_type", "code")
        .append_pair("client_id", client_id)
        .append_pair("redirect_uri", callback)
        .append_pair("resource", "https://api.example.invalid/mcp")
        .append_pair("scope", "cotizaciones:read")
        .append_pair("state", "opaque")
        .append_pair("code_challenge_method", "S256")
        .append_pair(
            "code_challenge",
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM",
        );
    let query = query.finish();
    let request = || {
        Request::builder()
            .uri(format!("/oauth/authorize?{query}"))
            .body(Body::empty())
            .unwrap()
    };
    let forbidden = Request::builder()
        .uri(format!("/oauth/authorize?{query}"))
        .header(ORIGIN, "https://evil.invalid")
        .body(Body::empty())
        .unwrap();
    assert_eq!(
        router.clone().oneshot(forbidden).await.unwrap().status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(hits.load(Ordering::SeqCst), 0);
    let result = router.clone().oneshot(request()).await.unwrap();
    assert_eq!(result.status(), StatusCode::SEE_OTHER);
    assert_eq!(result.headers()[CACHE_CONTROL], "no-store");
    let redirect = url::Url::parse(result.headers()[LOCATION].to_str().unwrap()).unwrap();
    assert_eq!(
        redirect.origin().ascii_serialization(),
        "https://app.example.invalid"
    );
    assert_eq!(redirect.path(), "/oauth/consent");
    assert_eq!(redirect.query(), None);
    assert_eq!(redirect.fragment(), Some(query.as_str()));
    let client_pk: u64 = sqlx::query_scalar("SELECT id FROM oauth_clients WHERE client_id=?")
        .bind(client_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    let old_grant=sqlx::query("INSERT INTO oauth_grants (user_id,client_id,resource_uri,scopes_json,issued_at,expires_at) VALUES (?,?,'https://api.example.invalid/mcp',JSON_ARRAY('cotizaciones:read'),UTC_TIMESTAMP(6),UTC_TIMESTAMP(6)+INTERVAL 1 DAY)").bind(admin).bind(client_pk).execute(&pool).await.unwrap().last_insert_id();
    changed.store(true, Ordering::SeqCst);
    assert_eq!(
        router.clone().oneshot(request()).await.unwrap().status(),
        StatusCode::BAD_REQUEST
    );
    let revoked: bool =
        sqlx::query_scalar("SELECT revoked_at IS NOT NULL FROM oauth_grants WHERE id=?")
            .bind(old_grant)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(revoked);
    let other = query.replace("cotizaciones%3Aread", "facturas%3Aread");
    let invalid = Request::builder()
        .uri(format!("/oauth/authorize?{other}"))
        .body(Body::empty())
        .unwrap();
    let before = hits.load(Ordering::SeqCst);
    assert_eq!(
        router.oneshot(invalid).await.unwrap().status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(hits.load(Ordering::SeqCst), before);
    let grants:i64=sqlx::query_scalar("SELECT COUNT(*) FROM oauth_grants WHERE client_id=(SELECT id FROM oauth_clients WHERE client_id=?)").bind(client_id).fetch_one(&pool).await.unwrap();
    assert_eq!(grants, 1);
}
