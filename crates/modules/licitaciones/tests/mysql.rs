use bastion_colaboracion::{
    CollaborationService, FormDefinition, FormSection, FormVariable, PublishFormInput, VariableKind,
};
use bastion_core::{CoreError, CoreService, Database};
use bastion_licitaciones::{CopyFormInput, PageQuery, TenderService, ValuesInput};
use sqlx::mysql::MySqlPoolOptions;

fn definition(title: &str) -> FormDefinition {
    FormDefinition {
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
}
