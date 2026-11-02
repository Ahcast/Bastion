use std::collections::BTreeSet;

use bastion_core::{CoreService, Database};
use bastion_cotizaciones::QuoteService;
use bastion_facturas::InvoiceService;
use bastion_licitaciones::TenderService;
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
    TenderService::migrate(&db).await.unwrap();
    let core = CoreService::new(
        db.clone(),
        Credential::generate().unwrap().expose(),
        [17; 32],
    )
    .unwrap();
    let quotes = QuoteService::new(db.clone(), core.admin());
    let invoices = InvoiceService::new(db.clone(), core.admin());
    let orders = OrderService::new(db.clone(), core.admin());
    let tenders = TenderService::new(db, core.admin());
    quotes.synchronize_release(true).await.unwrap();
    invoices.synchronize_release(true).await.unwrap();
    orders.synchronize_release(true).await.unwrap();
    tenders.synchronize_release(true).await.unwrap();
    sqlx::query("UPDATE module_registry SET enabled=1 WHERE module_id IN ('cotizaciones','facturas','ordenes_compra','licitaciones')").execute(&pool).await.unwrap();
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
            "cotizaciones:write".into(),
            "facturas:write".into(),
            "ordenes_compra:write".into(),
            "licitaciones:read".into(),
            "licitaciones:write".into(),
        ]),
    };
    let executor = DraftReadExecutor::new(quotes, invoices, orders).with_tenders(tenders);
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
    for module in ["facturas", "ordenes_compra"] {
        sqlx::query("INSERT INTO role_permissions (role_id,module_id,scope_key,scope_hash,capability) VALUES (?,?,'documentos',UNHEX(SHA2('documentos',256)),'ver')").bind(role).bind(module).execute(&pool).await.unwrap();
        let page = executor
            .call(&access, &format!("{module}.listar"), json!({"page_size":5}))
            .await
            .unwrap();
        assert_eq!(page["page_size"], 5);
        assert!(page["items"].as_array().unwrap().is_empty());
        assert_eq!(
            executor
                .call(&access, &format!("{module}.consultar"), json!({"id":1}))
                .await,
            Err(ReadToolError::NotFound)
        );
    }
    for (module, scope) in [
        ("cotizaciones", "documentos"),
        ("facturas", "documentos"),
        ("ordenes_compra", "documentos"),
        ("licitaciones", "expedientes"),
    ] {
        for capability in ["ver", "editar"] {
            sqlx::query("INSERT IGNORE INTO role_permissions (role_id,module_id,scope_key,scope_hash,capability) VALUES (?,?,?,UNHEX(SHA2(?,256)),?)").bind(role).bind(module).bind(scope).bind(scope).bind(capability).execute(&pool).await.unwrap();
        }
    }
    let quote = json!({"client_name":"Fixture", "order_number":"MCP", "quote_date":"2026-10-22", "valid_days":30, "notes":"", "terms":"", "discount_type":"amount", "discount_value":"0", "items":[]});
    let create = json!({"idempotency_key":"quote-fixture-key-001", "data":quote});
    let first = executor
        .call(&access, "cotizaciones.crear_borrador", create.clone())
        .await
        .unwrap();
    let replay = executor
        .call(&access, "cotizaciones.crear_borrador", create)
        .await
        .unwrap();
    assert_eq!(first["id"], replay["id"]);
    let id = first["id"].as_u64().unwrap();
    let mut invalid = first["data"].clone();
    invalid["status"] = json!("Enviada");
    assert_eq!(
        executor
            .call(
                &access,
                "cotizaciones.editar_borrador",
                json!({"id":id,"version":1,"data":invalid})
            )
            .await,
        Err(ReadToolError::Invalid)
    );
    let updated = executor
        .call(
            &access,
            "cotizaciones.editar_borrador",
            json!({"id":id,"version":1,"data":first["data"]}),
        )
        .await
        .unwrap();
    assert_eq!(updated["version"], 2);
    assert_eq!(
        executor
            .call(
                &access,
                "cotizaciones.editar_borrador",
                json!({"id":id,"version":1,"data":first["data"]})
            )
            .await,
        Err(ReadToolError::Conflict)
    );
    sqlx::query("UPDATE cotizaciones_drafts SET data_json=JSON_SET(data_json,'$.status','Enviada'),version=3 WHERE id=?").bind(id).execute(&pool).await.unwrap();
    assert_eq!(
        executor
            .call(
                &access,
                "cotizaciones.editar_borrador",
                json!({"id":id,"version":3,"data":first["data"]})
            )
            .await,
        Err(ReadToolError::Conflict)
    );
    let invoice = json!({"client_name":"Fixture", "order_number":"MCP", "invoice_date":"2026-10-22", "payment_term_type":"credito", "credit_days":30, "discount_type":"amount", "discount_value":"0", "notes":"", "public_notes":"", "expenses":[], "items":[]});
    let mut payment = invoice.clone();
    payment["initial_payment"] =
        json!({"amount":"1","date":"2026-10-22","method":"Efectivo","reference":""});
    assert_eq!(
        executor
            .call(
                &access,
                "facturas.crear_borrador",
                json!({"idempotency_key":"invoice-fixture-key-001","data":payment})
            )
            .await,
        Err(ReadToolError::Invalid)
    );
    let invoice = executor
        .call(
            &access,
            "facturas.crear_borrador",
            json!({"idempotency_key":"invoice-fixture-key-001","data":invoice}),
        )
        .await
        .unwrap();
    assert_eq!(invoice["status"], "Borrador");
    let invoice_id = invoice["id"].as_u64().unwrap();
    sqlx::query("UPDATE facturas_drafts SET status='Enviado',version=2 WHERE id=?")
        .bind(invoice_id)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        executor
            .call(
                &access,
                "facturas.editar_borrador",
                json!({"id":invoice_id,"version":2,"data":invoice["data"]})
            )
            .await,
        Err(ReadToolError::Conflict)
    );
    let destination = sqlx::query("INSERT INTO ordenes_destinations (destination,name_key) VALUES ('MCP fixture','mcp fixture')").execute(&pool).await.unwrap().last_insert_id();
    let order = json!({"n_orders":"OC-MCP","n_bill":"","order_date":"2026-10-23","received_date":null,"client_name":"Fixture","client_rtn":"","id_destination":destination,"comments":"","products":[{"description":"Equipo","cantidad":1,"sale_price":"60","purchase_price":"100","tracking":"","isv":true,"status":"Pendiente","coments":""}],"expenses":[]});
    let mut purchased = order.clone();
    purchased["products"][0]["status"] = json!("Comprado");
    assert_eq!(
        executor
            .call(
                &access,
                "ordenes_compra.crear_pendiente",
                json!({"idempotency_key":"orders-fixture-key-001","data":purchased})
            )
            .await,
        Err(ReadToolError::Invalid)
    );
    let order = executor
        .call(
            &access,
            "ordenes_compra.crear_pendiente",
            json!({"idempotency_key":"orders-fixture-key-001","data":order}),
        )
        .await
        .unwrap();
    assert_eq!(order["status"], "Pendiente");
    let order_id = order["id"].as_u64().unwrap();
    let mut delivered = order["data"].clone();
    delivered["n_bill"] = json!("FAC-1");
    assert_eq!(
        executor
            .call(
                &access,
                "ordenes_compra.editar_pendiente",
                json!({"id":order_id,"version":1,"data":delivered})
            )
            .await,
        Err(ReadToolError::Invalid)
    );
    let order = executor
        .call(
            &access,
            "ordenes_compra.editar_pendiente",
            json!({"id":order_id,"version":1,"data":order["data"]}),
        )
        .await
        .unwrap();
    assert_eq!(order["version"], 2);
    sqlx::query("UPDATE ordenes_orders SET calculation_json=JSON_SET(calculation_json,'$.data.products[0].status','Comprado'),version=3 WHERE id=?").bind(order_id).execute(&pool).await.unwrap();
    assert_eq!(
        executor
            .call(
                &access,
                "ordenes_compra.editar_pendiente",
                json!({"id":order_id,"version":3,"data":order["data"]})
            )
            .await,
        Err(ReadToolError::Conflict)
    );
    let tender = executor
        .call(
            &access,
            "licitaciones.crear_borrador",
            json!({"idempotency_key":"tender-fixture-key-001"}),
        )
        .await
        .unwrap();
    assert_eq!(tender["state"], "borrador");
    let tender_id = tender["id"].as_u64().unwrap();
    let renamed = executor
        .call(
            &access,
            "licitaciones.renombrar_borrador",
            json!({"id":tender_id,"version":1,"name":"Oferta de prueba"}),
        )
        .await
        .unwrap();
    assert_eq!(renamed["name"], "Oferta de prueba");
    let variables = json!([
        {"key":"lotes","label":"Lotes","kind":"integer","value":"1"},
        {"key":"stylo","label":"Estilo","kind":"text","value":"bid"}
    ]);
    let configured = executor
        .call(
            &access,
            "licitaciones.editar_variables",
            json!({"id":tender_id,"version":2,"variables":variables}),
        )
        .await
        .unwrap();
    assert_eq!(configured["version"], 3);
    let item = json!({"descripcion":"Equipo","pais_origen":"Honduras","fecha_entrega":"30 días","cantidad":"1","unidad":"und","transporte":"10.00","precio_unitario":"100.00"});
    let edited = executor
        .call(
            &access,
            "licitaciones.editar_lote",
            json!({"id":tender_id,"version":3,"numero":1,"items":[item]}),
        )
        .await
        .unwrap();
    assert_eq!(edited["version"], 4);
    let lot = executor
        .call(
            &access,
            "licitaciones.consultar_lote",
            json!({"id":tender_id,"numero":1}),
        )
        .await
        .unwrap();
    assert_eq!(lot["items"].as_array().unwrap().len(), 1);
    assert_eq!(
        executor
            .call(
                &access,
                "licitaciones.editar_variables",
                json!({"id":tender_id,"version":4,"variables":[]})
            )
            .await,
        Err(ReadToolError::Conflict)
    );
    assert_eq!(
        executor
            .call(
                &access,
                "licitaciones.renombrar_borrador",
                json!({"id":tender_id,"version":1,"name":"Otro"})
            )
            .await,
        Err(ReadToolError::Conflict)
    );
    let page = executor
        .call(&access, "licitaciones.listar", json!({}))
        .await
        .unwrap();
    assert_eq!(page["items"].as_array().unwrap().len(), 1);
    assert_eq!(
        executor
            .call(&access, "licitaciones.listar", json!({"search":"x"}))
            .await,
        Err(ReadToolError::Invalid)
    );
    let write_without_read = AuthenticatedAccess {
        user_id: access.user_id,
        client_id: access.client_id,
        grant_id: access.grant_id,
        scopes: BTreeSet::from(["cotizaciones:write".into()]),
    };
    assert_eq!(
        executor
            .call(
                &write_without_read,
                "cotizaciones.crear_borrador",
                json!({})
            )
            .await,
        Err(ReadToolError::Denied)
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
