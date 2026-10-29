use bastion_colaboracion::{
    CollaborationService, FormDefinition, FormSection, FormVariable, PublishFormInput, VariableKind,
};
use bastion_core::{CoreError, CoreService, Database};
use bastion_licitaciones::{
    CopyFormInput, GlobalVariable, GlobalVariablesInput, LotInput, LotItem, NameInput, PageQuery,
    PdfOrderInput, TenderService, ValuesInput,
};
use sqlx::mysql::MySqlPoolOptions;

fn definition(title: &str) -> FormDefinition {
    FormDefinition {
        template_source_id: None,
        title: title.into(),
        section: FormSection::Legal,
        variables: vec![FormVariable {
            key: "dato".into(),
            label: "Dato sintético".into(),
            kind: VariableKind::Text,
            required: true,
        }],
    }
}
#[tokio::test]
async fn mysql57_tenders_snapshots_permissions_races_receipts_and_deletion() {
    let Ok(url) = std::env::var("BASTION_LICITACIONES_TEST_URL") else {
        eprintln!("Requires isolated BASTION_LICITACIONES_TEST_URL; CI supplies it.");
        return;
    };
    let pool = MySqlPoolOptions::new()
        .max_connections(5)
        .connect(&url)
        .await
        .unwrap();
    let db = Database::from_test_pool(pool.clone());
    db.migrate().await.unwrap();
    CollaborationService::migrate(&db).await.unwrap();
    TenderService::migrate(&db).await.unwrap();
    TenderService::migrate(&db).await.unwrap();
    sqlx::query(
        "INSERT INTO persons (id,full_name) VALUES (1,'Admin sintético'),(2,'Rol sintético')",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO roles (id,name) VALUES (2,'Rol sintético')")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO users (id,person_id,role_id,email,status) VALUES (1,1,1,'admin@example.test','active'),(2,2,2,'user@example.test','active')").execute(&pool).await.unwrap();
    let core = CoreService::new(db.clone(), "x".repeat(32), [1; 32]).unwrap();
    let tenders = TenderService::new(db.clone(), core.admin());
    let library = CollaborationService::new(db, core.admin());
    tenders.synchronize_release(true).await.unwrap();
    library.synchronize_release(true).await.unwrap();
    assert!(matches!(
        tenders.create(1, "empty-create-key-01").await,
        Err(CoreError::NotFound)
    ));
    core.admin()
        .set_module_enabled(1, "licitaciones", true)
        .await
        .unwrap();
    core.admin()
        .set_module_enabled(1, "colaboracion", true)
        .await
        .unwrap();
    assert!(
        tenders
            .list(1, &PageQuery::default())
            .await
            .unwrap()
            .items
            .is_empty()
    );
    assert!(library.list_forms(1, 1, 25).await.unwrap().items.is_empty());
    assert!(matches!(
        tenders.create(2, "empty-create-key-02").await,
        Err(CoreError::Forbidden)
    ));
    // A custom role with every generic capability still cannot publish schemas.
    for module in ["licitaciones", "colaboracion"] {
        for capability in ["ver", "editar", "eliminar"] {
            sqlx::query("INSERT INTO role_permissions (role_id,module_id,scope_hash,scope_key,capability) VALUES (2,?,UNHEX(SHA2('*',256)),'*',?)").bind(module).bind(capability).execute(&pool).await.unwrap();
        }
    }
    let input = PublishFormInput {
        form_id: None,
        expected_revision: None,
        definition: definition("Versión uno"),
    };
    assert!(matches!(
        library
            .publish_form(2, "publish-not-admin-01", input.clone())
            .await,
        Err(CoreError::Forbidden)
    ));
    let (published_a, published_b) = tokio::join!(
        library.publish_form(1, "publish-same-key-01", input.clone()),
        library.publish_form(1, "publish-same-key-01", input)
    );
    let published = published_a.unwrap();
    assert_eq!(published_b.unwrap().id, published.id);
    let (first, retry) = tokio::join!(
        tenders.create(2, "create-same-key-01"),
        tenders.create(2, "create-same-key-01")
    );
    let first = first.unwrap();
    assert_eq!(retry.unwrap().id, first.id);
    assert!(first.forms.is_empty());
    let second = tenders.create(1, "create-other-key-01").await.unwrap();
    let copy_input = CopyFormInput {
        source_id: published.id,
        revision: 1,
    };
    let (a, b) = tokio::join!(
        tenders.copy_form(2, first.id, 1, "copy-form-same-key1", copy_input.clone()),
        tenders.copy_form(2, first.id, 1, "copy-form-same-key1", copy_input.clone())
    );
    let copied = a.unwrap();
    assert_eq!(b.unwrap().version, 2);
    assert_eq!(copied.forms.len(), 1);
    assert_eq!(copied.forms[0].missing_required, vec!["dato"]);
    assert!(matches!(
        tenders
            .copy_form(2, second.id, 1, "copy-form-same-key1", copy_input.clone())
            .await,
        Err(CoreError::Conflict)
    ));
    assert!(matches!(
        tenders
            .copy_form(2, first.id, 1, "copy-stale-version1", copy_input.clone())
            .await,
        Err(CoreError::PreconditionFailed)
    ));
    library
        .publish_form(
            1,
            "publish-revision-02",
            PublishFormInput {
                form_id: Some(published.id),
                expected_revision: Some(1),
                definition: definition("Versión dos"),
            },
        )
        .await
        .unwrap();
    let unchanged = tenders.detail(1, first.id).await.unwrap();
    assert_eq!(unchanged.forms[0].source.definition.title, "Versión uno");
    let copy_id = unchanged.forms[0].id;
    let values = || ValuesInput {
        values: std::collections::BTreeMap::from([("dato".into(), serde_json::json!("Privado"))]),
    };
    assert!(matches!(
        tenders
            .save_values(2, second.id, copy_id, 1, values())
            .await,
        Err(CoreError::NotFound)
    ));
    let (one, two) = tokio::join!(
        tenders.save_values(2, first.id, copy_id, 2, values()),
        tenders.save_values(2, first.id, copy_id, 2, values())
    );
    assert_eq!(usize::from(one.is_ok()) + usize::from(two.is_ok()), 1);
    assert!(matches!(
        one.as_ref().err().or(two.as_ref().err()),
        Some(CoreError::PreconditionFailed)
    ));
    assert!(tenders.detail(1, second.id).await.unwrap().forms.is_empty());
    core.admin()
        .set_module_enabled(1, "colaboracion", false)
        .await
        .unwrap();
    let stored = tenders.detail(2, first.id).await.unwrap();
    assert_eq!(stored.forms[0].values["dato"], "Privado");
    assert!(matches!(
        tenders
            .copy_form(
                2,
                first.id,
                stored.version,
                "copy-disabled-lib01",
                copy_input
            )
            .await,
        Err(CoreError::NotFound)
    ));
    assert!(
        tenders
            .save_values(2, first.id, copy_id, stored.version, values())
            .await
            .is_ok()
    );
    let current = tenders.detail(2, first.id).await.unwrap();
    assert!(matches!(
        tenders
            .delete(2, first.id, current.version, second.id)
            .await,
        Err(CoreError::Invalid)
    ));
    // Simulate a future Sent state: current APIs cannot set state or delete it.
    sqlx::query("UPDATE licitaciones_expedientes SET state='enviada' WHERE id=?")
        .bind(first.id)
        .execute(&pool)
        .await
        .unwrap();
    assert!(matches!(
        tenders.delete(2, first.id, current.version, first.id).await,
        Err(CoreError::Conflict)
    ));
    assert!(matches!(
        tenders
            .save_values(2, first.id, copy_id, current.version, values())
            .await,
        Err(CoreError::Conflict)
    ));
    tenders.delete(1, second.id, 1, second.id).await.unwrap();
    assert!(matches!(
        tenders.create(1, "create-other-key-01").await,
        Err(CoreError::NotFound)
    ));
    let events: Vec<serde_json::Value> =
        sqlx::query_scalar("SELECT details_json FROM audit_events WHERE module_id='licitaciones'")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert!(!serde_json::to_string(&events).unwrap().contains("Privado"));
    core.admin()
        .set_module_enabled(1, "licitaciones", false)
        .await
        .unwrap();
    assert!(matches!(
        tenders.detail(1, first.id).await,
        Err(CoreError::NotFound)
    ));
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM licitaciones_expedientes")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 2);
    core.admin()
        .set_module_enabled(1, "licitaciones", true)
        .await
        .unwrap();
    let named = tenders.create(1, "create-for-name-001").await.unwrap();
    assert!(named.name.is_empty());
    sqlx::query("DELETE FROM role_permissions WHERE role_id=2 AND module_id='licitaciones' AND capability='editar'").execute(&pool).await.unwrap();
    assert!(matches!(
        tenders
            .rename(
                2,
                named.id,
                1,
                NameInput {
                    name: "No autorizado".into()
                }
            )
            .await,
        Err(CoreError::Forbidden)
    ));
    let (a, b) = tokio::join!(
        tenders.rename(
            1,
            named.id,
            1,
            NameInput {
                name: "Oferta uno".into()
            }
        ),
        tenders.rename(
            1,
            named.id,
            1,
            NameInput {
                name: "Oferta dos".into()
            }
        )
    );
    assert_eq!(usize::from(a.is_ok()) + usize::from(b.is_ok()), 1);
    assert!(matches!(
        a.as_ref().err().or(b.as_ref().err()),
        Some(CoreError::PreconditionFailed)
    ));
    let updated = tenders
        .rename(
            1,
            named.id,
            2,
            NameInput {
                name: "  Equipos de oficina  ".into(),
            },
        )
        .await
        .unwrap();
    assert_eq!(updated.name, "Equipos de oficina");
    assert_eq!(updated.id, named.id);
    assert_eq!(updated.version, 3);
    assert!(updated.forms.is_empty());
    assert_eq!(
        tenders
            .rename(
                1,
                named.id,
                3,
                NameInput {
                    name: updated.name.clone()
                }
            )
            .await
            .unwrap()
            .version,
        3
    );
    assert!(matches!(
        tenders
            .rename(
                1,
                named.id,
                2,
                NameInput {
                    name: updated.name.clone()
                }
            )
            .await,
        Err(CoreError::PreconditionFailed)
    ));
    assert_eq!(
        tenders.list(1, &PageQuery::default()).await.unwrap().items[0].name,
        updated.name
    );
    let sent = tenders.detail(1, first.id).await.unwrap();
    assert!(matches!(
        tenders
            .rename(
                1,
                sent.id,
                sent.version,
                NameInput {
                    name: "No cambiar enviada".into()
                }
            )
            .await,
        Err(CoreError::Conflict)
    ));
    let details:Vec<serde_json::Value>=sqlx::query_scalar("SELECT details_json FROM audit_events WHERE module_id='licitaciones' AND action='expedient.rename'").fetch_all(&pool).await.unwrap();
    assert!(!serde_json::to_string(&details).unwrap().contains("Equipos"));
    core.admin()
        .set_module_enabled(1, "colaboracion", true)
        .await
        .unwrap();
    let exp = tenders.create(1, "create-global-test-001").await.unwrap();
    let a = tenders
        .copy_form(
            1,
            exp.id,
            1,
            "copy-global-test-0001",
            CopyFormInput {
                source_id: published.id,
                revision: 1,
            },
        )
        .await
        .unwrap();
    let b = tenders
        .copy_form(
            1,
            exp.id,
            2,
            "copy-global-test-0002",
            CopyFormInput {
                source_id: published.id,
                revision: 1,
            },
        )
        .await
        .unwrap();
    sqlx::query("UPDATE licitaciones_formularios SET values_json=? WHERE id=?")
        .bind(serde_json::json!({"dato":"Anterior uno"}))
        .bind(a.forms[0].id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE licitaciones_formularios SET values_json=? WHERE id=?")
        .bind(serde_json::json!({"dato":"Anterior dos"}))
        .bind(b.forms[1].id)
        .execute(&pool)
        .await
        .unwrap();
    let before = tenders.detail(1, exp.id).await.unwrap();
    assert!(before.variables.is_empty());
    assert_eq!(before.forms[0].legacy_values["dato"], "Anterior uno");
    assert_eq!(before.forms[1].legacy_values["dato"], "Anterior dos");
    let input = || GlobalVariablesInput {
        variables: vec![GlobalVariable {
            key: "dato".into(),
            label: "Dato".into(),
            kind: VariableKind::Text,
            value: Some(serde_json::json!("Global")),
        }],
    };
    assert!(matches!(
        tenders
            .save_global_variables(2, exp.id, 3, input(), false)
            .await,
        Err(CoreError::Forbidden)
    ));
    let updated = tenders
        .save_global_variables(1, exp.id, 3, input(), false)
        .await
        .unwrap();
    assert_eq!(updated.version, 4);
    assert!(
        updated
            .forms
            .iter()
            .all(|form| form.values["dato"] == "Global" && form.missing_required.is_empty())
    );
    assert_eq!(updated.forms[0].legacy_values["dato"], "Anterior uno");
    assert_eq!(updated.forms[1].legacy_values["dato"], "Anterior dos");
    assert!(matches!(
        tenders
            .save_global_variables(1, exp.id, 3, input(), false)
            .await,
        Err(CoreError::PreconditionFailed)
    ));
    assert!(
        tenders
            .detail(1, named.id)
            .await
            .unwrap()
            .variables
            .is_empty()
    );
    let config = |style: &str, count: &str| GlobalVariablesInput {
        variables: vec![
            GlobalVariable {
                key: "lotes".into(),
                label: "Lotes".into(),
                kind: VariableKind::Integer,
                value: Some(serde_json::json!(count)),
            },
            GlobalVariable {
                key: "stylo".into(),
                label: "Estilo".into(),
                kind: VariableKind::Text,
                value: Some(serde_json::json!(style)),
            },
        ],
    };
    let row = || LotItem {
        descripcion: "Equipo".into(),
        pais_origen: "Honduras".into(),
        fecha_entrega: "30 días".into(),
        cantidad: "2.5".into(),
        unidad: "und".into(),
        transporte: "10.00".into(),
        precio_unitario: "100.00".into(),
    };
    tenders
        .save_global_variables(1, exp.id, 4, config("bid", "2"), false)
        .await
        .unwrap();
    tenders
        .save_lot(1, exp.id, 1, 5, LotInput { items: vec![row()] })
        .await
        .unwrap();
    tenders
        .save_lot(1, exp.id, 2, 6, LotInput { items: vec![row()] })
        .await
        .unwrap();
    assert_eq!(tenders.lot(1, exp.id, 1).await.unwrap().total, "297.50");
    assert!(matches!(
        tenders
            .save_global_variables(1, exp.id, 7, config("std", "1"), false)
            .await,
        Err(CoreError::Conflict)
    ));
    assert_eq!(tenders.lot(1, exp.id, 2).await.unwrap().items.len(), 1);
    tenders
        .save_global_variables(1, exp.id, 7, config("std", "1"), true)
        .await
        .unwrap();
    let lot = tenders.lot(1, exp.id, 1).await.unwrap();
    assert_eq!(lot.total, "287.50");
    assert!(lot.items[0].pais_origen.is_empty());
    assert_eq!(lot.items[0].transporte, "0.00");
    assert!(tenders.lot(1, exp.id, 2).await.is_err());
    let order = tenders.pdf_order(1, exp.id).await.unwrap();
    assert!(order.items.iter().any(|item| item.key == "lote:1"));
    assert!(!order.items.iter().any(|item| item.key == "lote:2"));
    let keys: Vec<String> = order
        .items
        .iter()
        .rev()
        .map(|item| item.key.clone())
        .collect();
    assert!(matches!(
        tenders
            .save_pdf_order(
                1,
                exp.id,
                8,
                PdfOrderInput {
                    items: vec!["archivo:no-pertenece".into()]
                }
            )
            .await,
        Err(CoreError::Invalid)
    ));
    tenders
        .save_pdf_order(
            1,
            exp.id,
            8,
            PdfOrderInput {
                items: keys.clone(),
            },
        )
        .await
        .unwrap();
    let after = tenders.pdf_order(1, exp.id).await.unwrap();
    assert_eq!(
        after
            .items
            .iter()
            .map(|item| item.key.clone())
            .collect::<Vec<_>>(),
        keys
    );
    let mut ready = config("std", "1");
    for (key, kind, value) in [
        ("lpn", VariableKind::Text, "LPN-1"),
        ("inst", VariableKind::Text, "Entidad"),
        ("dia", VariableKind::Integer, "1"),
        ("mes", VariableKind::Text, "enero"),
        ("year", VariableKind::Integer, "2026"),
        ("descripcion", VariableKind::Text, "Equipo"),
        ("dato", VariableKind::Text, "Global"),
    ] {
        ready.variables.push(GlobalVariable {
            key: key.into(),
            label: key.into(),
            kind,
            value: Some(serde_json::json!(value)),
        });
    }
    tenders
        .save_global_variables(1, exp.id, 9, ready, false)
        .await
        .unwrap();
    let template = "#import \"items1_std.typ\": items";
    sqlx::query("DELETE FROM role_permissions WHERE role_id=2 AND module_id='licitaciones' AND capability='ver'").execute(&pool).await.unwrap();
    assert!(matches!(
        tenders
            .prepare_typst_snapshot(2, exp.id, 10, 8, template)
            .await,
        Err(CoreError::Forbidden)
    ));
    assert!(matches!(
        tenders
            .prepare_typst_snapshot(1, exp.id, 9, 8, template)
            .await,
        Err(CoreError::PreconditionFailed)
    ));
    let snapshot = tenders
        .prepare_typst_snapshot(1, exp.id, 10, 8, template)
        .await
        .unwrap();
    assert_eq!(snapshot.expediente.version, 10);
    assert_eq!(
        snapshot
            .order
            .iter()
            .map(|item| item.key.clone())
            .collect::<Vec<_>>(),
        keys
    );
    assert_eq!(snapshot.sources.offered_lots, vec![1]);
    let config: serde_json::Value =
        serde_json::from_slice(&snapshot.sources.files["config.json"]).unwrap();
    assert_eq!(config["total_paginas"], 8);
    let items: serde_json::Value =
        serde_json::from_slice(&snapshot.sources.files["items1_std.json"]).unwrap();
    assert_eq!(items[0]["descripcion"], "Equipo");
    assert!(matches!(
        tenders.create_pdf_job(1, exp.id, 9).await,
        Err(CoreError::PreconditionFailed)
    ));
    let job = tenders.create_pdf_job(1, exp.id, 10).await.unwrap();
    assert_eq!(job.state, "queued");
    let repeated = tenders.create_pdf_job(1, exp.id, 10).await.unwrap();
    assert_eq!(job.id, repeated.id);
    assert_eq!(
        tenders
            .pdf_job(1, exp.id, &job.id)
            .await
            .unwrap()
            .expediente_version,
        10
    );
    assert!(matches!(
        tenders.pdf_job(1, exp.id, "../../bad").await,
        Err(CoreError::Invalid)
    ));
    tenders.delete(1, exp.id, 10, exp.id).await.unwrap();
    let state: (String, bool) =
        sqlx::query_as("SELECT state,cleanup_pending FROM licitaciones_pdf_jobs WHERE id=?")
            .bind(&job.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(state, ("failed".into(), true));
}
