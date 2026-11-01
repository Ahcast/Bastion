use std::collections::BTreeSet;

use bastion_core::Database;
use bastion_mcp::{ApprovedClientStore, ClientApprovalError, validate_client_metadata};
use sqlx::mysql::MySqlPoolOptions;

#[tokio::test]
async fn mysql57_only_administrator_can_approve_registered_scopes_without_grants() {
    let Ok(url) = std::env::var("BASTION_MCP_TEST_URL") else {
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
    let admin_role: u64 =
        sqlx::query_scalar("SELECT id FROM roles WHERE system_key='system_administrator'")
            .fetch_one(&pool)
            .await
            .unwrap();
    let role = sqlx::query("INSERT INTO roles (name) VALUES ('OAuth approval role')")
        .execute(&pool)
        .await
        .unwrap()
        .last_insert_id();
    let admin_person =
        sqlx::query("INSERT INTO persons (full_name) VALUES ('OAuth approval administrator')")
            .execute(&pool)
            .await
            .unwrap()
            .last_insert_id();
    let admin = sqlx::query("INSERT INTO users (person_id,role_id,email,status) VALUES (?,?,'oauth-approval-admin@example.invalid','active')").bind(admin_person).bind(admin_role).execute(&pool).await.unwrap().last_insert_id();
    let person = sqlx::query("INSERT INTO persons (full_name) VALUES ('OAuth approval user')")
        .execute(&pool)
        .await
        .unwrap()
        .last_insert_id();
    let user = sqlx::query("INSERT INTO users (person_id,role_id,email,status) VALUES (?,?,'oauth-approval-user@example.invalid','active')").bind(person).bind(role).execute(&pool).await.unwrap().last_insert_id();
    let id = "https://client.example.invalid/metadata.json";
    let doc = serde_json::json!({"client_id":id,"client_name":"Example","redirect_uris":["https://client.example.invalid/cb"],"token_endpoint_auth_method":"none"}).to_string();
    let metadata = validate_client_metadata(id, id, 200, doc.as_bytes()).unwrap();
    let scopes = BTreeSet::from(["fixture:read".to_owned()]);
    let empty = BTreeSet::new();
    let store = ApprovedClientStore::new(pool.clone());
    assert!(matches!(
        store
            .approve_new_client(user, &metadata, &scopes, &scopes)
            .await,
        Err(ClientApprovalError::Forbidden)
    ));
    assert!(matches!(
        store
            .approve_new_client(admin, &metadata, &scopes, &empty)
            .await,
        Err(ClientApprovalError::InvalidScopes)
    ));
    let client = store
        .approve_new_client(admin, &metadata, &scopes, &scopes)
        .await
        .unwrap();
    assert!(matches!(
        store
            .approve_new_client(admin, &metadata, &scopes, &scopes)
            .await,
        Err(ClientApprovalError::AlreadyExists)
    ));
    let grants: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM oauth_grants WHERE client_id=?")
        .bind(client)
        .fetch_one(&pool)
        .await
        .unwrap();
    let (enabled, method): (bool, String) =
        sqlx::query_as("SELECT enabled,token_auth_method FROM oauth_clients WHERE id=?")
            .bind(client)
            .fetch_one(&pool)
            .await
            .unwrap();
    let redirects: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM oauth_client_redirects WHERE client_id=? AND uri='https://client.example.invalid/cb'").bind(client).fetch_one(&pool).await.unwrap();
    let audit: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM audit_events WHERE actor_id=? AND action='oauth.client.approve' AND target_id=?").bind(admin).bind(client.to_string()).fetch_one(&pool).await.unwrap();
    assert!(enabled);
    assert_eq!(method, "none");
    assert_eq!(grants, 0);
    assert_eq!(redirects, 1);
    assert_eq!(audit, 1);
}
