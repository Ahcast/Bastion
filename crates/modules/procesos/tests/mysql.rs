use std::collections::BTreeMap;

use bastion_core::{CoreError, CoreService, Database};
use bastion_procesos::{FIELDS, ProcessFilter, ProcessService};
use sqlx::mysql::MySqlPoolOptions;

fn csv(id: char, observation: &str, hash: char) -> String {
    let mut values = BTreeMap::new();
    for field in FIELDS {
        values.insert(field, "synthetic".to_owned());
    }
    for field in [
        "fecha_recepcion",
        "extraido_en",
        "primera_deteccion",
        "ultima_modificacion",
        "analizado_en",
    ] {
        values.insert(field, "2026-10-21T19:00:00-06:00".into());
    }
    values.insert("ultima_deteccion", observation.into());
    values.insert("proceso_id", format!("hc:{}", id.to_string().repeat(64)));
    values.insert("url", "https://example.test/detalle?id=1".into());
    values.insert("entidad", "Entidad de prueba".into());
    values.insert("expediente", "EXP-001".into());
    values.insert("contenido_hash", hash.to_string().repeat(64));
    values.insert("hash_analizado", hash.to_string().repeat(64));
    values.insert("analisis_estado", "analizado".into());
    values.insert("extraccion_estado", "completo".into());
    values.insert("analisis_error", String::new());
    values.insert("puntaje", "80".into());
    values.insert("items", "[\"Dos equipos\"]".into());
    values.insert("items_fuente", "[]".into());
    let mut writer = csv::Writer::from_writer(Vec::new());
    writer.write_record(FIELDS).unwrap();
    writer.write_record(FIELDS.map(|key| &values[key])).unwrap();
    format!(
        "\u{feff}{}",
        String::from_utf8(writer.into_inner().unwrap()).unwrap()
    )
}

#[tokio::test]
async fn mysql57_import_permissions_history_and_disable() {
    let Ok(url) = std::env::var("BASTION_PROCESOS_TEST_URL") else {
        eprintln!(
            "MySQL integration test requires isolated BASTION_PROCESOS_TEST_URL; CI supplies it."
        );
        return;
    };
    // Only a disposable CI database, never the application's configured MYSQL_*.
    let pool = MySqlPoolOptions::new()
        .max_connections(3)
        .connect(&url)
        .await
        .unwrap();
    let database = Database::from_test_pool(pool.clone());
    database.migrate().await.unwrap();
    ProcessService::migrate(&database).await.unwrap();
    // Both migration ledgers must support repeated execution without collision.
    database.migrate().await.unwrap();
    ProcessService::migrate(&database).await.unwrap();
    sqlx::query(
        "INSERT INTO persons (id,full_name) VALUES (1,'Admin test'),(2,'Admin two'),(3,'Viewer')",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO roles (id,name) VALUES (2,'Empty role')")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO users (id,person_id,role_id,email,status) VALUES (1,1,1,'admin@example.test','active'),(2,2,1,'admin2@example.test','active'),(3,3,2,'viewer@example.test','active')").execute(&pool).await.unwrap();
    let core = CoreService::new(database.clone(), "x".repeat(32), [1; 32]).unwrap();
    let service = ProcessService::new(database, core.admin());
    service.synchronize_release(true).await.unwrap();
    assert!(matches!(service.status(1).await, Err(CoreError::NotFound)));
    core.admin()
        .set_module_enabled(1, "procesos", true)
        .await
        .unwrap();
    assert!(matches!(service.status(3).await, Err(CoreError::Forbidden)));
    let first = csv('a', "2026-10-21T20:00:00-06:00", 'b');
    assert_eq!(service.import(1, &first).await.unwrap().inserted, 1);
    assert!(service.import(1, &first).await.unwrap().replayed);
    let next = csv('a', "2026-10-21T21:00:00-06:00", 'c');
    assert_eq!(service.import(1, &next).await.unwrap().updated, 1);
    assert_eq!(service.import(2, &first).await.unwrap().stale, 1);
    let inconsistent = csv('a', "2026-10-21T21:00:00-06:00", 'd');
    assert!(matches!(
        service.import(2, &inconsistent).await,
        Err(CoreError::Conflict)
    ));
    let second = csv('e', "2026-10-21T21:00:00-06:00", 'f');
    assert_eq!(service.import(2, &second).await.unwrap().inserted, 1);
    assert_eq!(service.status(1).await.unwrap()["total"], 2);
    assert!(matches!(
        service
            .import(2, &csv('a', "2026-10-21T22:00:00-06:00", 'e'))
            .await,
        Err(CoreError::Locked)
    ));
    let id = format!("hc:{}", "a".repeat(64));
    let detail = service.detail(1, &id).await.unwrap();
    assert_eq!(detail.source["contenido_hash"], "c".repeat(64));
    assert!(
        service
            .import(1, &first.replace("analizado,", "pendiente,"))
            .await
            .is_err()
    );
    let filter = ProcessFilter {
        desde: Some("2026-10-21".parse().unwrap()),
        hasta: Some("2026-10-21".parse().unwrap()),
        ..Default::default()
    };
    assert_eq!(service.rows(1, &filter).await.unwrap().items.len(), 2);
    assert!(
        service
            .export(1, &filter)
            .await
            .unwrap()
            .starts_with('\u{feff}')
    );
    assert!(matches!(
        service.export(3, &filter).await,
        Err(CoreError::Forbidden)
    ));
    core.admin()
        .set_module_enabled(1, "procesos", false)
        .await
        .unwrap();
    assert!(matches!(
        service.rows(1, &filter).await,
        Err(CoreError::NotFound)
    ));
    assert!(matches!(
        service.detail(1, &id).await,
        Err(CoreError::NotFound)
    ));
    assert!(matches!(
        service.import(1, &first).await,
        Err(CoreError::NotFound)
    ));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM procesos_rows")
            .fetch_one(&pool)
            .await
            .unwrap(),
        2
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM audit_events WHERE module_id='procesos' AND action='import'"
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        4
    );
}
