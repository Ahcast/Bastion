use std::collections::BTreeSet;

use bastion_core::{CoreService, Database};
use bastion_cotizaciones::QuoteService;
use bastion_facturas::InvoiceService;
use bastion_mcp::{AuthenticatedAccess, Credential};
use bastion_mcp_core::{DraftReadExecutor, ReadToolError};
use bastion_ordenes_compra::OrderService;
use serde_json::json;
use sqlx::mysql::MySqlPoolOptions;

#[tokio::test]
async fn mysql57_read_tools_reuse_domain_authorization_and_validate_arguments() {
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
    QuoteService::migrate(&db).await.unwrap();
    InvoiceService::migrate(&db).await.unwrap();
    OrderService::migrate(&db).await.unwrap();
    let core = CoreService::new(
        db.clone(),
        Credential::generate().unwrap().expose(),
        [17; 32],
    )
    .unwrap();
    let quotes = QuoteService::new(db.clone(), core.admin());
    let invoices = InvoiceService::new(db.clone(), core.admin());
    let orders = OrderService::new(db, core.admin());
    quotes.synchronize_release(true).await.unwrap();
    invoices.synchronize_release(true).await.unwrap();
    orders.synchronize_release(true).await.unwrap();
    sqlx::query("UPDATE module_registry SET enabled=1 WHERE module_id IN ('cotizaciones','facturas','ordenes_compra')").execute(&pool).await.unwrap();
    let role = sqlx::query("INSERT INTO roles (name) VALUES ('MCP read fixture role')")
        .execute(&pool)
        .await
        .unwrap()
        .last_insert_id();
    let person = sqlx::query("INSERT INTO persons (full_name) VALUES ('MCP read fixture')")
        .execute(&pool)
        .await
        .unwrap()
        .last_insert_id();
    let user = sqlx::query("INSERT INTO users (person_id,role_id,email,status) VALUES (?,?,'mcp-read-fixture@example.invalid','active')").bind(person).bind(role).execute(&pool).await.unwrap().last_insert_id();
    sqlx::query("INSERT INTO role_permissions (role_id,module_id,scope_key,scope_hash,capability) VALUES (?,'cotizaciones','documentos',UNHEX(SHA2('documentos',256)),'ver')").bind(role).execute(&pool).await.unwrap();
    let access = AuthenticatedAccess {
        user_id: user,
        client_id: 1,
        grant_id: 1,
        scopes: BTreeSet::from([
            "cotizaciones:read".into(),
            "facturas:read".into(),
            "ordenes_compra:read".into(),
        ]),
    };
    let executor = DraftReadExecutor::new(quotes, invoices, orders);
    let page = executor
        .call(&access, "cotizaciones.listar", json!({"page_size":5}))
        .await
        .unwrap();
    assert_eq!(page["page_size"], 5);
    assert!(page["items"].as_array().unwrap().is_empty());
    assert_eq!(
        executor.call(&access, "facturas.listar", json!({})).await,
        Err(ReadToolError::Denied)
    );
    assert_eq!(
        executor
            .call(&access, "ordenes_compra.listar", json!({}))
            .await,
        Err(ReadToolError::Denied)
    );
    assert_eq!(
        executor
            .call(&access, "cotizaciones.consultar", json!({"id":1}))
            .await,
        Err(ReadToolError::NotFound)
    );
    assert_eq!(
        executor
            .call(&access, "cotizaciones.listar", json!({"page_size":26}))
            .await,
        Err(ReadToolError::Invalid)
    );
    assert_eq!(
        executor
            .call(&access, "cotizaciones.listar", json!({"sql":"SELECT *"}))
            .await,
        Err(ReadToolError::Invalid)
    );
    let no_scope = AuthenticatedAccess {
        scopes: BTreeSet::new(),
        ..access
    };
    assert_eq!(
        executor
            .call(&no_scope, "cotizaciones.listar", json!({}))
            .await,
        Err(ReadToolError::Denied)
    );
}
