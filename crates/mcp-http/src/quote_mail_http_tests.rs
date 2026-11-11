use super::*;
use axum::{
    body::{Body, to_bytes},
    http::{
        Request,
        header::{AUTHORIZATION, CONTENT_TYPE, COOKIE, ORIGIN},
    },
};
use bastion_core::Database;
use bastion_cotizaciones::{QuoteInput, QuoteService};
use bastion_mcp::{AuthenticatedAccess, Credential};
use bastion_mcp_core::DraftReadExecutor;
use serde_json::{Value, json};
use sqlx::mysql::MySqlPoolOptions;
use std::collections::BTreeSet;
use tower::ServiceExt;

#[tokio::test]
async fn mysql57_confirmation_is_human_bound_and_queues_only_once() {
    let Ok(url) = std::env::var("BASTION_MCP_TEST_URL") else {
        return;
    };
    assert_eq!(
        std::env::var("BASTION_DOCUMENTS_BUCKET").unwrap(),
        "bastion-synthetic-attachment-test"
    );
    let pool = MySqlPoolOptions::new()
        .max_connections(8)
        .connect(&url)
        .await
        .unwrap();
    let db = Database::from_test_pool(pool.clone());
    db.migrate().await.unwrap();
    QuoteService::migrate(&db).await.unwrap();
    let core = CoreService::new(db.clone(), "x".repeat(32), [78; 32]).unwrap();
    let role: u64 =
        sqlx::query_scalar("SELECT id FROM roles WHERE system_key='system_administrator'")
            .fetch_one(&pool)
            .await
            .unwrap();
    let mut identities = Vec::new();
    for index in 0..2 {
        let person = sqlx::query("INSERT INTO persons (full_name) VALUES ('MCP mail fixture')")
            .execute(&pool)
            .await
            .unwrap()
            .last_insert_id();
        let actor = sqlx::query(
            "INSERT INTO users (person_id,role_id,email,status) VALUES (?,?,?,'active')",
        )
        .bind(person)
        .bind(role)
        .bind(format!("mcp-mail-{index}@example.invalid"))
        .execute(&pool)
        .await
        .unwrap()
        .last_insert_id();
        let session = Credential::generate().unwrap().expose();
        let csrf = Credential::generate().unwrap().expose();
        sqlx::query("INSERT INTO sessions (user_id,token_hash,csrf_hash,ip_address,started_at,last_activity_at,last_totp_at,expires_at) VALUES (?,UNHEX(SHA2(?,256)),UNHEX(SHA2(?,256)),'127.0.0.1',UTC_TIMESTAMP(6),UTC_TIMESTAMP(6),UTC_TIMESTAMP(6),UTC_TIMESTAMP(6)+INTERVAL 1 HOUR)")
            .bind(actor).bind(&session).bind(&csrf).execute(&pool).await.unwrap();
        identities.push((actor, session, csrf));
    }
    let actor = identities[0].0;
    let quotes = QuoteService::new(db.clone(), core.admin());
    quotes.synchronize_release(true).await.unwrap();
    core.admin()
        .set_module_enabled(actor, "cotizaciones", true)
        .await
        .unwrap();
    for (key, value) in [
        ("company.profile", json!({"name":"Empresa de prueba"})),
        ("smtp.host", json!("mail.example.invalid")),
        ("smtp.port", json!(587)),
        ("smtp.tls_mode", json!("starttls_required")),
    ] {
        core.admin()
            .set_core_setting(actor, key, value)
            .await
            .unwrap();
    }
    // A deliberately unusable SMTP credential ensures this fixture can only
    // enqueue. No mail worker is started by these tests.
    sqlx::query("INSERT INTO user_smtp_credentials (user_id,smtp_email,password_ciphertext) VALUES (?,'sender@example.invalid',X'01')")
        .bind(actor).execute(&pool).await.unwrap();
    let client=sqlx::query("INSERT INTO oauth_clients (client_id,client_id_hash,display_name,metadata_hash,approved_metadata_hash,enabled,created_at,updated_at) VALUES ('https://mcp-mail.example.invalid/metadata',UNHEX(SHA2('mcp-mail-fixture',256)),'Mail fixture',UNHEX(SHA2('mail',256)),UNHEX(SHA2('mail',256)),1,UTC_TIMESTAMP(6),UTC_TIMESTAMP(6))")
        .execute(&pool).await.unwrap().last_insert_id();
    for scope in ["cotizaciones:read", "cotizaciones:write"] {
        sqlx::query("INSERT INTO oauth_client_scopes (client_id,scope_name) VALUES (?,?)")
            .bind(client)
            .bind(scope)
            .execute(&pool)
            .await
            .unwrap();
    }
    let grant=sqlx::query("INSERT INTO oauth_grants (user_id,client_id,resource_uri,scopes_json,issued_at,expires_at) VALUES (?,?,'https://api.example.invalid/mcp',JSON_ARRAY('cotizaciones:read','cotizaciones:write'),UTC_TIMESTAMP(6),UTC_TIMESTAMP(6)+INTERVAL 1 DAY)")
        .bind(actor).bind(client).execute(&pool).await.unwrap().last_insert_id();
    let access = AuthenticatedAccess {
        user_id: actor,
        client_id: client,
        grant_id: grant,
        scopes: BTreeSet::from(["cotizaciones:read".into(), "cotizaciones:write".into()]),
    };
    let input:QuoteInput=serde_json::from_value(json!({"client_name":"Cliente de prueba","order_number":"MCP-TEST","quote_date":"2026-11-11","valid_days":30,"notes":"","terms":"","discount_type":"amount","discount_value":"0","items":[{"description":"Producto","quantity":"1","unit_price":"100","discount_type":"amount","discount_value":"0","tax_type":"isv15"}]})).unwrap();
    let quote = quotes
        .create(actor, "mcp-mail-quote-fixture-01", input)
        .await
        .unwrap();
    let mail = QuoteMailConfirmation::new(core.clone(), "https://app.example.invalid".into());
    let executor = DraftReadExecutor::new(
        quotes.clone(),
        bastion_facturas::InvoiceService::new(db.clone(), core.admin()),
        bastion_ordenes_compra::OrderService::new(db, core.admin()),
    )
    .with_quote_mail(mail.clone());
    let args = json!({"id":quote.id,"version":quote.version,"data":{"to":"recipient@example.invalid","cc":"","bcc":"","subject":"Confirmación privada","message":"Texto privado","scheduled_at":"2099-11-11T15:30:00-06:00"}});
    let proposal = executor
        .call(&access, "cotizaciones.preparar_envio", args.clone())
        .await
        .unwrap();
    let handle = proposal["handle"].as_str().unwrap();
    assert!(
        proposal["confirmation_url"]
            .as_str()
            .unwrap()
            .starts_with("https://app.example.invalid/mcp/quote-email#")
    );
    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM cotizaciones_email_jobs WHERE actor_id=?")
            .bind(actor)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, 0);
    let encrypted: Vec<u8> = sqlx::query_scalar(
        "SELECT payload_ciphertext FROM mcp_quote_email_confirmations WHERE user_id=?",
    )
    .bind(actor)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(!String::from_utf8_lossy(&encrypted).contains("Texto privado"));
    let router = quote_mail_router(QuoteMailState {
        core: core.clone(),
        origin: "https://app.example.invalid".into(),
        mail: mail.clone(),
    });
    let request = |path: &str, body: Value, who: usize, csrf: bool| {
        let identity = &identities[who];
        let mut builder = Request::builder()
            .method("POST")
            .uri(path)
            .header(CONTENT_TYPE, "application/json")
            .header(ORIGIN, "https://app.example.invalid")
            .header(
                COOKIE,
                format!(
                    "__Host-bastion_session={}; __Host-bastion_csrf={}",
                    identity.1, identity.2
                ),
            );
        if csrf {
            builder = builder.header("x-csrf-token", &identity.2);
        }
        builder.body(Body::from(body.to_string())).unwrap()
    };
    let review = "/oauth/quote-email/review";
    let decision = "/oauth/quote-email/decision";
    let body = json!({"handle":handle});
    assert_eq!(
        router
            .clone()
            .oneshot(request(review, body.clone(), 1, true))
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        router
            .clone()
            .oneshot(request(review, body.clone(), 0, false))
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
    let bearer = Request::builder()
        .method("POST")
        .uri(decision)
        .header(CONTENT_TYPE, "application/json")
        .header(AUTHORIZATION, "Bearer synthetic")
        .body(Body::from(
            json!({"handle":handle,"decision":"approve"}).to_string(),
        ))
        .unwrap();
    assert_eq!(
        router.clone().oneshot(bearer).await.unwrap().status(),
        StatusCode::FORBIDDEN
    );
    let response = router
        .clone()
        .oneshot(request(review, body, 0, true))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let data: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 65536).await.unwrap()).unwrap();
    assert_eq!(data["input"]["subject"], "Confirmación privada");
    let tampered = json!({"handle":handle,"decision":"approve","to":"different@example.invalid"});
    assert_eq!(
        router
            .clone()
            .oneshot(request(decision, tampered, 0, true))
            .await
            .unwrap()
            .status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    for _ in 0..2 {
        let response = router
            .clone()
            .oneshot(request(
                decision,
                json!({"handle":handle,"decision":"approve"}),
                0,
                true,
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }
    assert_eq!(
        mail.status(&access, handle).await.unwrap()["job"]["delivery_status"],
        "pending"
    );
    let jobs: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM cotizaciones_email_jobs WHERE actor_id=?")
            .bind(actor)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(jobs, 1);
    // Recover a crash after queue commit but before persisting the receipt,
    // including when the confirmation link has subsequently expired.
    sqlx::query("UPDATE mcp_quote_email_confirmations SET job_id=NULL,expires_at=UTC_TIMESTAMP(6)-INTERVAL 1 SECOND WHERE user_id=? AND confirmed_at IS NOT NULL")
        .bind(actor).execute(&pool).await.unwrap();
    assert_eq!(
        mail.status(&access, handle).await.unwrap()["job"]["delivery_status"],
        "pending"
    );
    assert_eq!(
        mail.decide(&identities[0].1, handle, true).await.unwrap()["status"],
        "queued"
    );
    let proposal = executor
        .call(&access, "cotizaciones.preparar_envio", args.clone())
        .await
        .unwrap();
    let denied = proposal["handle"].as_str().unwrap();
    assert_eq!(
        mail.decide(&identities[0].1, denied, false).await.unwrap()["status"],
        "denied"
    );
    assert!(mail.decide(&identities[0].1, denied, true).await.is_err());
    let proposal = executor
        .call(&access, "cotizaciones.preparar_envio", args)
        .await
        .unwrap();
    let expired = proposal["handle"].as_str().unwrap();
    sqlx::query("UPDATE mcp_quote_email_confirmations SET expires_at=UTC_TIMESTAMP(6)-INTERVAL 1 SECOND WHERE user_id=? AND confirmed_at IS NULL").bind(actor).execute(&pool).await.unwrap();
    assert!(mail.decide(&identities[0].1, expired, true).await.is_err());
    sqlx::query("UPDATE oauth_grants SET revoked_at=UTC_TIMESTAMP(6) WHERE id=?")
        .bind(grant)
        .execute(&pool)
        .await
        .unwrap();
    assert!(mail.status(&access, handle).await.is_err());
}
