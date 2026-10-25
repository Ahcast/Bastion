use bastion_colaboracion::{CollaborationService, FileFilter};
use bastion_core::{CoreError, CoreService, Database};
use sqlx::mysql::MySqlPoolOptions;

#[tokio::test]
async fn mysql57_migrations_folder_isolation_deletion_and_limits() {
    let Ok(url) = std::env::var("BASTION_COLABORACION_TEST_URL") else {
        eprintln!("Requires isolated BASTION_COLABORACION_TEST_URL; CI supplies it.");
        return;
    };
    let pool = MySqlPoolOptions::new()
        .max_connections(5)
        .connect(&url)
        .await
        .unwrap();
    let database = Database::from_test_pool(pool.clone());
    database.migrate().await.unwrap();
    let core = CoreService::new(database.clone(), "x".repeat(32), [1; 32]).unwrap();
    let service = CollaborationService::new(database.clone(), core.admin());
    service.synchronize_release(true).await.unwrap();
    let allowed: bool = sqlx::query_scalar(
        "SELECT environment_allowed FROM module_registry WHERE module_id='colaboracion'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(!allowed);
    CollaborationService::migrate(&database).await.unwrap();
    CollaborationService::migrate(&database).await.unwrap();
    sqlx::query("INSERT INTO persons (id,full_name) VALUES (1,'Admin test'),(2,'Viewer test'),(3,'Persona privada')").execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO roles (id,name) VALUES (2,'Viewer')")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO users (id,person_id,role_id,email,status) VALUES (1,1,1,'admin@example.test','active'),(2,2,2,'viewer@example.test','active')").execute(&pool).await.unwrap();
    service.synchronize_release(true).await.unwrap();
    assert!(matches!(
        service.list(1, &FileFilter::default()).await,
        Err(CoreError::NotFound)
    ));
    core.admin()
        .set_module_enabled(1, "colaboracion", true)
        .await
        .unwrap();
    assert!(matches!(
        service.list(2, &FileFilter::default()).await,
        Err(CoreError::Forbidden)
    ));
    sqlx::query("INSERT INTO role_permissions (role_id,module_id,scope_hash,scope_key,capability) VALUES (2,'colaboracion',UNHEX(SHA2('persona:2',256)),'persona:2','ver')").execute(&pool).await.unwrap();
    // Synthetic metadata tests SQL boundaries without any real S3 or private files.
    for (id, scope, person, folder) in [
        (1, "empresa", None, "empresa"),
        (2, "persona", Some(2_u64), "persona:2"),
        (3, "persona", Some(3), "persona:3"),
    ] {
        sqlx::query("INSERT INTO colaboracion_files (id,scope,id_persona,folder_key,title,notes,filename,size_bytes,mime,object_key,content_hash,request_hash,actor_id,idempotency_key,state) VALUES (?,?,?,?,?,'nota','a.txt',2,'text/plain',?,UNHEX(SHA2('hi',256)),UNHEX(SHA2('request',256)),1,?,'ready')")
            .bind(id).bind(scope).bind(person).bind(folder).bind(format!("Archivo {id} %")).bind(format!("colaboracion/1/test-{id}")).bind(format!("synthetic-file-key-{id}")).execute(&pool).await.unwrap();
    }
    let files = service.list(2, &FileFilter::default()).await.unwrap();
    assert_eq!(files.items.len(), 1);
    assert_eq!(files.items[0].id, 2);
    let json = serde_json::to_value(&files.items[0]).unwrap();
    for secret in [
        "object_key",
        "request_hash",
        "content_hash",
        "actor_id",
        "idempotency_key",
    ] {
        assert!(json.get(secret).is_none());
    }
    let literal = service
        .list(
            2,
            &FileFilter {
                search: Some("_".into()),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert!(literal.items.is_empty());
    let labels = core
        .admin()
        .module_person_labels(2, "colaboracion", "", 1, 25)
        .await
        .unwrap();
    assert_eq!(labels.len(), 1);
    assert_eq!(labels[0].id, 2);
    assert!(matches!(
        service
            .list(
                2,
                &FileFilter {
                    scope: Some("persona".into()),
                    id_persona: Some(3),
                    ..Default::default()
                }
            )
            .await,
        Err(CoreError::Forbidden)
    ));
    assert!(matches!(
        service.read(2, 3, false).await,
        Err(CoreError::NotFound)
    ));
    assert!(matches!(
        service.delete(2, 2, 2).await,
        Err(CoreError::NotFound)
    ));
    assert!(matches!(
        service.delete(1, 1, 2).await,
        Err(CoreError::Invalid)
    ));
    assert_eq!(
        service
            .list(
                1,
                &FileFilter {
                    page_size: Some(1),
                    ..Default::default()
                }
            )
            .await
            .unwrap()
            .has_more,
        true
    );
    core.admin().require_person_record(3).await.unwrap();
    let deleted_person = sqlx::query("DELETE FROM persons WHERE id=3")
        .execute(&pool)
        .await;
    assert!(deleted_person.is_err());
    service.delete(1, 3, 3).await.unwrap();
    assert!(matches!(
        service.read(1, 3, false).await,
        Err(CoreError::NotFound)
    ));
    let audit: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM audit_events WHERE module_id='colaboracion' AND action='file.trash'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(audit, 1);
    #[cfg(unix)]
    verify_private_storage_lifecycle(&database, &core).await;
    for _ in 0..120 {
        service.consume_request(2).await.unwrap();
    }
    assert!(matches!(
        service.consume_request(2).await,
        Err(CoreError::Locked)
    ));
    core.admin()
        .set_module_enabled(1, "colaboracion", false)
        .await
        .unwrap();
    assert!(matches!(
        service.list(1, &FileFilter::default()).await,
        Err(CoreError::NotFound)
    ));
}

#[cfg(unix)]
async fn verify_private_storage_lifecycle(database: &Database, core: &CoreService) {
    use base64::{Engine, engine::general_purpose::STANDARD};
    use bastion_colaboracion::UploadInput;
    use bastion_core::DocumentService;
    use std::{fs, os::unix::fs::PermissionsExt};
    let root = std::env::temp_dir().join(format!(
        "bastion-collaboration-test-{}",
        DocumentService::object_suffix()
    ));
    fs::create_dir_all(&root).unwrap();
    let aws = root.join("synthetic-aws");
    // Private local fake: no S3 account, production database or public URL.
    let script = format!(
        r#"#!/usr/bin/python3
import json, pathlib, shutil, sys
root = pathlib.Path({})
args = sys.argv[1:]
key = args[args.index('--key')+1] if '--key' in args else args[args.index('--prefix')+1]
path = root / 'objects' / key
op = args[1]
if op == 'list-object-versions':
    print(json.dumps(dict(IsTruncated=False, Versions=[dict(Key=key, VersionId='null')] if path.exists() else [])))
elif op == 'put-object':
    path.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(args[args.index('--body')+1], path)
    if (root / 'fail-put').exists():
        (root / 'fail-put').unlink()
        sys.exit(1)
elif op == 'get-object':
    shutil.copyfile(path, args[-1])
elif op == 'delete-object':
    if (root / 'fail-delete').exists():
        sys.exit(1)
    path.unlink(missing_ok=True)
else:
    sys.exit(2)
"#,
        serde_json::to_string(root.to_str().unwrap()).unwrap()
    );
    fs::write(&aws, script).unwrap();
    fs::set_permissions(&aws, fs::Permissions::from_mode(0o700)).unwrap();
    let service = CollaborationService::with_storage(
        database.clone(),
        core.admin(),
        DocumentService::synthetic_storage(aws, root.join("work")),
    );
    let default_policy = service.policy(1).await.unwrap();
    assert_eq!(default_policy.max_file_mb, 50);
    assert_eq!(default_policy.trash_retention_days, 30);
    assert!(matches!(
        service.save_policy(2, default_policy.clone()).await,
        Err(CoreError::Forbidden)
    ));
    let input = || UploadInput {
        scope: "persona".into(),
        id_persona: Some(2),
        title: "Documento sintético".into(),
        notes: "Nota".into(),
        filename: "a.txt".into(),
        content_base64: STANDARD.encode("Contenido sintético".as_bytes()),
    };
    fs::write(root.join("fail-put"), b"synthetic").unwrap();
    assert!(
        service
            .upload(1, "storage-retry-key-01", input())
            .await
            .is_err()
    );
    let record = service
        .upload(1, "storage-retry-key-01", input())
        .await
        .unwrap();
    assert_eq!(
        service
            .upload(1, "storage-retry-key-01", input())
            .await
            .unwrap()
            .id,
        record.id
    );
    let mut changed = input();
    changed.notes = "Cambio".into();
    assert!(matches!(
        service.upload(1, "storage-retry-key-01", changed).await,
        Err(CoreError::Conflict)
    ));
    let downloaded = service.read(2, record.id, false).await.unwrap();
    assert_eq!(
        STANDARD.decode(downloaded.content_base64).unwrap(),
        "Contenido sintético".as_bytes()
    );
    assert_eq!(
        service.read(2, record.id, true).await.unwrap().mime,
        "text/plain"
    );
    let object: String = sqlx::query_scalar("SELECT object_key FROM colaboracion_files WHERE id=?")
        .bind(record.id)
        .fetch_one(database.pool())
        .await
        .unwrap();
    fs::write(root.join("objects").join(&object), b"alterado").unwrap();
    assert!(matches!(
        service.read(2, record.id, false).await,
        Err(CoreError::Overloaded)
    ));
    service.delete(1, record.id, record.id).await.unwrap();
    service.restore(1, record.id).await.unwrap();
    assert!(
        service
            .list(2, &FileFilter::default())
            .await
            .unwrap()
            .items
            .iter()
            .any(|item| item.id == record.id)
    );
    service.delete(1, record.id, record.id).await.unwrap();
    assert!(service.purge(1, record.id, record.id + 1).await.is_err());
    service.purge(1, record.id, record.id).await.unwrap();
    fs::write(root.join("fail-delete"), b"synthetic").unwrap();
    assert!(service.clean_one().await.is_err());
    fs::remove_file(root.join("fail-delete")).unwrap();
    while service.clean_one().await.unwrap() {}
    assert!(!root.join("objects").join(object).exists());
    let expired = service
        .upload(1, "storage-retention-01", input())
        .await
        .unwrap();
    service.delete(1, expired.id, expired.id).await.unwrap();
    sqlx::query(
        "UPDATE colaboracion_files SET purge_after=UTC_TIMESTAMP(6)-INTERVAL 1 SECOND WHERE id=?",
    )
    .bind(expired.id)
    .execute(database.pool())
    .await
    .unwrap();
    assert!(matches!(
        service.restore(1, expired.id).await,
        Err(CoreError::Conflict)
    ));
    while service.clean_one().await.unwrap() {}
    let state: String = sqlx::query_scalar("SELECT state FROM colaboracion_files WHERE id=?")
        .bind(expired.id)
        .fetch_one(database.pool())
        .await
        .unwrap();
    assert_eq!(state, "deleted");
    assert!(matches!(
        service.upload(1, "storage-retry-key-01", input()).await,
        Err(CoreError::Conflict)
    ));
    fs::remove_dir_all(root).unwrap();
}
