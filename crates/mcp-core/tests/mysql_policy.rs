use std::collections::{BTreeMap, BTreeSet};

use bastion_core::{CoreService, Database};
use bastion_mcp::ExchangePolicy;
use bastion_mcp_core::{CoreScopePolicy, PermissionRequirement};
use sqlx::mysql::MySqlPoolOptions;

#[tokio::test]
async fn mysql57_checks_registered_scope_current_permission_and_module_state() {
    let Ok(url) = std::env::var("BASTION_MCP_TEST_URL") else {
        return;
    };
    let pool = MySqlPoolOptions::new()
        .max_connections(3)
        .connect(&url)
        .await
        .unwrap();
    let database = Database::from_test_pool(pool.clone());
    database.migrate().await.unwrap();
    let person = sqlx::query("INSERT INTO persons (full_name) VALUES ('OAuth policy test')")
        .execute(&pool)
        .await
        .unwrap()
        .last_insert_id();
    let role = sqlx::query("INSERT INTO roles (name) VALUES ('OAuth policy role')")
        .execute(&pool)
        .await
        .unwrap()
        .last_insert_id();
    let user = sqlx::query("INSERT INTO users (person_id,role_id,email,status) VALUES (?,?,'oauth-policy@example.invalid','active')").bind(person).bind(role).execute(&pool).await.unwrap().last_insert_id();
    sqlx::query("INSERT INTO module_registry (module_id,display_name,included_in_release,environment_allowed,enabled) VALUES ('mcp_policy_fixture','Policy fixture',1,1,1)").execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO role_permissions (role_id,module_id,scope_key,scope_hash,capability) VALUES (?,'mcp_policy_fixture','documentos',UNHEX(SHA2('documentos',256)),'ver')").bind(role).execute(&pool).await.unwrap();
    let core = CoreService::new(database, "test-bootstrap-secret".repeat(2), [7; 32]).unwrap();
    let requirement = PermissionRequirement {
        module_id: "mcp_policy_fixture".into(),
        scope_key: "documentos".into(),
        capability: "ver".into(),
    };
    let policy = CoreScopePolicy::new(
        core.admin(),
        BTreeMap::from([("fixture:read".into(), vec![requirement])]),
    )
    .unwrap();
    let requested = BTreeSet::from(["fixture:read".to_owned()]);
    let mut tx = pool.begin().await.unwrap();
    assert!(
        policy
            .authorize(&mut tx, user, 0, &requested)
            .await
            .unwrap()
    );
    assert!(
        !policy
            .authorize(&mut tx, user, 0, &BTreeSet::from(["unknown".to_owned()]))
            .await
            .unwrap()
    );
    tx.rollback().await.unwrap();
    sqlx::query("DELETE FROM role_permissions WHERE role_id=?")
        .bind(role)
        .execute(&pool)
        .await
        .unwrap();
    let mut tx = pool.begin().await.unwrap();
    assert!(
        !policy
            .authorize(&mut tx, user, 0, &requested)
            .await
            .unwrap()
    );
    tx.rollback().await.unwrap();
    sqlx::query("INSERT INTO role_permissions (role_id,module_id,scope_key,scope_hash,capability) VALUES (?,'mcp_policy_fixture','documentos',UNHEX(SHA2('documentos',256)),'ver')").bind(role).execute(&pool).await.unwrap();
    sqlx::query("UPDATE module_registry SET enabled=0 WHERE module_id='mcp_policy_fixture'")
        .execute(&pool)
        .await
        .unwrap();
    let mut tx = pool.begin().await.unwrap();
    assert!(
        !policy
            .authorize(&mut tx, user, 0, &requested)
            .await
            .unwrap()
    );
    tx.rollback().await.unwrap();
}
