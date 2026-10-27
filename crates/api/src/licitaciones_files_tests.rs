use super::*;
use axum::body::{Body, to_bytes};
use base64::{Engine, engine::general_purpose::STANDARD};
use bastion_core::Database;
use bastion_licitaciones::TenderService;
use tower::ServiceExt;

#[tokio::test]
async fn mysql57_tender_files_http_private_upload_copy_and_limits() {
    let Ok(url) = std::env::var("BASTION_TENDER_FILES_HTTP_TEST_URL") else {
        eprintln!("Requires isolated tender file HTTP DB; CI supplies it.");
        return;
    };
    let pool = sqlx::mysql::MySqlPoolOptions::new()
        .max_connections(6)
        .connect(&url)
        .await
        .unwrap();
    let db = Database::from_test_pool(pool.clone());
    db.migrate().await.unwrap();
    TenderService::migrate(&db).await.unwrap();
    bastion_colaboracion::CollaborationService::migrate(&db)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO persons (id,full_name) VALUES (1,'HTTP synthetic admin'),(2,'HTTP denied')",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO roles (id,name) VALUES (2,'HTTP denied')")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO users (id,person_id,role_id,email,status) VALUES (1,1,1,'admin@example.test','active'),(2,2,2,'denied@example.test','active')").execute(&pool).await.unwrap();
    for (id, token) in [
        (1, "tender-files-http-admin"),
        (2, "tender-files-http-denied"),
    ] {
        sqlx::query("INSERT INTO sessions (user_id,token_hash,csrf_hash,ip_address,started_at,last_activity_at,last_totp_at,expires_at) VALUES (?,UNHEX(SHA2(?,256)),UNHEX(SHA2('synthetic-csrf',256)),'127.0.0.1',UTC_TIMESTAMP(6),UTC_TIMESTAMP(6),UTC_TIMESTAMP(6),UTC_TIMESTAMP(6)+INTERVAL 1 HOUR)").bind(id).bind(token).execute(&pool).await.unwrap();
    }
    let core = CoreService::new(db.clone(), "x".repeat(32), [1; 32]).unwrap();
    let service = TenderService::new(db.clone(), core.admin());
    service.synchronize_release(true).await.unwrap();
    let library = bastion_colaboracion::CollaborationService::new(db, core.admin());
    library.synchronize_release(true).await.unwrap();
    core.admin()
        .set_module_enabled(1, "licitaciones", true)
        .await
        .unwrap();
    core.admin()
        .set_module_enabled(1, "colaboracion", true)
        .await
        .unwrap();
    let record = service.create(1, "http-files-create001").await.unwrap();
    let other = service.create(1, "http-files-create002").await.unwrap();
    let app = router(
        core.clone(),
        "https://example.test".into(),
        Vec::new(),
        "/unused".into(),
    )
    .unwrap();
    let request = |method: &str,
                   path: &str,
                   actor: Option<&str>,
                   csrf: bool,
                   version: u64,
                   key: &str,
                   body: String| {
        let mut b = axum::http::Request::builder()
            .method(method)
            .uri(path)
            .header("origin", "https://example.test")
            .header("content-type", "application/json")
            .header("if-match", format!("\"{version}\""))
            .header("idempotency-key", key);
        if let Some(token) = actor {
            b = b.header(
                "cookie",
                format!("__Host-bastion_session={token}; __Host-bastion_csrf=synthetic-csrf"),
            );
        }
        if csrf {
            b = b.header("x-csrf-token", "synthetic-csrf");
        }
        let mut r = b.body(Body::from(body)).unwrap();
        r.extensions_mut()
            .insert(ConnectInfo("127.0.0.1:5001".parse::<SocketAddr>().unwrap()));
        r
    };
    let path = format!("/api/v1/licitaciones/{}/archivos", record.id);
    let admin = Some("tender-files-http-admin");
    for (actor, status) in [
        (None, StatusCode::UNAUTHORIZED),
        (Some("tender-files-http-denied"), StatusCode::FORBIDDEN),
    ] {
        assert_eq!(
            app.clone()
                .oneshot(request(
                    "GET",
                    &path,
                    actor,
                    false,
                    1,
                    "tender-http-get-key1",
                    "".into()
                ))
                .await
                .unwrap()
                .status(),
            status
        );
    }
    let bytes = b"%PDF-1.4\n1 0 obj\n<< /Type /Catalog >>\nendobj\n%%EOF\n";
    let payload=serde_json::json!({"section":"legal","filename":"legal.pdf","content_base64":STANDARD.encode(bytes)}).to_string();
    assert_eq!(
        app.clone()
            .oneshot(request(
                "POST",
                &path,
                admin,
                false,
                1,
                "tender-http-upload01",
                payload.clone()
            ))
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
    let added = app
        .clone()
        .oneshot(request(
            "POST",
            &path,
            admin,
            true,
            1,
            "tender-http-upload01",
            payload.clone(),
        ))
        .await
        .unwrap();
    assert_eq!(added.status(), StatusCode::OK);
    assert_eq!(added.headers()["etag"], "\"2\"");
    assert_eq!(
        app.clone()
            .oneshot(request(
                "POST",
                &path,
                admin,
                true,
                1,
                "tender-http-upload01",
                payload
            ))
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    let response = app
        .clone()
        .oneshot(request(
            "GET",
            &path,
            admin,
            false,
            1,
            "tender-http-list-key",
            "".into(),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let files: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 1_000_000).await.unwrap()).unwrap();
    assert_eq!(files["items"].as_array().unwrap().len(), 1);
    let file = files["items"][0]["id"].as_str().unwrap();
    assert!(files["items"][0].get("object_key").is_none());
    let preview = format!("{path}/{file}/preview");
    let response = app
        .clone()
        .oneshot(request(
            "GET",
            &preview,
            admin,
            false,
            2,
            "tender-http-read-key",
            "".into(),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let content: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 1_000_000).await.unwrap()).unwrap();
    assert_eq!(content["mime"], "application/pdf");
    assert_eq!(content["content_base64"], STANDARD.encode(bytes));
    let cross = format!("/api/v1/licitaciones/{}/archivos/{file}/download", other.id);
    assert_eq!(
        app.clone()
            .oneshot(request(
                "GET",
                &cross,
                admin,
                false,
                1,
                "tender-http-crosskey",
                "".into()
            ))
            .await
            .unwrap()
            .status(),
        StatusCode::NOT_FOUND
    );
    let wrong=serde_json::json!({"section":"legal","filename":"fake.png","content_base64":STANDARD.encode(bytes)}).to_string();
    assert_eq!(
        app.clone()
            .oneshot(request(
                "POST",
                &path,
                admin,
                true,
                2,
                "tender-http-fake-key",
                wrong
            ))
            .await
            .unwrap()
            .status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        app.clone()
            .oneshot(request(
                "POST",
                &path,
                admin,
                true,
                2,
                "tender-http-largekey",
                " ".repeat(27_000_001)
            ))
            .await
            .unwrap()
            .status(),
        StatusCode::PAYLOAD_TOO_LARGE
    );
    let source = library
        .upload(
            1,
            "http-source-upload01",
            bastion_colaboracion::UploadInput {
                scope: "empresa".into(),
                id_persona: None,
                title: "Synthetic source".into(),
                notes: String::new(),
                filename: "source.pdf".into(),
                content_base64: STANDARD.encode(bytes),
            },
        )
        .await
        .unwrap();
    let copied = serde_json::json!({"section":"tecnica","source_id":source.id}).to_string();
    assert_eq!(
        app.clone()
            .oneshot(request(
                "POST",
                &(path.clone() + "/desde-colaboracion"),
                admin,
                true,
                2,
                "tender-http-copy-key",
                copied
            ))
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    let remove = format!("{path}/{file}");
    let confirmation = serde_json::json!({"confirm_id":file}).to_string();
    assert_eq!(
        app.clone()
            .oneshot(request(
                "DELETE",
                &remove,
                admin,
                true,
                2,
                "tender-http-rem-key1",
                confirmation.clone()
            ))
            .await
            .unwrap()
            .status(),
        StatusCode::PRECONDITION_FAILED
    );
    assert_eq!(
        app.clone()
            .oneshot(request(
                "DELETE",
                &remove,
                admin,
                true,
                3,
                "tender-http-rem-key1",
                confirmation
            ))
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    assert!(service.clean_one_attachment().await.unwrap());
    service.delete(1, record.id, 4, record.id).await.unwrap();
    while service.clean_one_attachment().await.unwrap() {}
}
