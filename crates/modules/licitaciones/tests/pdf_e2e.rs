use bastion_colaboracion::{CollaborationService, VariableKind};
use bastion_core::{CoreService, Database};
use bastion_licitaciones::{
    AttachmentSection, AttachmentUpload, GlobalVariable, GlobalVariablesInput, InitialFormsInput,
    LotInput, LotItem, NameInput, TenderService,
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
    sqlx::query("INSERT INTO persons (id,full_name) VALUES (1,'Synthetic admin')")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO users (id,person_id,role_id,email,status) VALUES (1,1,1,'admin@example.test','active')").execute(&pool).await.unwrap();
    let core = CoreService::new(db.clone(), "x".repeat(32), [1; 32]).unwrap();
    let service = TenderService::new(db, core.admin());
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
            .any(|line| line.split_whitespace().collect::<Vec<_>>() == vec!["Pages:", "7"])
    );
    std::fs::remove_file(output).unwrap();
    service
        .delete(1, exp.id, exp.version, exp.id)
        .await
        .unwrap();
    while service.clean_one_pdf_job().await.unwrap() {}
    while service.clean_one_attachment().await.unwrap() {}
}
