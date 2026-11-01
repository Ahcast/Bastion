use std::collections::{BTreeMap, BTreeSet};

use bastion_core::{CoreService, Database};
use bastion_mcp::{
    ApprovedClientStore, AuthorizationParameters, ConsentError, Credential, CredentialKind,
    MysqlConsent, MysqlTokenExchange, S256Challenge, TokenExchangeError, TokenParameters,
    validate_client_metadata,
};
use bastion_mcp_core::{CoreConsentPolicy, CoreScopePolicy, PermissionRequirement};
use sqlx::mysql::MySqlPoolOptions;

#[tokio::test]
async fn mysql57_consent_is_session_bound_rechecks_metadata_and_requires_permissions() {
    let Ok(url) = std::env::var("BASTION_MCP_TEST_URL") else {
        return;
    };
    let pool = MySqlPoolOptions::new()
        .max_connections(5)
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
    let role = sqlx::query("INSERT INTO roles (name) VALUES ('OAuth consent role')")
        .execute(&pool)
        .await
        .unwrap()
        .last_insert_id();
    let admin_person =
        sqlx::query("INSERT INTO persons (full_name) VALUES ('OAuth consent administrator')")
            .execute(&pool)
            .await
            .unwrap()
            .last_insert_id();
    let admin=sqlx::query("INSERT INTO users (person_id,role_id,email,status) VALUES (?,?,'oauth-consent-admin@example.invalid','active')").bind(admin_person).bind(admin_role).execute(&pool).await.unwrap().last_insert_id();
    let person = sqlx::query("INSERT INTO persons (full_name) VALUES ('OAuth consent user')")
        .execute(&pool)
        .await
        .unwrap()
        .last_insert_id();
    let user=sqlx::query("INSERT INTO users (person_id,role_id,email,status) VALUES (?,?,'oauth-consent-user@example.invalid','active')").bind(person).bind(role).execute(&pool).await.unwrap().last_insert_id();
    sqlx::query("INSERT INTO module_registry (module_id,display_name,included_in_release,environment_allowed,enabled) VALUES ('mcp_consent_fixture','Consent fixture',1,1,1)").execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO role_permissions (role_id,module_id,scope_key,scope_hash,capability) VALUES (?,'mcp_consent_fixture','documentos',UNHEX(SHA2('documentos',256)),'ver')").bind(role).execute(&pool).await.unwrap();
    let session = Credential::generate().unwrap().expose();
    let csrf = Credential::generate().unwrap().expose();
    sqlx::query("INSERT INTO sessions (user_id,token_hash,csrf_hash,ip_address,started_at,last_activity_at,last_totp_at,expires_at) VALUES (?,UNHEX(SHA2(?,256)),UNHEX(SHA2(?,256)),'127.0.0.1',UTC_TIMESTAMP(6),UTC_TIMESTAMP(6),UTC_TIMESTAMP(6),UTC_TIMESTAMP(6)+INTERVAL 1 HOUR)").bind(user).bind(&session).bind(&csrf).execute(&pool).await.unwrap();
    let other_session = Credential::generate().unwrap().expose();
    sqlx::query("INSERT INTO sessions (user_id,token_hash,csrf_hash,ip_address,started_at,last_activity_at,last_totp_at,expires_at) VALUES (?,UNHEX(SHA2(?,256)),UNHEX(SHA2(?,256)),'127.0.0.1',UTC_TIMESTAMP(6),UTC_TIMESTAMP(6),UTC_TIMESTAMP(6),UTC_TIMESTAMP(6)+INTERVAL 1 HOUR)").bind(user).bind(&other_session).bind(&csrf).execute(&pool).await.unwrap();
    let client_uri = "https://consent-client.example.invalid/metadata.json";
    let redirect = "https://consent-client.example.invalid/callback";
    let metadata_json=serde_json::json!({"client_id":client_uri,"client_name":"Consent fixture","redirect_uris":[redirect],"token_endpoint_auth_method":"none"}).to_string();
    let metadata =
        validate_client_metadata(client_uri, client_uri, 200, metadata_json.as_bytes()).unwrap();
    let scopes = BTreeSet::from(["fixture:read".to_owned()]);
    let client = ApprovedClientStore::new(pool.clone())
        .approve_new_client(admin, &metadata, &scopes, &scopes)
        .await
        .unwrap();
    let core = CoreService::new(db, "test-bootstrap-secret".repeat(2), [7; 32]).unwrap();
    let rule = PermissionRequirement {
        module_id: "mcp_consent_fixture".into(),
        scope_key: "documentos".into(),
        capability: "ver".into(),
    };
    let registry = BTreeMap::from([("fixture:read".to_owned(), vec![rule])]);
    let consent = MysqlConsent::new(
        pool.clone(),
        CoreConsentPolicy,
        CoreScopePolicy::new(core.admin(), registry.clone()).unwrap(),
    );
    let exchange = MysqlTokenExchange::new(
        pool.clone(),
        CoreScopePolicy::new(core.admin(), registry).unwrap(),
    );
    let request = AuthorizationParameters {
        client_id: client_uri.into(),
        redirect_uri: redirect.into(),
        resource: "https://example.invalid/mcp".into(),
        scopes: scopes.clone(),
        state: Some("opaque-state".into()),
        challenge: S256Challenge::parse("E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM").unwrap(),
    };
    assert!(matches!(
        consent
            .begin("invalid-session", &request, &request.resource)
            .await,
        Err(ConsentError::Unauthorized)
    ));
    let rejected = consent
        .begin(&session, &request, &request.resource)
        .await
        .unwrap();
    consent.deny(&session, &rejected.handle).await.unwrap();
    assert!(matches!(
        consent
            .approve(&session, &rejected.handle, &request.resource)
            .await,
        Err(ConsentError::InvalidRequest)
    ));
    let pending = consent
        .begin(&session, &request, &request.resource)
        .await
        .unwrap();
    assert!(matches!(
        consent
            .approve(&other_session, &pending.handle, &request.resource)
            .await,
        Err(ConsentError::InvalidRequest)
    ));
    let grants: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM oauth_grants WHERE user_id=?")
        .bind(user)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(grants, 0);
    sqlx::query("UPDATE oauth_clients SET metadata_hash=UNHEX(SHA2('changed',256)),approved_metadata_hash=UNHEX(SHA2('changed',256)) WHERE id=?").bind(client).execute(&pool).await.unwrap();
    assert!(matches!(
        consent
            .approve(&session, &pending.handle, &request.resource)
            .await,
        Err(ConsentError::InvalidRequest)
    ));
    sqlx::query("UPDATE oauth_clients SET metadata_hash=?,approved_metadata_hash=? WHERE id=?")
        .bind(metadata.document_hash().as_slice())
        .bind(metadata.document_hash().as_slice())
        .bind(client)
        .execute(&pool)
        .await
        .unwrap();
    let result = consent
        .approve(&session, &pending.handle, &request.resource)
        .await
        .unwrap();
    assert_eq!(result.redirect_uri, redirect);
    assert_eq!(result.state.as_deref(), Some("opaque-state"));
    assert!(matches!(
        consent
            .approve(&session, &pending.handle, &request.resource)
            .await,
        Err(ConsentError::InvalidRequest)
    ));
    let token_request = TokenParameters::AuthorizationCode {
        client_id: client_uri.into(),
        redirect_uri: redirect.into(),
        resource: request.resource.clone(),
        code: result.code,
        verifier: "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk".into(),
    };
    let tokens = exchange.redeem_code(&token_request).await.unwrap();
    let identity = exchange
        .authenticate_access(&tokens.access_token, &request.resource)
        .await
        .unwrap();
    assert_eq!(identity.user_id, user);
    assert_eq!(identity.client_id, client);
    assert_eq!(identity.scopes, scopes);
    assert!(matches!(
        exchange
            .authenticate_access(&tokens.access_token, "https://example.invalid/other")
            .await,
        Err(TokenExchangeError::InvalidGrant)
    ));
    let refresh = TokenParameters::RefreshToken {
        client_id: client_uri.into(),
        resource: request.resource.clone(),
        refresh_token: Credential::parse(&tokens.refresh_token.expose()).unwrap(),
        scopes: None,
    };
    sqlx::query("DELETE FROM role_permissions WHERE role_id=?")
        .bind(role)
        .execute(&pool)
        .await
        .unwrap();
    assert!(matches!(
        exchange
            .authenticate_access(&tokens.access_token, &request.resource)
            .await,
        Err(TokenExchangeError::InvalidGrant)
    ));
    assert!(matches!(
        exchange.rotate_refresh(&refresh).await,
        Err(TokenExchangeError::InvalidGrant)
    ));
    let unused: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM oauth_refresh_tokens WHERE token_hash=? AND consumed_at IS NULL",
    )
    .bind(
        tokens
            .refresh_token
            .digest(CredentialKind::RefreshToken)
            .as_slice(),
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(unused, 1);
}
