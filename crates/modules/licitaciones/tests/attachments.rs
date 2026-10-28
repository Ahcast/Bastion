use base64::{Engine, engine::general_purpose::STANDARD};
use bastion_colaboracion::{
    CollaborationService, FormDefinition, FormSection, FormVariable, PublishFormInput, UploadInput,
    VariableKind,
};
use bastion_core::{CoreError, CoreService, Database, DocumentService};
use bastion_licitaciones::{AttachmentCopy, AttachmentSection, AttachmentUpload, TenderService};
use sqlx::mysql::MySqlPoolOptions;

fn upload(section: AttachmentSection, name: &str, bytes: &[u8]) -> AttachmentUpload {
    AttachmentUpload {
        section,
        filename: name.into(),
        content_base64: STANDARD.encode(bytes),
    }
}
#[tokio::test]
async fn mysql57_tender_files_private_copies_integrity_recovery_and_budgets() {
    let Ok(url) = std::env::var("BASTION_TENDER_FILES_TEST_URL") else {
        eprintln!("Requires isolated tender files DB and synthetic storage; CI supplies them.");
        return;
    };
    let pool = MySqlPoolOptions::new()
        .max_connections(6)
        .connect(&url)
        .await
        .unwrap();
    let db = Database::from_test_pool(pool.clone());
    db.migrate().await.unwrap();
    CollaborationService::migrate(&db).await.unwrap();
    TenderService::migrate(&db).await.unwrap();
    sqlx::query(
        "INSERT INTO persons (id,full_name) VALUES (1,'Synthetic admin'),(2,'Synthetic reader')",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO roles (id,name) VALUES (2,'Synthetic reader')")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO users (id,person_id,role_id,email,status) VALUES (1,1,1,'admin@example.test','active'),(2,2,2,'reader@example.test','active')").execute(&pool).await.unwrap();
    let core = CoreService::new(db.clone(), "x".repeat(32), [1; 32]).unwrap();
    let service = TenderService::new(db.clone(), core.admin());
    let library = CollaborationService::new(db, core.admin());
    service.synchronize_release(true).await.unwrap();
    library.synchronize_release(true).await.unwrap();
    core.admin()
        .set_module_enabled(1, "licitaciones", true)
        .await
        .unwrap();
    core.admin()
        .set_module_enabled(1, "colaboracion", true)
        .await
        .unwrap();
    let first = service.create(1, "tender-files-create01").await.unwrap();
    let second = service.create(1, "tender-files-create02").await.unwrap();
    let pdf = b"%PDF-1.4\n1 0 obj\n<< /Type /Catalog >>\nendobj\n%%EOF\n";
    assert!(matches!(
        service
            .upload_attachment(
                2,
                first.id,
                1,
                "tender-file-denied01",
                upload(AttachmentSection::Legal, "a.pdf", pdf)
            )
            .await,
        Err(CoreError::Forbidden)
    ));
    assert!(matches!(
        service
            .upload_attachment(
                1,
                first.id,
                1,
                "tender-file-wrongtype",
                upload(AttachmentSection::Legal, "a.jpg", pdf)
            )
            .await,
        Err(CoreError::Invalid)
    ));
    let a = service
        .upload_attachment(
            1,
            first.id,
            1,
            "tender-file-upload01",
            upload(AttachmentSection::Legal, "legal.pdf", pdf),
        )
        .await
        .unwrap();
    assert_eq!(a.version, 2);
    let same = service
        .upload_attachment(
            1,
            first.id,
            1,
            "tender-file-upload01",
            upload(AttachmentSection::Legal, "legal.pdf", pdf),
        )
        .await
        .unwrap();
    assert_eq!(same.version, 2);
    assert!(matches!(
        service
            .upload_attachment(
                1,
                first.id,
                2,
                "tender-file-upload01",
                upload(AttachmentSection::Tecnica, "legal.pdf", pdf)
            )
            .await,
        Err(CoreError::Conflict)
    ));
    let files = service.attachments(1, first.id).await.unwrap();
    assert_eq!(files.items.len(), 1);
    assert_eq!(files.retained_bytes, pdf.len() as u64);
    let file = &files.items[0];
    assert!(matches!(
        service.read_attachment(1, second.id, &file.id, true).await,
        Err(CoreError::NotFound)
    ));
    assert_eq!(
        service
            .read_attachment(1, first.id, &file.id, true)
            .await
            .unwrap()
            .content_base64,
        STANDARD.encode(pdf)
    );
    let original = library
        .upload(
            1,
            "tender-library-upload01",
            UploadInput {
                scope: "empresa".into(),
                id_persona: None,
                title: "Synthetic source".into(),
                notes: String::new(),
                filename: "source.pdf".into(),
                content_base64: STANDARD.encode(pdf),
            },
        )
        .await
        .unwrap();
    let original_id = original.id;
    let template = library
        .upload(
            1,
            "tender-template-upload1",
            UploadInput {
                scope: "empresa".into(),
                id_persona: None,
                title: "Plantilla".into(),
                notes: String::new(),
                filename: "formulario.typ".into(),
                content_base64: STANDARD.encode(b"= Documento\n#let nombre = \"\"\n"),
            },
        )
        .await
        .unwrap();
    let form = library
        .publish_form(
            1,
            "tender-template-form01",
            PublishFormInput {
                form_id: None,
                expected_revision: None,
                definition: FormDefinition {
                    title: "Formulario".into(),
                    section: FormSection::Legal,
                    template_source_id: Some(template.id),
                    variables: vec![FormVariable {
                        key: "nombre".into(),
                        label: "Nombre".into(),
                        kind: VariableKind::Text,
                        required: true,
                    }],
                },
            },
        )
        .await
        .unwrap();
    assert_eq!(form.definition.template_source_id, Some(template.id));
    assert_eq!(form.definition.variables[0].key, "nombre");
    let copied = service
        .copy_attachment(
            1,
            first.id,
            2,
            "tender-library-copy01",
            AttachmentCopy {
                section: AttachmentSection::Tecnica,
                source_id: original_id,
            },
        )
        .await
        .unwrap();
    assert_eq!(copied.version, 3);
    let files = service.attachments(1, first.id).await.unwrap();
    let copied_file = files
        .items
        .iter()
        .find(|f| f.source_id == Some(original_id))
        .unwrap();
    assert_eq!(
        copied_file.source_sha256.as_deref(),
        Some(copied_file.sha256.as_str())
    );
    assert!(copied_file.is_reference && copied_file.available);
    let key: Option<String> =
        sqlx::query_scalar("SELECT object_key FROM licitaciones_archivos WHERE id=?")
            .bind(&copied_file.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(key.is_none());
    assert_eq!(
        service
            .read_attachment(1, first.id, &copied_file.id, true)
            .await
            .unwrap()
            .content_base64,
        STANDARD.encode(pdf)
    );
    library.delete(1, original_id, original_id).await.unwrap();
    let missing = service.attachments(1, first.id).await.unwrap();
    let missing = missing
        .items
        .iter()
        .find(|item| item.id == copied_file.id)
        .unwrap();
    assert!(!missing.available);
    assert_eq!(missing.filename, "Archivo de Colaboración no disponible");
    core.admin()
        .set_module_enabled(1, "colaboracion", false)
        .await
        .unwrap();
    assert!(copied_file.is_reference);
    assert!(matches!(
        service
            .read_attachment(1, first.id, &copied_file.id, true)
            .await,
        Err(CoreError::NotFound)
    ));
    assert!(
        service
            .copy_attachment(
                1,
                first.id,
                2,
                "tender-library-copy01",
                AttachmentCopy {
                    section: AttachmentSection::Tecnica,
                    source_id: original_id
                }
            )
            .await
            .is_ok()
    );
    assert!(matches!(
        service
            .copy_attachment(
                1,
                first.id,
                3,
                "tender-library-copy02",
                AttachmentCopy {
                    section: AttachmentSection::Legal,
                    source_id: original_id
                }
            )
            .await,
        Err(CoreError::NotFound)
    ));
    let pending = "licitaciones/synthetic-reserved-budget";
    sqlx::query("INSERT INTO licitaciones_archivo_limpieza(object_key,expediente_id,byte_size,state,reason,available_at) VALUES (?,?,999999999,'reserved','abandoned_upload',UTC_TIMESTAMP(6)+INTERVAL 1 DAY)")
        .bind(pending).bind(first.id).execute(&pool).await.unwrap();
    assert!(matches!(
        service
            .upload_attachment(
                1,
                first.id,
                3,
                "tender-budget-overflow",
                upload(AttachmentSection::Legal, "other.pdf", pdf)
            )
            .await,
        Err(CoreError::Invalid)
    ));
    sqlx::query("DELETE FROM licitaciones_archivo_limpieza WHERE object_key=?")
        .bind(pending)
        .execute(&pool)
        .await
        .unwrap();
    let removed = service
        .remove_attachment(1, first.id, &file.id, 3, &file.id)
        .await
        .unwrap();
    assert_eq!(removed.version, 4);
    assert!(matches!(
        service.read_attachment(1, first.id, &file.id, false).await,
        Err(CoreError::NotFound)
    ));
    core.admin()
        .set_module_enabled(1, "licitaciones", false)
        .await
        .unwrap();
    assert!(service.clean_one_attachment().await.unwrap());
    core.admin()
        .set_module_enabled(1, "licitaciones", true)
        .await
        .unwrap();
    let orphan = "licitaciones/synthetic-orphan/object";
    let storage = DocumentService::from_environment();
    storage
        .put_with_limit(orphan.into(), pdf.to_vec(), 20_000_000)
        .await
        .unwrap();
    storage
        .put_with_limit(format!("{orphan}-neighbor"), pdf.to_vec(), 20_000_000)
        .await
        .unwrap();
    sqlx::query("INSERT INTO licitaciones_archivo_limpieza(object_key,expediente_id,byte_size,state,reason,available_at) VALUES (?,?,?,'reserved','abandoned_upload',UTC_TIMESTAMP(6)-INTERVAL 1 MINUTE)").bind(orphan).bind(first.id).bind(pdf.len() as u64).execute(&pool).await.unwrap();
    assert!(service.clean_one_attachment().await.unwrap());
    assert!(
        storage
            .get_with_limit(orphan.into(), 20_000_000)
            .await
            .is_err()
    );
    assert!(
        storage
            .get_with_limit(format!("{orphan}-neighbor"), 20_000_000)
            .await
            .is_ok()
    );
    storage
        .remove_all_versions(format!("{orphan}-neighbor"))
        .await
        .unwrap();
    let mut large = vec![b' '; 100_000_000];
    large[..pdf.len()].copy_from_slice(pdf);
    assert!(
        service
            .upload_attachment(
                1,
                second.id,
                1,
                "tender-exact-20mb001",
                upload(AttachmentSection::Legal, "large.pdf", &large)
            )
            .await
            .is_ok()
    );
    large.push(b' ');
    assert!(matches!(
        service
            .upload_attachment(
                1,
                second.id,
                2,
                "tender-over-20mb0001",
                upload(AttachmentSection::Legal, "large.pdf", &large)
            )
            .await,
        Err(CoreError::Invalid)
    ));
    service.delete(1, first.id, 4, first.id).await.unwrap();
    service.delete(1, second.id, 2, second.id).await.unwrap();
    while service.clean_one_attachment().await.unwrap() {}
    let pending: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM licitaciones_archivo_limpieza")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(pending, 0);
}
