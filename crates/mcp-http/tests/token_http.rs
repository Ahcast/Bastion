use std::collections::BTreeSet;

use axum::{
    body::{Body, to_bytes},
    http::{
        Request, StatusCode,
        header::{AUTHORIZATION, CACHE_CONTROL, CONTENT_TYPE, ORIGIN},
    },
};
use bastion_core::Database;
use bastion_mcp::{Credential, CredentialKind, ExchangePolicy, MysqlTokenExchange};
use bastion_mcp_http::{TokenEndpoint, draft_token_router};
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
async fn mysql57_token_http_requires_valid_pkce_and_never_caches_tokens() {
    let Ok(url) = std::env::var("BASTION_MCP_TEST_URL") else {
        return;
    };
    let pool = MySqlPoolOptions::new()
        .max_connections(4)
        .connect(&url)
        .await
        .unwrap();
    Database::from_test_pool(pool.clone())
        .migrate()
        .await
        .unwrap();
    let person = sqlx::query("INSERT INTO persons (full_name) VALUES ('OAuth HTTP test')")
        .execute(&pool)
        .await
        .unwrap()
        .last_insert_id();
    let role = sqlx::query("INSERT INTO roles (name) VALUES ('OAuth HTTP role')")
        .execute(&pool)
        .await
        .unwrap()
        .last_insert_id();
    let user=sqlx::query("INSERT INTO users (person_id,role_id,email,status) VALUES (?,?,'oauth-http@example.invalid','active')").bind(person).bind(role).execute(&pool).await.unwrap().last_insert_id();
    let client=sqlx::query("INSERT INTO oauth_clients (client_id,client_id_hash,display_name,metadata_hash,approved_metadata_hash,enabled,token_auth_method,created_at,updated_at) VALUES ('http-client',UNHEX(SHA2('http-client',256)),'HTTP fixture',UNHEX(SHA2('metadata',256)),UNHEX(SHA2('metadata',256)),1,'none',UTC_TIMESTAMP(6),UTC_TIMESTAMP(6))").execute(&pool).await.unwrap().last_insert_id();
    sqlx::query("INSERT INTO oauth_client_scopes (client_id,scope_name) VALUES (?,'read')")
        .bind(client)
        .execute(&pool)
        .await
        .unwrap();
    let grant=sqlx::query("INSERT INTO oauth_grants (user_id,client_id,resource_uri,scopes_json,issued_at,expires_at) VALUES (?,?,'https://example.invalid/mcp',JSON_ARRAY('read'),UTC_TIMESTAMP(6)-INTERVAL 1 SECOND,UTC_TIMESTAMP(6)+INTERVAL 1 DAY)").bind(user).bind(client).execute(&pool).await.unwrap().last_insert_id();
    let code = Credential::generate().unwrap();
    sqlx::query("INSERT INTO oauth_authorization_codes (grant_id,code_hash,redirect_uri,challenge,issued_at,expires_at) VALUES (?,?,'https://client.invalid/cb','E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM',UTC_TIMESTAMP(6)-INTERVAL 1 SECOND,UTC_TIMESTAMP(6)+INTERVAL 5 MINUTE)").bind(grant).bind(code.digest(CredentialKind::AuthorizationCode).as_slice()).execute(&pool).await.unwrap();
    let router = draft_token_router(TokenEndpoint::new(
        MysqlTokenExchange::new(pool.clone(), FixturePolicy),
        "https://example.invalid/mcp".into(),
        BTreeSet::new(),
    ));
    let body = format!(
        "grant_type=authorization_code&client_id=http-client&redirect_uri=https%3A%2F%2Fclient.invalid%2Fcb&resource=https%3A%2F%2Fexample.invalid%2Fmcp&code={}&code_verifier={}",
        code.expose(),
        "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"
    );
    let request = |content: &str| {
        Request::builder()
            .method("POST")
            .uri("/oauth/token")
            .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(Body::from(content.to_owned()))
            .unwrap()
    };
    let bad = body.replace(
        "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk",
        &"a".repeat(43),
    );
    assert_eq!(
        router
            .clone()
            .oneshot(request(&bad))
            .await
            .unwrap()
            .status(),
        StatusCode::BAD_REQUEST
    );
    let origin = Request::builder()
        .method("POST")
        .uri("/oauth/token")
        .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header(ORIGIN, "https://evil.example.invalid")
        .body(Body::from(body.clone()))
        .unwrap();
    assert_eq!(
        router.clone().oneshot(origin).await.unwrap().status(),
        StatusCode::FORBIDDEN
    );
    let authenticated = Request::builder()
        .method("POST")
        .uri("/oauth/token")
        .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header(AUTHORIZATION, "Basic ignored")
        .body(Body::from(body.clone()))
        .unwrap();
    assert_eq!(
        router
            .clone()
            .oneshot(authenticated)
            .await
            .unwrap()
            .status(),
        StatusCode::BAD_REQUEST
    );
    let response = router.clone().oneshot(request(&body)).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers().get(CACHE_CONTROL).unwrap(), "no-store");
    let bytes = to_bytes(response.into_body(), 8192).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json["token_type"], "Bearer");
    assert_eq!(json["scope"], "read");
    assert!(
        json["access_token"]
            .as_str()
            .is_some_and(|value| value.len() == 43)
    );
    let replay = router.oneshot(request(&body)).await.unwrap();
    assert_eq!(replay.status(), StatusCode::BAD_REQUEST);
    assert_eq!(replay.headers().get(CACHE_CONTROL).unwrap(), "no-store");
    let revoked: Option<chrono::NaiveDateTime> =
        sqlx::query_scalar("SELECT revoked_at FROM oauth_grants WHERE id=?")
            .bind(grant)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(revoked.is_some());
}
