use std::collections::BTreeMap;

use bastion_core::{CoreError, CoreService, Database, DomainActor};
use bastion_procesos::{
    FIELDS, ProcessFilter, ProcessService, ProcessSort, ReadyDelivery, SCOUT_FIELDS,
};
use chrono::{Datelike, Days, NaiveDate, NaiveDateTime};
use sha2::{Digest, Sha256};
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

fn batch_csv(count: u64, start: u64, last: Option<&str>) -> String {
    let seed = csv('a', "2026-10-21T22:00:00-06:00", 'f');
    let mut reader = csv::Reader::from_reader(seed.trim_start_matches('\u{feff}').as_bytes());
    let record = reader.records().next().unwrap().unwrap();
    let mut writer = csv::Writer::from_writer(Vec::new());
    writer.write_record(FIELDS).unwrap();
    for index in start..start + count {
        let mut fields = record.iter().map(str::to_owned).collect::<Vec<_>>();
        fields[1] = format!("hc:{index:064x}");
        writer.write_record(fields).unwrap();
    }
    if let Some(last) = last {
        let mut reader = csv::Reader::from_reader(last.trim_start_matches('\u{feff}').as_bytes());
        for record in reader.records() {
            writer.write_record(record.unwrap().iter()).unwrap();
        }
    }
    String::from_utf8(writer.into_inner().unwrap()).unwrap()
}

struct DeliveryFiles(std::path::PathBuf);
impl DeliveryFiles {
    fn new(legacy: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "bastion-scout-db-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&path).unwrap();
        let mut reader = csv::Reader::from_reader(legacy.trim_start_matches('\u{feff}').as_bytes());
        let mut writer = csv::Writer::from_writer(Vec::new());
        writer.write_record(SCOUT_FIELDS).unwrap();
        for record in reader.records() {
            let mut fields = record
                .unwrap()
                .iter()
                .map(str::to_owned)
                .collect::<Vec<_>>();
            let items: Vec<String> = serde_json::from_str(&fields[18]).unwrap();
            fields[18] = items
                .iter()
                .enumerate()
                .map(|(n, item)| format!("{}. {}", n + 1, item))
                .collect::<Vec<_>>()
                .join("\n");
            writer.write_record(fields).unwrap();
        }
        let csv = format!(
            "\u{feff}{}",
            String::from_utf8(writer.into_inner().unwrap()).unwrap()
        );
        let manifest = serde_json::json!({"status":"exitoso","output_ready":true,"run_id":"2026-10-22","maestro_sha256":hex::encode(Sha256::digest(csv.as_bytes())),"total":1,"stages":{"scout":"exitoso","combiner":"exitoso","clasificador":"exitoso"}});
        std::fs::write(path.join("maestro.csv"), csv).unwrap();
        std::fs::write(path.join("main.json"), manifest.to_string()).unwrap();
        Self(path)
    }
    fn read(&self) -> ReadyDelivery {
        ReadyDelivery::read(&self.0.join("maestro.csv")).unwrap()
    }
}
impl Drop for DeliveryFiles {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
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
    let id = format!("hc:{}", "a".repeat(64));
    assert_eq!(
        service
            .review(1, &id, 0, Some(true), None)
            .await
            .unwrap()
            .version,
        1
    );
    let shared = service.review(2, &id, 1, None, Some("=1+1")).await.unwrap();
    assert_eq!(shared.version, 2);
    assert!(matches!(
        service.review(1, &id, 1, Some(false), None).await,
        Err(CoreError::PreconditionFailed)
    ));
    assert!(matches!(
        service.review(3, &id, 2, None, Some("No permitido")).await,
        Err(CoreError::Forbidden)
    ));
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
    // Upgrade existing rows, including both offset and UTC source timestamps.
    sqlx::query("UPDATE procesos_rows SET source_json=JSON_SET(source_json,'$.primera_deteccion','2026-10-22T01:00:00Z') WHERE proceso_id=?")
        .bind(format!("hc:{}", "e".repeat(64))).execute(&pool).await.unwrap();
    sqlx::query("ALTER TABLE procesos_rows DROP INDEX ix_procesos_primera_deteccion, DROP COLUMN primera_deteccion")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::raw_sql(include_str!("../migrations/0004_first_detection.sql"))
        .execute(&pool)
        .await
        .unwrap();
    let recovered: Vec<NaiveDateTime> =
        sqlx::query_scalar("SELECT primera_deteccion FROM procesos_rows")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert!(
        recovered
            .iter()
            .all(|date| date.to_string() == "2026-10-22 01:00:00")
    );
    assert!(matches!(
        service
            .import(2, &csv('a', "2026-10-21T22:00:00-06:00", 'e'))
            .await,
        Err(CoreError::Locked)
    ));
    let detail = service.detail(1, &id).await.unwrap();
    assert_eq!(detail.source["contenido_hash"], "c".repeat(64));
    assert!(detail.marked);
    assert_eq!(detail.note, "=1+1");
    assert_eq!(detail.version, 2);
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
    let marked_filter = ProcessFilter {
        marked: Some(true),
        ..Default::default()
    };
    assert_eq!(
        service.rows(2, &marked_filter).await.unwrap().items.len(),
        1
    );
    let exported = service.export(1, &filter).await.unwrap();
    assert!(exported.contains("de_interes,nota"));
    assert!(exported.contains("'=1+1"));
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
    // Expected Scout volume; a second actor must not duplicate the same 600 rows.
    sqlx::query("UPDATE procesos_imports SET created_at=UTC_TIMESTAMP(6)-INTERVAL 5 MINUTE")
        .execute(&pool)
        .await
        .unwrap();
    let bulk = batch_csv(600, 0, None);
    let started = std::time::Instant::now();
    assert_eq!(service.import(1, &bulk).await.unwrap().inserted, 600);
    assert_eq!(service.import(2, &bulk).await.unwrap().unchanged, 600);
    eprintln!(
        "600 synthetic Scout rows plus second-actor reimport: {:?}",
        started.elapsed()
    );
    // A conflict after the first write batch must roll back those earlier writes.
    let rollback = batch_csv(101, 1000, Some(&inconsistent));
    assert!(matches!(
        service.import(1, &rollback).await,
        Err(CoreError::Conflict)
    ));
    assert_eq!(service.status(1).await.unwrap()["total"], 602);
    assert!(service.detail(2, &id).await.unwrap().marked);
    // Current Scout format can replace an equivalent legacy row without changing its identity or review.
    let files = DeliveryFiles::new(&next);
    let delivery = files.read();
    assert!(matches!(
        service.import_local_scout(&delivery).await,
        Err(CoreError::Forbidden)
    ));
    let local_core = core.clone().with_local_scout_import(true);
    let local = ProcessService::new(local_core.database(), local_core.admin());
    assert!(
        local_core
            .admin()
            .authorize_actor(DomainActor::LocalScout, "procesos", "importacion", "editar")
            .await
            .is_ok()
    );
    assert!(matches!(
        local_core
            .admin()
            .authorize_actor(DomainActor::LocalScout, "procesos", "marcas", "editar")
            .await,
        Err(CoreError::Forbidden)
    ));
    assert!(matches!(
        local_core
            .admin()
            .authorize_actor(DomainActor::LocalScout, "core", "usuarios", "editar")
            .await,
        Err(CoreError::Forbidden)
    ));
    let receipt = local.import_local_scout(&delivery).await.unwrap();
    assert_eq!(receipt.unchanged, 1);
    let restarted = ProcessService::new(local_core.database(), local_core.admin());
    assert!(
        restarted
            .import_local_scout(&files.read())
            .await
            .unwrap()
            .replayed
    );
    let record = service.detail(2, &id).await.unwrap();
    // A user-triggered refresh works without enabling the internal Scout actor.
    assert!(matches!(
        service.import_scout_for_user(3, &delivery).await,
        Err(CoreError::Forbidden)
    ));
    let refreshed = service.import_scout_for_user(1, &delivery).await.unwrap();
    assert!(!refreshed.replayed);
    assert_eq!(refreshed.unchanged, 1);
    assert!(
        service
            .import_scout_for_user(1, &delivery)
            .await
            .unwrap()
            .replayed
    );
    assert!(record.marked);
    assert_eq!(record.note, "=1+1");
    assert_eq!(record.source["items"], "1. Dos equipos");
    assert!(record.source.get("resumen").is_some());
    let audit = core
        .admin()
        .list_audit(
            1,
            None,
            None,
            Some("procesos"),
            Some("import"),
            None,
            1,
            100,
        )
        .await
        .unwrap();
    assert!(
        audit
            .iter()
            .any(|event| event.actor_name.as_deref() == Some("bastion-scout (local)"))
    );
    // Overview counts all matching rows, not just a page, and uses Honduras day boundaries.
    let base = service.status(1).await.unwrap();
    let today: NaiveDate = base["as_of"].as_str().unwrap().parse().unwrap();
    let midnight = today.and_hms_opt(6, 0, 0).unwrap();
    let monday = today
        .checked_sub_days(Days::new(u64::from(today.weekday().num_days_from_monday())))
        .unwrap()
        .and_hms_opt(6, 0, 0)
        .unwrap();
    sqlx::query("UPDATE procesos_rows SET fecha_recepcion=?,primera_deteccion=?")
        .bind(midnight - chrono::Duration::days(60))
        .bind(monday - chrono::Duration::days(60))
        .execute(&pool)
        .await
        .unwrap();
    let municipality = format!("hc:{}", "e".repeat(64));
    let bulk_first = format!("hc:{:064x}", 0);
    let bulk_second = format!("hc:{:064x}", 1);
    for (key, entity, date, score, discovery) in [
        (&id, "Secretaría de Salud", midnight, 80, monday),
        (
            &municipality,
            "Alcaldía Municipal de Prueba",
            midnight + chrono::Duration::days(1),
            20,
            monday,
        ),
        (
            &bulk_first,
            "Secretaría de Salud Especial",
            midnight + chrono::Duration::days(2),
            95,
            monday - chrono::Duration::seconds(1),
        ),
        (
            &bulk_second,
            "Municipalidad de Otro Lugar",
            midnight - chrono::Duration::seconds(1),
            95,
            monday,
        ),
    ] {
        sqlx::query("UPDATE procesos_rows SET entidad=?,fecha_recepcion=?,puntaje=?,primera_deteccion=?,expediente='TEST-SEARCH',resumen_corto='SOLO_IA',source_json=JSON_SET(source_json,'$.descripcion','Una descripción','$.items_fuente','[{\"especificacion\":\"osciloscopio 50%_\"}]') WHERE proceso_id=?")
            .bind(entity).bind(date).bind(score as u8).bind(discovery).bind(key).execute(&pool).await.unwrap();
    }
    let dashboard = service
        .overview(
            1,
            &ProcessFilter {
                page_size: Some(1),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(dashboard["total"], 602);
    assert_eq!(dashboard["due"][0]["count"], 1);
    assert_eq!(dashboard["due"][1]["count"], 1);
    assert_eq!(dashboard["due"][2]["count"], 1);
    assert_eq!(dashboard["weekly"][5]["count"], 3);
    assert_eq!(dashboard["weekly"][4]["count"], 1);
    let hidden = ProcessFilter {
        hide_municipalities: Some(true),
        ..Default::default()
    };
    let dashboard = service.overview(1, &hidden).await.unwrap();
    assert_eq!(dashboard["total"], 600);
    assert_eq!(dashboard["due"][1]["count"], 0);
    assert_eq!(dashboard["weekly"][5]["count"], 1);
    let entities = service.entities(1).await.unwrap();
    assert!(
        entities["entities"]
            .as_array()
            .unwrap()
            .iter()
            .all(|v| !v.as_str().unwrap().contains("Municipal"))
    );
    assert!(
        entities["entities"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v == "Secretaría de Salud")
    );
    let exact = ProcessFilter {
        entidad: Some("Secretaría de Salud".into()),
        ..Default::default()
    };
    assert_eq!(service.rows(1, &exact).await.unwrap().items.len(), 1);
    let marked = ProcessFilter {
        marked: Some(true),
        ..Default::default()
    };
    assert_eq!(service.overview(1, &marked).await.unwrap()["total"], 1);
    let search = ProcessFilter {
        q: Some("osciloscopio 50%_".into()),
        ..Default::default()
    };
    assert_eq!(service.rows(1, &search).await.unwrap().items.len(), 4);
    assert!(
        service
            .rows(
                1,
                &ProcessFilter {
                    q: Some("SOLO_IA".into()),
                    ..Default::default()
                }
            )
            .await
            .unwrap()
            .items
            .is_empty()
    );
    for (sort, first) in [
        (ProcessSort::FechaAsc, &bulk_second),
        (ProcessSort::FechaDesc, &bulk_first),
        (ProcessSort::PuntajeDesc, &bulk_first),
        (ProcessSort::PuntajeAsc, &municipality),
    ] {
        let filter = ProcessFilter {
            sort: Some(sort),
            q: Some("TEST-SEARCH".into()),
            ..Default::default()
        };
        let rows = service.rows(1, &filter).await.unwrap().items;
        assert_eq!(&rows[0].proceso_id, first);
        let exported = service.export(1, &filter).await.unwrap();
        let mut reader =
            csv::Reader::from_reader(exported.trim_start_matches('\u{feff}').as_bytes());
        assert_eq!(
            &reader.records().next().unwrap().unwrap()[1],
            first.as_str()
        );
    }
    assert!(matches!(
        service.overview(3, &hidden).await,
        Err(CoreError::Forbidden)
    ));
    assert!(matches!(
        service.entities(3).await,
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
        service.entities(1).await,
        Err(CoreError::NotFound)
    ));
    assert!(matches!(
        service.overview(1, &hidden).await,
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
    assert!(matches!(
        local.import_local_scout(&delivery).await,
        Err(CoreError::NotFound)
    ));
    assert!(matches!(
        service.import_scout_for_user(1, &delivery).await,
        Err(CoreError::NotFound)
    ));
    assert!(matches!(
        service.review(1, &id, 2, Some(false), None).await,
        Err(CoreError::NotFound)
    ));
    // An incompatible migration ledger must also block reactivation.
    sqlx::query("UPDATE _procesos_migrations SET checksum=REPEAT('x',48) WHERE version=2")
        .execute(&pool)
        .await
        .unwrap();
    service.synchronize_release(true).await.unwrap();
    assert!(matches!(
        core.admin().set_module_enabled(1, "procesos", true).await,
        Err(CoreError::Forbidden)
    ));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM procesos_rows")
            .fetch_one(&pool)
            .await
            .unwrap(),
        602
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM audit_events WHERE module_id='procesos' AND action='import'"
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        8
    );
}
