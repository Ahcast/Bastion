use std::collections::BTreeSet;

use bastion_core::Database;
use bastion_mcp::{
    Credential, CredentialKind, ExchangePolicy, MysqlTokenExchange, TokenExchangeError,
    TokenParameters,
};
use sqlx::{MySql, Transaction, mysql::MySqlPoolOptions};

struct FixturePolicy(bool);

impl ExchangePolicy for FixturePolicy {
    async fn authorize(
        &self,
        _: &mut Transaction<'_, MySql>,
        _: u64,
        _: u64,
        _: &BTreeSet<String>,
    ) -> Result<bool, sqlx::Error> {
        Ok(self.0)
    }
}

// Dedicated ephemeral CI database only; never use the operating database.
#[tokio::test]
async fn mysql57_code_exchange_is_atomic_and_replay_revokes_grant() {
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
    let person = sqlx::query("INSERT INTO persons (full_name) VALUES ('OAuth exchange test')")
        .execute(&pool)
        .await
        .unwrap()
        .last_insert_id();
    let role = sqlx::query("INSERT INTO roles (name) VALUES ('OAuth exchange role')")
        .execute(&pool)
        .await
        .unwrap()
        .last_insert_id();
    let user = sqlx::query("INSERT INTO users (person_id,role_id,email,status) VALUES (?,?,'oauth-exchange@example.invalid','active')").bind(person).bind(role).execute(&pool).await.unwrap().last_insert_id();
    let client = sqlx::query("INSERT INTO oauth_clients (client_id,client_id_hash,display_name,metadata_hash,approved_metadata_hash,enabled,token_auth_method,created_at,updated_at) VALUES ('exchange-client',UNHEX(SHA2('exchange-client',256)),'Exchange client',UNHEX(SHA2('metadata',256)),UNHEX(SHA2('metadata',256)),1,'none',UTC_TIMESTAMP(6),UTC_TIMESTAMP(6))").execute(&pool).await.unwrap().last_insert_id();
    sqlx::query("INSERT INTO oauth_client_scopes (client_id,scope_name) VALUES (?,'read')")
        .bind(client)
        .execute(&pool)
        .await
        .unwrap();
    let grant = sqlx::query("INSERT INTO oauth_grants (user_id,client_id,resource_uri,scopes_json,issued_at,expires_at) VALUES (?,?,'https://example.invalid/mcp',JSON_ARRAY('read'),UTC_TIMESTAMP(6)-INTERVAL 1 SECOND,UTC_TIMESTAMP(6)+INTERVAL 1 DAY)").bind(user).bind(client).execute(&pool).await.unwrap().last_insert_id();
    let code = Credential::generate().unwrap();
    sqlx::query("INSERT INTO oauth_authorization_codes (grant_id,code_hash,redirect_uri,challenge,issued_at,expires_at) VALUES (?,?,'https://client.invalid/cb','E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM',UTC_TIMESTAMP(6)-INTERVAL 1 SECOND,UTC_TIMESTAMP(6)+INTERVAL 5 MINUTE)").bind(grant).bind(code.digest(CredentialKind::AuthorizationCode).as_slice()).execute(&pool).await.unwrap();
    let request = TokenParameters::AuthorizationCode {
        client_id: "exchange-client".into(),
        redirect_uri: "https://client.invalid/cb".into(),
        resource: "https://example.invalid/mcp".into(),
        code,
        verifier: "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk".into(),
    };
    let denied = MysqlTokenExchange::new(pool.clone(), FixturePolicy(false));
    assert!(matches!(
        denied.redeem_code(&request).await,
        Err(TokenExchangeError::InvalidGrant)
    ));
    let unused: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM oauth_authorization_codes WHERE grant_id=? AND consumed_at IS NULL",
    )
    .bind(grant)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(unused, 1);
    let exchange = MysqlTokenExchange::new(pool.clone(), FixturePolicy(true));
    let tokens = exchange.redeem_code(&request).await.unwrap();
    assert!((1..=900).contains(&tokens.expires_in));
    let stored: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM oauth_access_tokens WHERE grant_id=? AND token_hash=?",
    )
    .bind(grant)
    .bind(
        tokens
            .access_token
            .digest(CredentialKind::AccessToken)
            .as_slice(),
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(stored, 1);
    assert!(matches!(
        exchange.redeem_code(&request).await,
        Err(TokenExchangeError::InvalidGrant)
    ));
    let revoked: Option<chrono::NaiveDateTime> =
        sqlx::query_scalar("SELECT revoked_at FROM oauth_grants WHERE id=?")
            .bind(grant)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(revoked.is_some());
}
