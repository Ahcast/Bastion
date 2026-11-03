use base64::{Engine, engine::general_purpose::STANDARD};
use bastion_colaboracion::{CollaborationService, UploadInput, VariableKind};
use bastion_core::{CoreService, Database};
use bastion_licitaciones::{
    AttachmentCopy, AttachmentSection, AttachmentUpload, GlobalVariable, GlobalVariablesInput,
    InitialFormsInput, LotInput, LotItem, NameInput, TenderService,
};
use sqlx::mysql::MySqlPoolOptions;
use std::io::Read;

#[tokio::test]
async fn isolated_job_generates_and_downloads_a_multipage_tiff() {
    let Ok(url) = std::env::var("BASTION_TENDER_PDF_E2E_URL") else {
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
    // Keep a synthetic legacy form to verify existing rendering compatibility.
    sqlx::query("INSERT INTO licitaciones_formularios_instalados (form_key,enabled) VALUES ('integridad',TRUE)")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO persons (id,full_name) VALUES (1,'Synthetic admin')")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO users (id,person_id,role_id,email,status) VALUES (1,1,1,'admin@example.test','active')").execute(&pool).await.unwrap();
    let core = CoreService::new(db.clone(), "x".repeat(32), [1; 32]).unwrap();
    let service = TenderService::new(db.clone(), core.admin());
    let library = CollaborationService::new(db, core.admin());
    library.synchronize_release(true).await.unwrap();
    core.admin()
        .set_module_enabled(1, "colaboracion", true)
        .await
        .unwrap();
    service.synchronize_release(true).await.unwrap();
    core.admin()
        .set_module_enabled(1, "licitaciones", true)
        .await
        .unwrap();
    let exp = service.create(1, "pdf-e2e-expediente01").await.unwrap();
    let exp = service
        .rename(
            1,
            exp.id,
            exp.version,
            NameInput {
                name: "Expediente de prueba".into(),
            },
        )
        .await
        .unwrap();
    let variables = [
        ("lpn", VariableKind::Text, "LPN-PRUEBA"),
        ("inst", VariableKind::Text, "Entidad"),
        ("dia", VariableKind::Integer, "1"),
        ("mes", VariableKind::Text, "enero"),
        ("year", VariableKind::Integer, "2026"),
        ("lotes", VariableKind::Integer, "1"),
        ("stylo", VariableKind::Text, "std"),
        ("descripcion", VariableKind::Text, "Equipo"),
    ]
    .into_iter()
    .map(|(key, kind, value)| GlobalVariable {
        key: key.into(),
        label: key.into(),
        kind,
        value: Some(serde_json::json!(value)),
    })
    .collect();
    let exp = service
        .save_global_variables(
            1,
            exp.id,
            exp.version,
            GlobalVariablesInput { variables },
            false,
        )
        .await
        .unwrap();
    let exp = service
        .save_lot(
            1,
            exp.id,
            1,
            exp.version,
            LotInput {
                items: vec![LotItem {
                    descripcion: "Equipo".into(),
                    pais_origen: "".into(),
                    fecha_entrega: "30 dias".into(),
                    cantidad: "2.5".into(),
                    unidad: "unidad".into(),
                    transporte: "0.00".into(),
                    precio_unitario: "100.00".into(),
                }],
            },
        )
        .await
        .unwrap();
    let exp = service
        .save_initial_forms(
            1,
            exp.id,
            exp.version,
            InitialFormsInput {
                forms: vec!["integridad".into()],
            },
        )
        .await
        .unwrap();
    let source = library.upload(1, "direct-typst-source01", UploadInput { scope:"empresa".into(), id_persona:None, title:"Declaracion directa".into(), notes:String::new(), filename:"directo.typ".into(), content_base64:STANDARD.encode(b"#import \"config.typ\": datos_base\n= Typst legal directo\n#variables.lpn / Total #variables.total_paginas") }).await.unwrap();
    let exp = service
        .copy_attachment(
            1,
            exp.id,
            exp.version,
            "direct-typst-reference01",
            AttachmentCopy {
                section: AttachmentSection::Legal,
                source_id: source.id,
            },
        )
        .await
        .unwrap();
    let files = service.attachments(1, exp.id).await.unwrap();
    let direct = files
        .items
        .iter()
        .find(|f| f.source_id == Some(source.id))
        .unwrap();
    assert!(direct.is_reference && direct.available);
    assert_eq!(direct.title.as_deref(), Some("Declaracion directa"));
    let direct_id = direct.id.clone();
    let object_key: Option<String> =
        sqlx::query_scalar("SELECT object_key FROM licitaciones_archivos WHERE id=?")
            .bind(&direct_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(object_key.is_none());
    let tiff = "SUkqAAgAAAAKAAABBAABAAAAAgAAAAEBBAABAAAAAgAAAAIBAwADAAAAhgAAAAMBAwABAAAAAQAAAAYBAwABAAAAAgAAABEBBAABAAAAjAAAABUBAwABAAAAAwAAABYBBAABAAAAAgAAABcBBAABAAAADAAAABwBAwABAAAAAQAAAKgAAAAIAAgACAD/AAD/AAD/AAD/AAAAAAAAAAAAAElJKgAIAAAACgAAAQQAAQAAAAIAAAABAQQAAQAAAAMAAAACAQMAAwAAACYBAAADAQMAAQAAAAEAAAAGAQMAAQAAAAIAAAARAQQAAQAAACwBAAAVAQMAAQAAAAMAAAAWAQQAAQAAAAMAAAAXAQQAAQAAABIAAAAcAQMAAQAAAAEAAAAAAAAACAAIAAgAAAD/AAD/AAD/AAD/AAD/AAD/AAA=";
    let exp = service
        .upload_attachment(
            1,
            exp.id,
            exp.version,
            "pdf-e2e-attachment01",
            AttachmentUpload {
                section: AttachmentSection::Legal,
                filename: "anexo.tiff".into(),
                content_base64: tiff.into(),
            },
        )
        .await
        .unwrap();
    let order = service.pdf_order(1, exp.id).await.unwrap();
    let exp = service
        .save_pdf_order(
            1,
            exp.id,
            exp.version,
            bastion_licitaciones::PdfOrderInput {
                items: order.items.into_iter().map(|item| item.key).collect(),
                titles: Some(std::collections::BTreeMap::from([(
                    format!("archivo:{direct_id}"),
                    "Documento legal personalizado".into(),
                )])),
            },
        )
        .await
        .unwrap();
    assert!(
        service
            .pdf_order(1, exp.id)
            .await
            .unwrap()
            .items
            .iter()
            .any(|item| item.label == "Documento legal personalizado")
    );
    // Isolate staging failures from broker failures, using only synthetic data.
    let stage = std::env::temp_dir().join(format!(
        "pdf-e2e-stage-{}",
        bastion_core::DocumentService::object_suffix()
    ));
    std::fs::create_dir(&stage).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&stage, std::fs::Permissions::from_mode(0o700)).unwrap();
    }
    let template_dir = std::env::var("BASTION_TENDER_TEMPLATE_DIR").unwrap();
    let bundle = bastion_licitaciones::TenderTemplateBundle::from_private_directory(
        std::path::Path::new(&template_dir),
    )
    .unwrap();
    let prepared = service
        .stage_pdf_inputs(1, exp.id, exp.version, stage.clone(), &bundle)
        .await
        .unwrap();
    assert_eq!(prepared.conversions.len(), 1);
    std::fs::remove_dir_all(stage).unwrap();
    let job = service
        .create_pdf_job(1, exp.id, exp.version)
        .await
        .unwrap();
    assert!(service.process_one_pdf_job().await.unwrap());
    let job = service.pdf_job(1, exp.id, &job.id).await.unwrap();
    assert_eq!(job.state, "ready");
    let (mut file, size) = service.download_pdf_job(1, exp.id, &job.id).await.unwrap();
    let mut pdf = Vec::new();
    file.read_to_end(&mut pdf).unwrap();
    assert_eq!(size, pdf.len() as u64);
    assert!(pdf.starts_with(b"%PDF-"));
    let output = std::env::temp_dir().join(format!("pdf-e2e-{}.pdf", job.id));
    std::fs::write(&output, &pdf).unwrap();
    let info = std::process::Command::new("/usr/bin/pdfinfo")
        .arg(&output)
        .output()
        .unwrap();
    assert!(info.status.success());
    let text = String::from_utf8(info.stdout).unwrap();
    assert!(
        text.lines()
            .any(|line| line.split_whitespace().collect::<Vec<_>>() == vec!["Pages:", "8"])
    );
    let text = std::process::Command::new("/usr/bin/pdftotext")
        .arg(&output)
        .arg("-")
        .output()
        .unwrap();
    assert!(text.status.success());
    assert!(
        String::from_utf8(text.stdout)
            .unwrap()
            .contains("Documento legal personalizado")
    );
    std::fs::remove_file(&output).unwrap();
    // Preview ignores legal templates and does not replace the final job.
    let preview = service
        .preview_economic_offer(1, exp.id, exp.version)
        .await
        .unwrap();
    let bytes = STANDARD.decode(preview.content_base64).unwrap();
    assert!(bytes.starts_with(b"%PDF-"));
    std::fs::write(&output, bytes).unwrap();
    let info = std::process::Command::new("/usr/bin/pdfinfo")
        .arg(&output)
        .output()
        .unwrap();
    assert!(info.status.success());
    assert!(
        String::from_utf8(info.stdout)
            .unwrap()
            .lines()
            .any(|line| line.split_whitespace().collect::<Vec<_>>() == vec!["Pages:", "2"])
    );
    let text = std::process::Command::new("/usr/bin/pdftotext")
        .arg(&output)
        .arg("-")
        .output()
        .unwrap();
    assert!(text.status.success());
    let text = String::from_utf8(text.stdout).unwrap();
    assert!(text.contains("Paginas 2"));
    assert!(!text.contains("Integridad"));
    assert!(!text.contains("Typst legal directo"));
    assert_eq!(
        service.latest_pdf_job(1, exp.id).await.unwrap().unwrap().id,
        job.id
    );
    assert_eq!(
        service.pdf_job(1, exp.id, &job.id).await.unwrap().state,
        "ready"
    );
    assert!(matches!(
        service
            .preview_economic_offer(1, exp.id, exp.version + 1)
            .await,
        Err(bastion_core::CoreError::PreconditionFailed)
    ));
    std::fs::remove_file(output).unwrap();
    let exp = service
        .remove_attachment(1, exp.id, &direct_id, exp.version, &direct_id)
        .await
        .unwrap();
    assert!(library.file_snapshot(1, source.id, 100_000).await.is_ok());
    service
        .delete(1, exp.id, exp.version, exp.id)
        .await
        .unwrap();
    while service.clean_one_pdf_job().await.unwrap() {}
    while service.clean_one_attachment().await.unwrap() {}
}
