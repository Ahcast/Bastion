use base64::{Engine, engine::general_purpose::STANDARD};
use bastion_colaboracion::{
    CollaborationService, FormDefinition, FormSection, FormVariable, PublishFormInput, UploadInput,
    VariableKind,
};
use bastion_core::{CoreError, CoreService, Database, DocumentService};
use bastion_licitaciones::{
    AttachmentCopy, AttachmentSection, AttachmentUpload, CopyFormInput, GlobalVariable,
    GlobalVariablesInput, TenderService, TenderTemplateBundle,
};
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
    let staged = service
        .render_attachment(1, first.id, 2, &file.id)
        .await
        .unwrap();
    assert_eq!(staged.bytes, pdf);
    assert_eq!(staged.mime, "application/pdf");
    assert!(matches!(
        service.render_attachment(1, first.id, 1, &file.id).await,
        Err(CoreError::PreconditionFailed)
    ));
    assert!(matches!(
        service.render_attachment(1, second.id, 1, &file.id).await,
        Err(CoreError::NotFound)
    ));
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
    let template_exp = service.create(1, "tender-template-exp01").await.unwrap();
    let template_exp = service
        .copy_form(
            1,
            template_exp.id,
            template_exp.version,
            "tender-template-copy01",
            CopyFormInput {
                source_id: form.id,
                revision: form.revision,
            },
        )
        .await
        .unwrap();
    let form_copy_id = template_exp.forms[0].id;
    let captured = service
        .render_template(1, template_exp.id, template_exp.version, form_copy_id)
        .await
        .unwrap();
    assert_eq!(captured.source_id, template.id);
    assert_eq!(captured.source, "= Documento\n#let nombre = \"\"\n");
    assert!(matches!(
        service
            .render_template(1, template_exp.id, template_exp.version - 1, form_copy_id)
            .await,
        Err(CoreError::PreconditionFailed)
    ));
    assert!(matches!(
        service.render_template(1, first.id, 2, form_copy_id).await,
        Err(CoreError::NotFound)
    ));

    let mut variables = Vec::new();
    for (key, kind, value) in [
        ("nombre", VariableKind::Text, "Proveedor"),
        ("lpn", VariableKind::Text, "LPN-1"),
        ("inst", VariableKind::Text, "Entidad"),
        ("dia", VariableKind::Integer, "1"),
        ("mes", VariableKind::Text, "enero"),
        ("year", VariableKind::Integer, "2026"),
        ("descripcion", VariableKind::Text, "Equipo"),
        ("lotes", VariableKind::Integer, "1"),
        ("stylo", VariableKind::Text, "std"),
    ] {
        variables.push(GlobalVariable {
            key: key.into(),
            label: key.into(),
            kind,
            value: Some(serde_json::json!(value)),
        });
    }
    let template_exp = service
        .save_global_variables(
            1,
            template_exp.id,
            template_exp.version,
            GlobalVariablesInput { variables },
            false,
        )
        .await
        .unwrap();
    let root =
        std::env::temp_dir().join(format!("tender-stage-{}", DocumentService::object_suffix()));
    std::fs::create_dir(&root).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
    }
    let bundle = TenderTemplateBundle {
        bid_offer: "= Oferta BID".into(),
        std_offer: "= Oferta STD".into(),
        bid_lot: "#import \"items1_bid.typ\": items".into(),
        std_lot: "#import \"items1_std.typ\": items".into(),
        functions: "#let f = 1".into(),
    };
    let request = service
        .stage_pdf_inputs(
            1,
            template_exp.id,
            template_exp.version,
            root.clone(),
            &bundle,
        )
        .await
        .unwrap();
    assert_eq!(request.pieces, 4);
    assert_eq!(request.sources, vec![3, 4]);
    assert_eq!(request.global_sources, vec![4]);
    assert_eq!(
        std::fs::read_to_string(root.join("source-000003.typ")).unwrap(),
        "= Oferta STD"
    );
    assert_eq!(
        std::fs::read_to_string(root.join("source-000004.typ")).unwrap(),
        "= Documento\n#let nombre = \"\"\n"
    );
    std::fs::remove_dir_all(root).unwrap();
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
    let staged = service
        .render_attachment(1, first.id, 3, &copied_file.id)
        .await
        .unwrap();
    assert_eq!(staged.bytes, pdf);
    assert!(matches!(
        service
            .render_attachment(1, first.id, 2, &copied_file.id)
            .await,
        Err(CoreError::PreconditionFailed)
    ));
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
