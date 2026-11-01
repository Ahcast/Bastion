use bastion_core::Database;
use bastion_mcp::{
    ClientRequestError, ClientRequestStore, ReviewDecision, validate_client_metadata,
};
use sqlx::mysql::MySqlPoolOptions;
use std::collections::BTreeSet;

#[tokio::test]
async fn mysql57_client_requests_require_admin_and_never_create_user_grants() {
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
    let admin_role: u64 =
        sqlx::query_scalar("SELECT id FROM roles WHERE system_key='system_administrator'")
            .fetch_one(&pool)
            .await
            .unwrap();
    let role = sqlx::query("INSERT INTO roles (name) VALUES ('OAuth request fixture role')")
        .execute(&pool)
        .await
        .unwrap()
        .last_insert_id();
    let admin_person =
        sqlx::query("INSERT INTO persons (full_name) VALUES ('OAuth request fixture admin')")
            .execute(&pool)
            .await
            .unwrap()
            .last_insert_id();
    let admin=sqlx::query("INSERT INTO users (person_id,role_id,email,status) VALUES (?,?,'oauth-request-admin@example.invalid','active')").bind(admin_person).bind(admin_role).execute(&pool).await.unwrap().last_insert_id();
    let person =
        sqlx::query("INSERT INTO persons (full_name) VALUES ('OAuth request fixture user')")
            .execute(&pool)
            .await
            .unwrap()
            .last_insert_id();
    let user=sqlx::query("INSERT INTO users (person_id,role_id,email,status) VALUES (?,?,'oauth-request-user@example.invalid','active')").bind(person).bind(role).execute(&pool).await.unwrap().last_insert_id();
    let id = "https://request-client.example.invalid/metadata.json";
    let raw=serde_json::json!({"client_id":id,"client_name":"Request fixture","redirect_uris":["https://request-client.example.invalid/cb"],"token_endpoint_auth_method":"none"}).to_string();
    let metadata = validate_client_metadata(id, id, 200, raw.as_bytes()).unwrap();
    let changed = validate_client_metadata(id, id, 200, format!("{raw} ").as_bytes()).unwrap();
    let registered = BTreeSet::from(["quote:read".to_owned(), "invoice:read".to_owned()]);
    let quote = BTreeSet::from(["quote:read".to_owned()]);
    let invoice = BTreeSet::from(["invoice:read".to_owned()]);
    let store = ClientRequestStore::new(pool.clone());
    assert!(matches!(
        store
            .submit(
                user,
                &metadata,
                &BTreeSet::from(["unknown".into()]),
                &registered
            )
            .await,
        Err(ClientRequestError::Invalid)
    ));
    let request = store
        .submit(user, &metadata, &quote, &registered)
        .await
        .unwrap();
    assert_eq!(
        store
            .submit(user, &metadata, &quote, &registered)
            .await
            .unwrap(),
        request
    );
    assert!(matches!(
        store.submit(user, &metadata, &invoice, &registered).await,
        Err(ClientRequestError::Conflict)
    ));
    assert!(matches!(
        store
            .review(
                user,
                request,
                ReviewDecision::Approve {
                    metadata: &metadata,
                    scopes: &quote
                },
                &registered
            )
            .await,
        Err(ClientRequestError::Forbidden)
    ));
    assert!(matches!(
        store
            .review(
                admin,
                request,
                ReviewDecision::Approve {
                    metadata: &changed,
                    scopes: &quote
                },
                &registered
            )
            .await,
        Err(ClientRequestError::Invalid)
    ));
    store
        .review(
            admin,
            request,
            ReviewDecision::Approve {
                metadata: &metadata,
                scopes: &quote,
            },
            &registered,
        )
        .await
        .unwrap();
    let (enabled, approved): (bool, bool) = sqlx::query_as(
        "SELECT enabled,approved_metadata_hash=metadata_hash FROM oauth_clients WHERE client_id=?",
    )
    .bind(id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(enabled && approved);
    let grants: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM oauth_grants WHERE user_id=?")
        .bind(user)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(grants, 0);
    assert!(matches!(
        store
            .review(admin, request, ReviewDecision::Reject, &registered)
            .await,
        Err(ClientRequestError::Conflict)
    ));
    assert!(matches!(
        store.submit(user, &metadata, &quote, &registered).await,
        Err(ClientRequestError::AlreadyApproved)
    ));
    let second = store
        .submit(user, &metadata, &invoice, &registered)
        .await
        .unwrap();
    store
        .review(admin, second, ReviewDecision::Reject, &registered)
        .await
        .unwrap();
    let status: String = sqlx::query_scalar("SELECT status FROM oauth_client_requests WHERE id=?")
        .bind(second)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(status, "rejected");
    let third = store
        .submit(user, &metadata, &invoice, &registered)
        .await
        .unwrap();
    assert_ne!(third, second);
    let client: u64 = sqlx::query_scalar("SELECT id FROM oauth_clients WHERE client_id=?")
        .bind(id)
        .fetch_one(&pool)
        .await
        .unwrap();
    let stale=sqlx::query("INSERT INTO oauth_grants (user_id,client_id,resource_uri,scopes_json,issued_at,expires_at) VALUES (?,?,'https://example.invalid/mcp',JSON_ARRAY('quote:read'),UTC_TIMESTAMP(6),UTC_TIMESTAMP(6)+INTERVAL 1 DAY)").bind(user).bind(client).execute(&pool).await.unwrap().last_insert_id();
    sqlx::query("UPDATE oauth_clients SET enabled=0 WHERE id=?")
        .bind(client)
        .execute(&pool)
        .await
        .unwrap();
    store
        .review(
            admin,
            third,
            ReviewDecision::Approve {
                metadata: &metadata,
                scopes: &invoice,
            },
            &registered,
        )
        .await
        .unwrap();
    let revoked: bool =
        sqlx::query_scalar("SELECT revoked_at IS NOT NULL FROM oauth_grants WHERE id=?")
            .bind(stale)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(revoked);
}
