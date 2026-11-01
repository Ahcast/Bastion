use axum::{
    body::{Body, to_bytes},
    http::{
        Request, StatusCode,
        header::{CONTENT_TYPE, COOKIE, ORIGIN},
    },
};
use bastion_core::{CoreService, Database};
use bastion_mcp::{ClientMetadata, Credential, validate_client_metadata};
use bastion_mcp_http::{
    ClientMetadataSource, DraftClientRequestState, MetadataFetchError, draft_client_request_router,
};
use serde_json::json;
use sqlx::mysql::MySqlPoolOptions;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use tower::ServiceExt;

#[derive(Clone)]
struct FakeFetcher(Arc<AtomicUsize>);
impl ClientMetadataSource for FakeFetcher {
    async fn fetch(&self, id: &str) -> Result<ClientMetadata, MetadataFetchError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        if id != "https://fixture.example/metadata.json" {
            return Err(MetadataFetchError::InvalidClient);
        }
        let body=json!({"client_id":id,"client_name":"Fixture","redirect_uris":["https://fixture.example/cb"],"token_endpoint_auth_method":"none"}).to_string();
        validate_client_metadata(id, id, 200, body.as_bytes())
            .map_err(|_| MetadataFetchError::InvalidResponse)
    }
}

#[tokio::test]
async fn mysql57_client_request_http_checks_session_csrf_admin_and_scope() {
    let Ok(url) = std::env::var("BASTION_MCP_TEST_URL") else {
        return;
    };
    let pool = MySqlPoolOptions::new()
        .max_connections(6)
        .connect(&url)
        .await
        .unwrap();
    let db = Database::from_test_pool(pool.clone());
    db.migrate().await.unwrap();
    let admin_role: u64 =
        sqlx::query_scalar("SELECT id FROM roles WHERE system_key='system_administrator'")
            .fetch_one(&pool)
            .await
            .unwrap();
    let role = sqlx::query("INSERT INTO roles (name) VALUES ('MCP HTTP request role')")
        .execute(&pool)
        .await
        .unwrap()
        .last_insert_id();
    let person = sqlx::query("INSERT INTO persons (full_name) VALUES ('MCP HTTP request user')")
        .execute(&pool)
        .await
        .unwrap()
        .last_insert_id();
    let user=sqlx::query("INSERT INTO users (person_id,role_id,email,status) VALUES (?,?,'mcp-request-http-user@example.invalid','active')").bind(person).bind(role).execute(&pool).await.unwrap().last_insert_id();
    let person = sqlx::query("INSERT INTO persons (full_name) VALUES ('MCP HTTP request admin')")
        .execute(&pool)
        .await
        .unwrap()
        .last_insert_id();
    let admin=sqlx::query("INSERT INTO users (person_id,role_id,email,status) VALUES (?,?,'mcp-request-http-admin@example.invalid','active')").bind(person).bind(admin_role).execute(&pool).await.unwrap().last_insert_id();
    let user_session = Credential::generate().unwrap().expose();
    let admin_session = Credential::generate().unwrap().expose();
    let csrf = Credential::generate().unwrap().expose();
    for (actor, token) in [(user, &user_session), (admin, &admin_session)] {
        sqlx::query("INSERT INTO sessions (user_id,token_hash,csrf_hash,ip_address,started_at,last_activity_at,last_totp_at,expires_at) VALUES (?,UNHEX(SHA2(?,256)),UNHEX(SHA2(?,256)),'127.0.0.1',UTC_TIMESTAMP(6),UTC_TIMESTAMP(6),UTC_TIMESTAMP(6),UTC_TIMESTAMP(6)+INTERVAL 1 HOUR)").bind(actor).bind(token).bind(&csrf).execute(&pool).await.unwrap();
    }
    let core = CoreService::new(db, Credential::generate().unwrap().expose(), [29; 32]).unwrap();
    let hits = Arc::new(AtomicUsize::new(0));
    let router = draft_client_request_router(
        DraftClientRequestState::new(
            pool.clone(),
            core,
            "https://app.example.invalid".into(),
            FakeFetcher(hits.clone()),
            2,
        )
        .unwrap(),
    );
    let request = |path: &str, session: &str, origin: &str, body: serde_json::Value| {
        Request::builder()
            .method("POST")
            .uri(path)
            .header(CONTENT_TYPE, "application/json")
            .header(ORIGIN, origin)
            .header(
                COOKIE,
                format!("__Host-bastion_session={session}; __Host-bastion_csrf={csrf}"),
            )
            .header("x-csrf-token", &csrf)
            .body(Body::from(body.to_string()))
            .unwrap()
    };
    let input =
        json!({"client_id":"https://fixture.example/metadata.json","scopes":["cotizaciones:read"]});
    assert_eq!(
        router
            .clone()
            .oneshot(request(
                "/oauth/clients/request",
                &user_session,
                "https://evil.invalid",
                input.clone()
            ))
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(hits.load(Ordering::SeqCst), 0);
    assert_eq!(
        router
            .clone()
            .oneshot(request(
                "/oauth/clients/request",
                &user_session,
                "https://app.example.invalid",
                json!({"client_id":"https://fixture.example/metadata.json","scopes":["unknown"]})
            ))
            .await
            .unwrap()
            .status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(hits.load(Ordering::SeqCst), 0);
    let created = router
        .clone()
        .oneshot(request(
            "/oauth/clients/request",
            &user_session,
            "https://app.example.invalid",
            input,
        ))
        .await
        .unwrap();
    assert_eq!(created.status(), StatusCode::OK);
    let body: serde_json::Value =
        serde_json::from_slice(&to_bytes(created.into_body(), 8192).await.unwrap()).unwrap();
    let id = body["request_id"].as_u64().unwrap();
    assert_eq!(
        router
            .clone()
            .oneshot(request(
                "/oauth/clients/pending",
                &user_session,
                "https://app.example.invalid",
                json!({})
            ))
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
    let own = router
        .clone()
        .oneshot(request(
            "/oauth/clients/mine",
            &user_session,
            "https://app.example.invalid",
            json!({}),
        ))
        .await
        .unwrap();
    let own: serde_json::Value =
        serde_json::from_slice(&to_bytes(own.into_body(), 8192).await.unwrap()).unwrap();
    assert!(
        own["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["id"] == id)
    );
    let review = json!({"request_id":id,"decision":"approve","client_id":"https://fixture.example/metadata.json","scopes":["cotizaciones:read"]});
    assert_eq!(
        router
            .clone()
            .oneshot(request(
                "/oauth/clients/review",
                &user_session,
                "https://app.example.invalid",
                review.clone()
            ))
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(hits.load(Ordering::SeqCst), 1);
    assert_eq!(
        router
            .clone()
            .oneshot(request(
                "/oauth/clients/review",
                &admin_session,
                "https://app.example.invalid",
                review
            ))
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    assert_eq!(hits.load(Ordering::SeqCst), 2);
    let pending = router
        .oneshot(request(
            "/oauth/clients/pending",
            &admin_session,
            "https://app.example.invalid",
            json!({}),
        ))
        .await
        .unwrap();
    let pending: serde_json::Value =
        serde_json::from_slice(&to_bytes(pending.into_body(), 8192).await.unwrap()).unwrap();
    assert!(
        !pending["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["id"] == id)
    );
    let grants: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM oauth_grants WHERE user_id=?")
        .bind(user)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(grants, 0);
}
