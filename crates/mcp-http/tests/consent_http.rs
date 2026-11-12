use axum::{
    body::{Body, to_bytes},
    http::{
        Request, StatusCode,
        header::{CACHE_CONTROL, CONTENT_TYPE, COOKIE, ORIGIN},
    },
};
use bastion_core::{CoreService, Database};
use bastion_mcp::{ApprovedClientStore, Credential, validate_client_metadata};
use bastion_mcp_core::{CoreScopePolicy, PermissionRequirement};
use bastion_mcp_http::{DraftConsentState, DraftOauthMetadata, draft_consent_router};
use serde_json::json;
use sqlx::mysql::MySqlPoolOptions;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
use tower::ServiceExt;

struct Guard(Arc<AtomicBool>);
impl bastion_mcp_http::MetadataIntegrity for Guard {
    fn check_client_id<'a>(
        &'a self,
        _: &'a str,
    ) -> std::pin::Pin<
        Box<
            dyn std::future::Future<Output = Result<(), bastion_mcp_http::MetadataIntegrityError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async {
            if self.0.load(Ordering::SeqCst) {
                Err(bastion_mcp_http::MetadataIntegrityError::Invalid)
            } else {
                Ok(())
            }
        })
    }
    fn check_client_pk(
        &self,
        _: u64,
    ) -> std::pin::Pin<
        Box<
            dyn std::future::Future<Output = Result<(), bastion_mcp_http::MetadataIntegrityError>>
                + Send
                + '_,
        >,
    > {
        self.check_client_id("")
    }
}

#[tokio::test]
async fn mysql57_browser_consent_requires_csrf_session_and_explicit_decision() {
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
    let role = sqlx::query("INSERT INTO roles (name) VALUES ('OAuth HTTP consent role')")
        .execute(&pool)
        .await
        .unwrap()
        .last_insert_id();
    let admin_person =
        sqlx::query("INSERT INTO persons (full_name) VALUES ('OAuth HTTP consent admin')")
            .execute(&pool)
            .await
            .unwrap()
            .last_insert_id();
    let admin=sqlx::query("INSERT INTO users (person_id,role_id,email,status) VALUES (?,?,'oauth-http-admin@example.invalid','active')").bind(admin_person).bind(admin_role).execute(&pool).await.unwrap().last_insert_id();
    let person = sqlx::query("INSERT INTO persons (full_name) VALUES ('OAuth HTTP consent user')")
        .execute(&pool)
        .await
        .unwrap()
        .last_insert_id();
    let user=sqlx::query("INSERT INTO users (person_id,role_id,email,status) VALUES (?,?,'oauth-http-user@example.invalid','active')").bind(person).bind(role).execute(&pool).await.unwrap().last_insert_id();
    sqlx::query("INSERT INTO module_registry (module_id,display_name,included_in_release,environment_allowed,enabled) VALUES ('oauth_http_fixture','fixture',1,1,1)").execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO role_permissions (role_id,module_id,scope_key,scope_hash,capability) VALUES (?,'oauth_http_fixture','documentos',UNHEX(SHA2('documentos',256)),'ver')").bind(role).execute(&pool).await.unwrap();
    let session = Credential::generate().unwrap().expose();
    let other = Credential::generate().unwrap().expose();
    let csrf = Credential::generate().unwrap().expose();
    for token in [&session, &other] {
        sqlx::query("INSERT INTO sessions (user_id,token_hash,csrf_hash,ip_address,started_at,last_activity_at,last_totp_at,expires_at) VALUES (?,UNHEX(SHA2(?,256)),UNHEX(SHA2(?,256)),'127.0.0.1',UTC_TIMESTAMP(6),UTC_TIMESTAMP(6),UTC_TIMESTAMP(6),UTC_TIMESTAMP(6)+INTERVAL 1 HOUR)").bind(user).bind(token).bind(&csrf).execute(&pool).await.unwrap();
    }
    let client_id = "https://oauth-http-client.example.invalid/metadata.json";
    let redirect = "https://oauth-http-client.example.invalid/callback";
    let raw=json!({"client_id":client_id,"client_name":"Client fixture","redirect_uris":[redirect],"token_endpoint_auth_method":"none"}).to_string();
    let metadata = validate_client_metadata(client_id, client_id, 200, raw.as_bytes()).unwrap();
    let scopes = BTreeSet::from(["fixture:read".to_owned()]);
    ApprovedClientStore::new(pool.clone())
        .approve_new_client(admin, &metadata, &scopes, &scopes)
        .await
        .unwrap();
    let core = CoreService::new(db, Credential::generate().unwrap().expose(), [23; 32]).unwrap();
    let policy = CoreScopePolicy::new(
        core.admin(),
        BTreeMap::from([(
            "fixture:read".into(),
            vec![PermissionRequirement {
                module_id: "oauth_http_fixture".into(),
                scope_key: "documentos".into(),
                capability: "ver".into(),
            }],
        )]),
    )
    .unwrap();
    let metadata = Arc::new(
        DraftOauthMetadata::new(
            "https://example.invalid/mcp".into(),
            "https://example.invalid/oauth".into(),
            "https://example.invalid/oauth/authorize".into(),
            "https://example.invalid/oauth/token".into(),
        )
        .unwrap(),
    );
    let changed = Arc::new(AtomicBool::new(false));
    let router = draft_consent_router(
        DraftConsentState::new(
            pool.clone(),
            core,
            policy,
            metadata,
            "https://app.example.invalid".into(),
        )
        .unwrap()
        .with_integrity_guard(Arc::new(Guard(changed.clone()))),
    );
    let mut query = url::form_urlencoded::Serializer::new(String::new());
    query
        .append_pair("response_type", "code")
        .append_pair("client_id", client_id)
        .append_pair("redirect_uri", redirect)
        .append_pair("resource", "https://example.invalid/mcp")
        .append_pair("scope", "fixture:read")
        .append_pair("state", "opaque-state")
        .append_pair("code_challenge_method", "S256")
        .append_pair(
            "code_challenge",
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM",
        );
    let query = query.finish();
    let request =
        |path: &str, token: &str, origin: &str, csrf_header: &str, body: serde_json::Value| {
            Request::builder()
                .method("POST")
                .uri(path)
                .header(CONTENT_TYPE, "application/json")
                .header(ORIGIN, origin)
                .header(
                    COOKIE,
                    format!("__Host-bastion_session={token}; __Host-bastion_csrf={csrf}"),
                )
                .header("x-csrf-token", csrf_header)
                .body(Body::from(body.to_string()))
                .unwrap()
        };
    let body = json!({"authorization_query":query});
    assert_eq!(
        router
            .clone()
            .oneshot(request(
                "/oauth/consent/begin",
                &session,
                "https://evil.invalid",
                &csrf,
                body.clone()
            ))
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        router
            .clone()
            .oneshot(request(
                "/oauth/consent/begin",
                &session,
                "https://app.example.invalid",
                "bad",
                body.clone()
            ))
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
    let started = router
        .clone()
        .oneshot(request(
            "/oauth/consent/begin",
            &session,
            "https://app.example.invalid",
            &csrf,
            body.clone(),
        ))
        .await
        .unwrap();
    assert_eq!(started.status(), StatusCode::OK);
    assert_eq!(started.headers()[CACHE_CONTROL], "no-store");
    let started: serde_json::Value =
        serde_json::from_slice(&to_bytes(started.into_body(), 8192).await.unwrap()).unwrap();
    let handle = started["handle"].as_str().unwrap();
    assert_eq!(
        router
            .clone()
            .oneshot(request(
                "/oauth/consent/decision",
                &other,
                "https://app.example.invalid",
                &csrf,
                json!({"handle":handle,"decision":"approve"})
            ))
            .await
            .unwrap()
            .status(),
        StatusCode::BAD_REQUEST
    );
    let approved = router
        .clone()
        .oneshot(request(
            "/oauth/consent/decision",
            &session,
            "https://app.example.invalid",
            &csrf,
            json!({"handle":handle,"decision":"approve"}),
        ))
        .await
        .unwrap();
    assert_eq!(approved.status(), StatusCode::OK);
    let approved: serde_json::Value =
        serde_json::from_slice(&to_bytes(approved.into_body(), 8192).await.unwrap()).unwrap();
    let redirect_url = url::Url::parse(approved["redirect_to"].as_str().unwrap()).unwrap();
    let pairs: BTreeMap<_, _> = redirect_url.query_pairs().into_owned().collect();
    assert_eq!(pairs.get("iss").unwrap(), "https://example.invalid/oauth");
    assert_eq!(pairs.get("state").unwrap(), "opaque-state");
    assert!(pairs.contains_key("code"));
    assert_eq!(
        router
            .clone()
            .oneshot(request(
                "/oauth/consent/decision",
                &session,
                "https://app.example.invalid",
                &csrf,
                json!({"handle":handle,"decision":"approve"})
            ))
            .await
            .unwrap()
            .status(),
        StatusCode::BAD_REQUEST
    );
    let next = router
        .clone()
        .oneshot(request(
            "/oauth/consent/begin",
            &session,
            "https://app.example.invalid",
            &csrf,
            body.clone(),
        ))
        .await
        .unwrap();
    let next: serde_json::Value =
        serde_json::from_slice(&to_bytes(next.into_body(), 8192).await.unwrap()).unwrap();
    let denied = router
        .clone()
        .oneshot(request(
            "/oauth/consent/decision",
            &session,
            "https://app.example.invalid",
            &csrf,
            json!({"handle":next["handle"],"decision":"deny"}),
        ))
        .await
        .unwrap();
    assert_eq!(denied.status(), StatusCode::OK);
    let denied: serde_json::Value =
        serde_json::from_slice(&to_bytes(denied.into_body(), 8192).await.unwrap()).unwrap();
    let denied_url = url::Url::parse(denied["redirect_to"].as_str().unwrap()).unwrap();
    assert!(
        denied_url
            .query_pairs()
            .any(|(key, value)| key == "error" && value == "access_denied")
    );
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM oauth_grants WHERE user_id=?")
        .bind(user)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
    changed.store(true, Ordering::SeqCst);
    let blocked = router
        .oneshot(request(
            "/oauth/consent/begin",
            &session,
            "https://app.example.invalid",
            &csrf,
            body,
        ))
        .await
        .unwrap();
    assert_eq!(blocked.status(), StatusCode::BAD_REQUEST);
    let blocked: serde_json::Value =
        serde_json::from_slice(&to_bytes(blocked.into_body(), 8192).await.unwrap()).unwrap();
    assert_eq!(blocked["error"], "client_review_required");
}
