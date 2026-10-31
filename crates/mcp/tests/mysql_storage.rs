use bastion_core::Database;
use bastion_mcp::{MysqlOauthStore, StoreError, revoke_user_grants_in_transaction};
use sqlx::mysql::MySqlPoolOptions;

// This URL must point at a dedicated, empty database. CI supplies one in its
// disposable MySQL 5.7 container; never use the operating Bluehost database.
#[tokio::test]
async fn mysql57_revoke_is_owned_audited_and_transactional() {
    let Ok(url) = std::env::var("BASTION_MCP_TEST_URL") else {
        eprintln!("BASTION_MCP_TEST_URL is unset; isolated MySQL integration test skipped");
        return;
    };
    let pool = MySqlPoolOptions::new()
        .max_connections(3)
        .connect(&url)
        .await
        .unwrap();
    Database::from_test_pool(pool.clone())
        .migrate()
        .await
        .unwrap();
    // Execute this staged schema explicitly: its own migrator is not yet
    // integrated with the Core migration history or the deployment command.
    sqlx::raw_sql(include_str!("../migrations/0001_oauth.sql"))
        .execute(&pool)
        .await
        .unwrap();

    let person = sqlx::query("INSERT INTO persons (full_name) VALUES ('OAuth test user')")
        .execute(&pool)
        .await
        .unwrap()
        .last_insert_id();
    let role = sqlx::query("INSERT INTO roles (name) VALUES ('OAuth test role')")
        .execute(&pool)
        .await
        .unwrap()
        .last_insert_id();
    let user = sqlx::query("INSERT INTO users (person_id,role_id,email,status) VALUES (?,?,'oauth-test@example.invalid','active')").bind(person).bind(role).execute(&pool).await.unwrap().last_insert_id();
    let other_person = sqlx::query("INSERT INTO persons (full_name) VALUES ('OAuth other user')")
        .execute(&pool)
        .await
        .unwrap()
        .last_insert_id();
    let other = sqlx::query("INSERT INTO users (person_id,role_id,email,status) VALUES (?,?,'oauth-other@example.invalid','active')").bind(other_person).bind(role).execute(&pool).await.unwrap().last_insert_id();
    let client = sqlx::query("INSERT INTO oauth_clients (client_id,client_id_hash,display_name,metadata_hash,enabled,created_at,updated_at) VALUES ('synthetic-client',UNHEX(SHA2('synthetic-client',256)),'Synthetic client',UNHEX(SHA2('metadata',256)),1,UTC_TIMESTAMP(6),UTC_TIMESTAMP(6))").execute(&pool).await.unwrap().last_insert_id();
    let grant = sqlx::query("INSERT INTO oauth_grants (user_id,client_id,resource_uri,scopes_json,issued_at,expires_at) VALUES (?,?,'https://example.invalid/mcp',JSON_ARRAY('read'),UTC_TIMESTAMP(6),UTC_TIMESTAMP(6)+INTERVAL 1 DAY)").bind(user).bind(client).execute(&pool).await.unwrap().last_insert_id();
    let store = MysqlOauthStore::new(pool.clone());

    assert!(matches!(
        store.revoke_own_grant(other, grant).await,
        Err(StoreError::NotFound)
    ));
    let mut tx = pool.begin().await.unwrap();
    assert_eq!(
        revoke_user_grants_in_transaction(&mut tx, user, "password_reset")
            .await
            .unwrap(),
        1
    );
    tx.rollback().await.unwrap();
    let revoked: Option<chrono::NaiveDateTime> =
        sqlx::query_scalar("SELECT revoked_at FROM oauth_grants WHERE id=?")
            .bind(grant)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(revoked.is_none());

    let (first, second) = tokio::join!(
        store.revoke_own_grant(user, grant),
        store.revoke_own_grant(user, grant)
    );
    first.unwrap();
    second.unwrap();
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM audit_events WHERE actor_id=? AND action='oauth.grant.revoke' AND target_id=?").bind(user).bind(grant.to_string()).fetch_one(&pool).await.unwrap();
    assert_eq!(count, 1);
    let revoked: Option<chrono::NaiveDateTime> =
        sqlx::query_scalar("SELECT revoked_at FROM oauth_grants WHERE id=?")
            .bind(grant)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(revoked.is_some());

    let admin_role: u64 =
        sqlx::query_scalar("SELECT id FROM roles WHERE system_key='system_administrator'")
            .fetch_one(&pool)
            .await
            .unwrap();
    let admin_person =
        sqlx::query("INSERT INTO persons (full_name) VALUES ('OAuth test administrator')")
            .execute(&pool)
            .await
            .unwrap()
            .last_insert_id();
    let admin = sqlx::query("INSERT INTO users (person_id,role_id,email,status) VALUES (?,?,'oauth-admin@example.invalid','active')").bind(admin_person).bind(admin_role).execute(&pool).await.unwrap().last_insert_id();
    for owner in [user, other] {
        sqlx::query("INSERT INTO oauth_grants (user_id,client_id,resource_uri,scopes_json,issued_at,expires_at) VALUES (?,?,'https://example.invalid/mcp',JSON_ARRAY('read'),UTC_TIMESTAMP(6),UTC_TIMESTAMP(6)+INTERVAL 1 DAY)").bind(owner).bind(client).execute(&pool).await.unwrap();
    }
    assert!(matches!(
        store.revoke_client(user, client).await,
        Err(StoreError::Forbidden)
    ));
    store.revoke_client(admin, client).await.unwrap();
    store.revoke_client(admin, client).await.unwrap();
    let active: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM oauth_grants WHERE client_id=? AND revoked_at IS NULL",
    )
    .bind(client)
    .fetch_one(&pool)
    .await
    .unwrap();
    let audit_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM audit_events WHERE actor_id=? AND action='oauth.client.revoke' AND target_id=?").bind(admin).bind(client.to_string()).fetch_one(&pool).await.unwrap();
    assert_eq!(active, 0);
    assert_eq!(audit_count, 1);
}
