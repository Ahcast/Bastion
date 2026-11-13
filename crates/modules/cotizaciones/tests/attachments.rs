use base64::{Engine, engine::general_purpose::STANDARD};
use bastion_core::{CoreError, CoreService, Database};
use bastion_cotizaciones::{
    MAX_ATTACHMENT_BYTES, QuoteAttachmentInput, QuoteEmailInput, QuoteInput, QuoteMailService,
    QuoteService,
};
use sqlx::mysql::MySqlPoolOptions;

fn quote() -> QuoteInput {
    serde_json::from_value(serde_json::json!({"client_name":"Cliente sintético","order_number":"","quote_date":"2026-10-23","valid_days":30,"notes":"PRIVATE-NOTE","terms":"","discount_type":"amount","discount_value":"0","items":[{"description":"Producto","quantity":"1","unit_price":"100","discount_type":"amount","discount_value":"0","tax_type":"isv15"}]})).unwrap()
}
fn file(name: &str, bytes: &[u8]) -> QuoteAttachmentInput {
    QuoteAttachmentInput {
        filename: name.into(),
        title: String::new(),
        content_base64: STANDARD.encode(bytes),
    }
}
#[tokio::test]
async fn mysql57_private_attachments_versions_receipts_permissions_and_cleanup() {
    let Ok(url) = std::env::var("BASTION_COTIZACIONES_ATTACHMENTS_TEST_URL") else {
        eprintln!("Requires isolated attachment database and test-only storage CLI.");
        return;
    };
    assert!(
        std::env::var("BASTION_AWS_CLI")
            .unwrap()
            .ends_with("/tests/fixtures/private_storage.py")
    );
    assert_eq!(
        std::env::var("BASTION_DOCUMENTS_BUCKET").unwrap(),
        "bastion-synthetic-attachment-test"
    );
    let pool = MySqlPoolOptions::new()
        .max_connections(5)
        .connect(&url)
        .await
        .unwrap();
    let database = Database::from_test_pool(pool.clone());
    database.migrate().await.unwrap();
    QuoteService::migrate(&database).await.unwrap();
    sqlx::query(
        "INSERT INTO persons (id,full_name) VALUES (1,'Attachment admin'),(2,'Attachment viewer')",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO roles (id,name) VALUES (2,'Attachment viewer')")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO users (id,person_id,role_id,email,status) VALUES (1,1,1,'admin@example.test','active'),(2,2,2,'viewer@example.test','active')").execute(&pool).await.unwrap();
    let core = CoreService::new(database.clone(), "x".repeat(32), [1; 32]).unwrap();
    let service = QuoteService::new(database.clone(), core.admin());
    service.synchronize_release(true).await.unwrap();
    core.admin()
        .set_module_enabled(1, "cotizaciones", true)
        .await
        .unwrap();
    sqlx::query("INSERT INTO role_permissions (role_id,module_id,scope_hash,scope_key,capability) VALUES (2,'cotizaciones',UNHEX(SHA2('documentos',256)),'documentos','ver')").execute(&pool).await.unwrap();
    let q = service
        .create(1, "attachment-quote-create-01", quote())
        .await
        .unwrap();
    let ready = service.document_readiness(1, &core).await.unwrap();
    assert!(ready.attachments);
    assert!(!ready.pdf);
    assert!(matches!(
        service
            .upload_attachment(2, q.id, 1, "attachment-forbidden-01", file("x", b"data"))
            .await,
        Err(CoreError::Forbidden)
    ));
    let one = service
        .upload_attachment(
            1,
            q.id,
            1,
            "attachment-upload-key-01",
            file("private.pdf", b"%PDF-synthetic"),
        )
        .await
        .unwrap();
    assert_eq!(one.quote_version, 2);
    assert_eq!(one.attachment.mime, "application/pdf");
    let two = service
        .upload_attachment(
            1,
            q.id,
            2,
            "attachment-upload-key-02",
            file("document.html", b"<script>not executed</script>"),
        )
        .await
        .unwrap();
    assert_eq!(two.quote_version, 3);
    assert_eq!(two.attachment.mime, "application/octet-stream");
    let repeat = service
        .upload_attachment(
            1,
            q.id,
            1,
            "attachment-upload-key-01",
            file("private.pdf", b"%PDF-synthetic"),
        )
        .await
        .unwrap();
    assert_eq!(repeat.attachment.id, one.attachment.id);
    assert!(matches!(
        service
            .upload_attachment(
                1,
                q.id,
                3,
                "attachment-upload-key-01",
                file("private.pdf", b"different")
            )
            .await,
        Err(CoreError::Conflict)
    ));
    assert!(matches!(
        service
            .upload_attachment(1, q.id, 1, "attachment-stale-key-01", file("x", b"data"))
            .await,
        Err(CoreError::PreconditionFailed)
    ));
    let downloaded = service
        .attachment_content(2, q.id, &one.attachment.id)
        .await
        .unwrap();
    assert_eq!(
        STANDARD.decode(downloaded.content_base64).unwrap(),
        b"%PDF-synthetic"
    );
    let other = service
        .create(1, "attachment-quote-create-02", quote())
        .await
        .unwrap();
    assert!(matches!(
        service
            .attachment_content(1, other.id, &one.attachment.id)
            .await,
        Err(CoreError::NotFound)
    ));
    assert!(matches!(
        service
            .remove_attachment(2, q.id, &one.attachment.id, 3)
            .await,
        Err(CoreError::Forbidden)
    ));
    assert!(matches!(
        service
            .upload_attachment(
                1,
                q.id,
                3,
                "attachment-path-key-01",
                file("../escape", b"x")
            )
            .await,
        Err(CoreError::Invalid)
    ));
    for (key, value) in [
        (
            "company.profile",
            serde_json::json!({"name":"Empresa sintética"}),
        ),
        ("smtp.host", serde_json::json!("mail.example.test")),
        ("smtp.port", serde_json::json!(587)),
        ("smtp.tls_mode", serde_json::json!("starttls_required")),
    ] {
        core.admin().set_core_setting(1, key, value).await.unwrap();
    }
    // This deliberately unusable credential proves no SMTP delivery is attempted below.
    sqlx::query("INSERT INTO user_smtp_credentials (user_id,smtp_email,password_ciphertext) VALUES (1,'sender@example.test',X'01')").execute(&pool).await.unwrap();
    let mail = QuoteMailService::new(core.clone());
    let job = mail
        .queue(
            1,
            q.id,
            3,
            "attachment-mail-queue-01",
            QuoteEmailInput {
                to: "recipient@example.test".into(),
                cc: String::new(),
                bcc: String::new(),
                subject: String::new(),
                message: String::new(),
                scheduled_at: None,
                collaboration_file_ids: Vec::new(),
            },
        )
        .await
        .unwrap();
    let encrypted: Vec<u8> =
        sqlx::query_scalar("SELECT payload_ciphertext FROM cotizaciones_email_jobs WHERE id=?")
            .bind(job.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    let envelope = core.unseal_domain_json(&encrypted).unwrap();
    assert_eq!(envelope["attachments"].as_array().unwrap().len(), 2);
    assert!(!String::from_utf8_lossy(&encrypted).contains("private.pdf"));
    // Library selections stay in the encrypted mail snapshot, not in the quote.
    bastion_colaboracion::CollaborationService::migrate(&database)
        .await
        .unwrap();
    let library = bastion_colaboracion::CollaborationService::new(database.clone(), core.admin());
    library.synchronize_release(true).await.unwrap();
    core.admin()
        .set_module_enabled(1, "colaboracion", true)
        .await
        .unwrap();
    let mut source_ids = Vec::new();
    for (index, mime) in [
        (1, "application/pdf"),
        (2, "application/pdf"),
        (3, "text/plain"),
    ] {
        let source = sqlx::query("INSERT INTO colaboracion_files (scope,id_persona,folder_key,title,notes,filename,size_bytes,mime,object_key,content_hash,request_hash,actor_id,idempotency_key,state) VALUES ('empresa',NULL,'empresa',?,'',?,12,?,?,UNHEX(SHA2('synthetic',256)),UNHEX(SHA2('request',256)),1,?,'ready')")
            .bind(format!("Library {index}")).bind(format!("library-{index}.pdf")).bind(mime).bind(format!("colaboracion/synthetic/{index}" )).bind(format!("library-synthetic-{index}")).execute(&pool).await.unwrap();
        source_ids.push(source.last_insert_id());
    }
    let with_library = |ids: Vec<u64>| QuoteEmailInput {
        to: "recipient@example.test".into(),
        cc: String::new(),
        bcc: String::new(),
        subject: String::new(),
        message: String::new(),
        scheduled_at: Some("2099-01-01T12:00:00Z".into()),
        collaboration_file_ids: ids,
    };
    let selected = mail
        .queue(
            1,
            q.id,
            3,
            "attachment-library-mail-01",
            with_library(source_ids[..2].to_vec()),
        )
        .await
        .unwrap();
    let payload: Vec<u8> =
        sqlx::query_scalar("SELECT payload_ciphertext FROM cotizaciones_email_jobs WHERE id=?")
            .bind(selected.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    let snapshot = core.unseal_domain_json(&payload).unwrap();
    assert_eq!(snapshot["collaboration_files"].as_array().unwrap().len(), 2);
    assert_eq!(snapshot["attachments"].as_array().unwrap().len(), 2);
    assert!(!String::from_utf8_lossy(&payload).contains("library-1.pdf"));
    assert_eq!(service.detail(1, q.id).await.unwrap().version, 3);
    assert!(matches!(
        mail.queue(
            1,
            q.id,
            3,
            "attachment-library-nonpdf",
            with_library(vec![source_ids[2]])
        )
        .await,
        Err(CoreError::Invalid)
    ));
    sqlx::query("INSERT INTO role_permissions (role_id,module_id,scope_hash,scope_key,capability) VALUES (2,'cotizaciones',UNHEX(SHA2('documentos',256)),'documentos','editar')").execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO user_smtp_credentials (user_id,smtp_email,password_ciphertext) VALUES (2,'viewer@example.test',X'01')").execute(&pool).await.unwrap();
    assert!(matches!(
        mail.queue(
            2,
            q.id,
            3,
            "attachment-library-denied",
            with_library(vec![source_ids[0]])
        )
        .await,
        Err(CoreError::NotFound)
    ));
    sqlx::query("DELETE FROM role_permissions WHERE role_id=2 AND module_id='cotizaciones' AND capability='editar'").execute(&pool).await.unwrap();
    library
        .delete(1, source_ids[0], source_ids[0])
        .await
        .unwrap();
    sqlx::query("UPDATE cotizaciones_email_jobs SET scheduled_at='2000-01-01 00:00:00' WHERE id=?")
        .bind(selected.id)
        .execute(&pool)
        .await
        .unwrap();
    assert!(mail.deliver_one().await.unwrap());
    let revoked = mail.job(1, q.id, selected.id).await.unwrap();
    assert_eq!(revoked.delivery_status, "failed");
    assert_eq!(
        revoked.last_error_code.as_deref(),
        Some("authorization_changed")
    );
    let large = vec![b'x'; MAX_ATTACHMENT_BYTES];
    let big = service
        .upload_attachment(
            1,
            q.id,
            3,
            "attachment-maximum-key-01",
            file("maximum.bin", &large),
        )
        .await
        .unwrap();
    assert_eq!(big.attachment.byte_size, MAX_ATTACHMENT_BYTES as u32);
    assert_eq!(
        STANDARD
            .decode(
                service
                    .attachment_content(1, q.id, &big.attachment.id)
                    .await
                    .unwrap()
                    .content_base64
            )
            .unwrap(),
        large
    );
    assert!(matches!(
        service
            .upload_attachment(
                1,
                q.id,
                4,
                "attachment-oversize-key-01",
                file("too-big", &vec![0; MAX_ATTACHMENT_BYTES + 1])
            )
            .await,
        Err(CoreError::Invalid)
    ));
    assert!(mail.deliver_one().await.unwrap());
    let failed = mail.job(1, q.id, job.id).await.unwrap();
    assert_eq!(failed.delivery_status, "failed");
    assert_eq!(failed.last_error_code.as_deref(), Some("quote_changed"));
    let (a, b) = tokio::join!(
        service.upload_attachment(
            1,
            q.id,
            4,
            "attachment-concurrent-01",
            file("once.txt", b"one")
        ),
        service.upload_attachment(
            1,
            q.id,
            4,
            "attachment-concurrent-01",
            file("once.txt", b"one")
        )
    );
    let a = a.unwrap();
    assert_eq!(a.attachment.id, b.unwrap().attachment.id);
    let page = service.attachments(1, q.id, 1).await.unwrap();
    assert_eq!(page.items.len(), 4);
    assert_eq!(page.quote_version, 5);
    let metadata = serde_json::to_value(&page).unwrap();
    assert!(!metadata.to_string().contains("object_key"));
    let clone = service
        .clone_quote(1, q.id, 5, "attachment-clone-key-01")
        .await
        .unwrap();
    assert!(
        service
            .attachments(1, clone.id, 1)
            .await
            .unwrap()
            .items
            .is_empty()
    );
    assert_eq!(
        service
            .remove_attachment(1, q.id, &one.attachment.id, 5)
            .await
            .unwrap(),
        6
    );
    assert!(matches!(
        service
            .attachment_content(1, q.id, &one.attachment.id)
            .await,
        Err(CoreError::NotFound)
    ));
    assert!(service.clean_one_document().await.unwrap());
    let audit: Vec<serde_json::Value> = sqlx::query_scalar(
        "SELECT details_json FROM audit_events WHERE action LIKE 'quote.attachment.%'",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert!(
        !serde_json::to_string(&audit)
            .unwrap()
            .contains("private.pdf")
    );
    service.delete(1, q.id, 6).await.unwrap();
    sqlx::query("UPDATE cotizaciones_document_deletions SET available_at=UTC_TIMESTAMP(6)")
        .execute(&pool)
        .await
        .unwrap();
    while service.clean_one_document().await.unwrap() {}
    let remains: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM cotizaciones_documents WHERE quote_id=?")
            .bind(q.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(remains, 0);
}
