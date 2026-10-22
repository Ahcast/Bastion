use bastion_core::{CoreAdmin, CoreError, Database};
use chrono::{DateTime, FixedOffset, NaiveDate, NaiveDateTime, TimeZone};
use futures_util::TryStreamExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{FromRow, MySql, QueryBuilder, Transaction};
use std::collections::BTreeMap;

use crate::{FIELDS, MAX_ROWS, ScoutRow, parse_csv};

#[derive(Clone)]
pub struct ProcessService {
    database: Database,
    admin: CoreAdmin,
}

#[derive(Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProcessFilter {
    pub marked: Option<bool>,
    pub q: Option<String>,
    pub entidad: Option<String>,
    pub desde: Option<NaiveDate>,
    pub hasta: Option<NaiveDate>,
    pub page: Option<u32>,
    pub page_size: Option<u32>,
}

#[derive(Serialize, FromRow)]
pub struct ProcessRecord {
    pub marked: bool,
    pub note: String,
    pub version: u64,
    pub proceso_id: String,
    pub entidad: String,
    pub expediente: String,
    #[serde(serialize_with = "utc_date")]
    pub fecha_recepcion: NaiveDateTime,
    pub puntaje: u8,
    pub rubro: String,
    pub resumen_corto: String,
    pub source: serde_json::Value,
}

fn utc_date<S: serde::Serializer>(date: &NaiveDateTime, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(&date.and_utc().to_rfc3339())
}

#[derive(Serialize)]
pub struct ProcessPage {
    pub items: Vec<ProcessRecord>,
    pub page: u32,
    pub page_size: u32,
    pub has_more: bool,
}

#[derive(Default, Serialize, Deserialize)]
pub struct ImportResult {
    pub import_id: u64,
    pub inserted: u32,
    pub updated: u32,
    pub unchanged: u32,
    pub stale: u32,
    pub replayed: bool,
}

#[derive(Serialize, FromRow)]
pub struct ReviewRecord {
    pub marked: bool,
    pub note: String,
    pub version: u64,
}

impl ProcessService {
    pub async fn check_enabled(&self) -> Result<(), CoreError> {
        let active: Option<bool> = sqlx::query_scalar("SELECT (included_in_release AND environment_allowed AND enabled) FROM module_registry WHERE module_id='procesos'")
            .fetch_optional(self.database.pool()).await?;
        if active == Some(true) {
            Ok(())
        } else {
            Err(CoreError::NotFound)
        }
    }

    pub async fn check_access(
        &self,
        actor: u64,
        scope: &str,
        capability: &str,
    ) -> Result<(), CoreError> {
        self.begin(actor, scope, capability).await?.commit().await?;
        Ok(())
    }

    pub async fn consume_request(&self, actor: u64) -> Result<(), CoreError> {
        let mut tx = self.database.pool().begin().await?;
        sqlx::query("INSERT INTO procesos_actor_limits (actor_id,window_start,requests) VALUES (?,UTC_TIMESTAMP(6),1) ON DUPLICATE KEY UPDATE requests=IF(window_start<=UTC_TIMESTAMP(6)-INTERVAL 1 MINUTE,1,requests+1),window_start=IF(window_start<=UTC_TIMESTAMP(6)-INTERVAL 1 MINUTE,UTC_TIMESTAMP(6),window_start)")
            .bind(actor).execute(&mut *tx).await?;
        let requests: u32 =
            sqlx::query_scalar("SELECT requests FROM procesos_actor_limits WHERE actor_id=?")
                .bind(actor)
                .fetch_one(&mut *tx)
                .await?;
        tx.commit().await?;
        if requests > 120 {
            return Err(CoreError::Locked);
        }
        Ok(())
    }

    pub fn new(database: Database, admin: CoreAdmin) -> Self {
        Self { database, admin }
    }

    pub async fn migrate(database: &Database) -> Result<(), CoreError> {
        let mut migrator = sqlx::migrate!("./migrations");
        // Fixed, module-owned ledger; never a user-provided SQL identifier.
        migrator.dangerous_set_table_name("_procesos_migrations");
        migrator
            .run(database.pool())
            .await
            .map_err(CoreError::Migration)
    }

    pub async fn synchronize_release(&self, allowed: bool) -> Result<(), CoreError> {
        let exists: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM information_schema.tables WHERE table_schema=DATABASE() AND table_name='_procesos_migrations'")
            .fetch_one(self.database.pool()).await?;
        let ready = if exists > 0 {
            let applied = sqlx::query_as::<_, (i64, bool, Vec<u8>)>(
                "SELECT version,success,checksum FROM _procesos_migrations",
            )
            .fetch_all(self.database.pool())
            .await?;
            let expected = sqlx::migrate!("./migrations");
            applied.len() == expected.iter().count()
                && expected.iter().all(|migration| {
                    applied.iter().any(|(version, success, checksum)| {
                        *version == migration.version
                            && *success
                            && checksum.as_slice() == migration.checksum.as_ref()
                    })
                })
        } else {
            false
        };
        sqlx::query("INSERT INTO module_registry (module_id,display_name,included_in_release,environment_allowed,enabled) VALUES ('procesos','Procesos',?, ?,0) ON DUPLICATE KEY UPDATE included_in_release=VALUES(included_in_release),environment_allowed=VALUES(environment_allowed)")
            .bind(true).bind(allowed && ready).execute(self.database.pool()).await?;
        Ok(())
    }

    async fn begin(
        &self,
        actor: u64,
        scope: &str,
        capability: &str,
    ) -> Result<Transaction<'_, MySql>, CoreError> {
        self.admin
            .authorize(actor, "procesos", scope, capability)
            .await?;
        let mut tx = self.database.pool().begin().await?;
        // Serialize with administrative disable and with other domain operations.
        let enabled: Option<bool> = sqlx::query_scalar("SELECT (included_in_release AND environment_allowed AND enabled) FROM module_registry WHERE module_id='procesos' FOR UPDATE")
            .fetch_optional(&mut *tx).await?;
        if enabled != Some(true) {
            return Err(CoreError::NotFound);
        }
        Ok(tx)
    }

    pub async fn status(&self, actor: u64) -> Result<serde_json::Value, CoreError> {
        let mut tx = self.begin(actor, "consulta", "ver").await?;
        let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM procesos_rows")
            .fetch_one(&mut *tx)
            .await?;
        let last: Option<NaiveDateTime> =
            sqlx::query_scalar("SELECT MAX(created_at) FROM procesos_imports")
                .fetch_one(&mut *tx)
                .await?;
        tx.commit().await?;
        Ok(
            serde_json::json!({"total":total,"last_import_at":last.map(|d| d.and_utc().to_rfc3339())}),
        )
    }

    pub async fn rows(&self, actor: u64, filter: &ProcessFilter) -> Result<ProcessPage, CoreError> {
        let (page, size, offset) = filter.bounds()?;
        let mut tx = self.begin(actor, "consulta", "ver").await?;
        let mut query = select_rows(filter, false)?;
        query
            .push(" ORDER BY r.fecha_recepcion DESC,r.proceso_id ASC LIMIT ")
            .push_bind(size + 1)
            .push(" OFFSET ")
            .push_bind(offset);
        let mut items = query
            .build_query_as::<ProcessRecord>()
            .fetch_all(&mut *tx)
            .await?;
        let has_more = items.len() > size as usize;
        items.truncate(size as usize);
        tx.commit().await?;
        Ok(ProcessPage {
            items,
            page,
            page_size: size,
            has_more,
        })
    }

    pub async fn detail(&self, actor: u64, id: &str) -> Result<ProcessRecord, CoreError> {
        if id.len() != 67 {
            return Err(CoreError::Invalid);
        }
        let mut tx = self.begin(actor, "consulta", "ver").await?;
        let record = sqlx::query_as::<_, ProcessRecord>("SELECT r.proceso_id,r.entidad,r.expediente,r.fecha_recepcion,r.puntaje,r.rubro,r.resumen_corto,r.source_json AS source,COALESCE(m.marked,0) AS marked,COALESCE(m.note,'') AS note,CAST(COALESCE(m.version,0) AS UNSIGNED) AS version FROM procesos_rows r LEFT JOIN procesos_marks m ON m.proceso_id=r.proceso_id WHERE r.proceso_id=?")
            .bind(id).fetch_optional(&mut *tx).await?.ok_or(CoreError::NotFound)?;
        tx.commit().await?;
        Ok(record)
    }

    pub async fn import(&self, actor: u64, csv: &str) -> Result<ImportResult, CoreError> {
        let mut tx = self.begin(actor, "importacion", "editar").await?;
        // Database-backed quota survives restarts and applies across instances.
        let hash = Sha256::digest(csv.as_bytes()).to_vec();
        if let Some(previous) = sqlx::query_scalar::<_, serde_json::Value>(
            "SELECT result_json FROM procesos_imports WHERE actor_id=? AND file_hash=?",
        )
        .bind(actor)
        .bind(&hash)
        .fetch_optional(&mut *tx)
        .await?
        {
            let mut result: ImportResult =
                serde_json::from_value(previous).map_err(|_| CoreError::Conflict)?;
            result.replayed = true;
            return Ok(result);
        }
        let recent: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM procesos_imports WHERE actor_id=? AND created_at>UTC_TIMESTAMP(6)-INTERVAL 1 MINUTE")
            .bind(actor).fetch_one(&mut *tx).await?;
        if recent >= 2 {
            return Err(CoreError::Locked);
        }
        let rows = parse_csv(csv)?;
        let mut result = ImportResult::default();
        let mut existing = BTreeMap::new();
        if !rows.is_empty() {
            let mut query = QueryBuilder::<MySql>::new(
                "SELECT proceso_id,contenido_hash,ultima_deteccion FROM procesos_rows WHERE proceso_id IN (",
            );
            let mut ids = query.separated(",");
            for row in &rows {
                ids.push_bind(&row.proceso_id);
            }
            query.push(") FOR UPDATE");
            let current = query
                .build_query_as::<(String, String, NaiveDateTime)>()
                .fetch_all(&mut *tx)
                .await?;
            existing.extend(
                current
                    .into_iter()
                    .map(|(id, hash, seen)| (id, (hash, seen))),
            );
        }
        let mut pending = Vec::new();
        for row in &rows {
            let old = existing.get(&row.proceso_id);
            if old.is_some_and(|(_, seen)| row.ultima_deteccion.naive_utc() < *seen) {
                result.stale += 1;
                continue;
            }
            match old {
                Some((hash, _)) if *hash == row.contenido_hash => result.unchanged += 1,
                Some(_) => result.updated += 1,
                None => result.inserted += 1,
            }
            let source = serde_json::to_string(&row.source).map_err(|_| CoreError::Invalid)?;
            // Bound each statement below the usual MySQL 5.7 packet budget.
            if source.len() > 1024 * 1024 {
                return Err(CoreError::Invalid);
            }
            let same_observation =
                old.is_some_and(|(_, seen)| row.ultima_deteccion.naive_utc() == *seen);
            pending.push((row, source, same_observation));
        }
        let mut start = 0;
        while start < pending.len() {
            let mut end = start;
            let mut bytes = 0;
            while end < pending.len()
                && end - start < 100
                && (end == start || bytes + pending[end].1.len() <= 512 * 1024)
            {
                bytes += pending[end].1.len();
                end += 1;
            }
            save_batch(&mut tx, &pending[start..end]).await?;
            start = end;
        }
        result.import_id = sqlx::query("INSERT INTO procesos_imports (actor_id,file_hash,result_json,created_at) VALUES (?,?,'{}',UTC_TIMESTAMP(6))")
            .bind(actor).bind(hash).execute(&mut *tx).await?.last_insert_id();
        let value = serde_json::to_value(&result).map_err(|_| CoreError::Invalid)?;
        sqlx::query("UPDATE procesos_imports SET result_json=? WHERE id=?")
            .bind(&value)
            .bind(result.import_id)
            .execute(&mut *tx)
            .await?;
        audit(
            &mut tx,
            actor,
            "import",
            &result.import_id.to_string(),
            value,
        )
        .await?;
        tx.commit().await?;
        Ok(result)
    }

    pub async fn review(
        &self,
        actor: u64,
        id: &str,
        expected: u64,
        marked: Option<bool>,
        note: Option<&str>,
    ) -> Result<ReviewRecord, CoreError> {
        if id.len() != 67
            || (marked.is_none() && note.is_none())
            || note.is_some_and(|n| n.chars().count() > 10_000 || n.contains('\0'))
        {
            return Err(CoreError::Invalid);
        }
        let scope = if marked.is_some() { "marcas" } else { "notas" };
        let mut tx = self.begin(actor, scope, "editar").await?;
        if marked.is_some() && note.is_some() {
            self.admin
                .authorize(actor, "procesos", "notas", "editar")
                .await?;
        }
        let exists: Option<String> = sqlx::query_scalar(
            "SELECT proceso_id FROM procesos_rows WHERE proceso_id=? FOR UPDATE",
        )
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?;
        if exists.is_none() {
            return Err(CoreError::NotFound);
        }
        let previous = sqlx::query_as::<_, ReviewRecord>(
            "SELECT marked,note,version FROM procesos_marks WHERE proceso_id=? FOR UPDATE",
        )
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?
        .unwrap_or(ReviewRecord {
            marked: false,
            note: String::new(),
            version: 0,
        });
        if previous.version != expected {
            return Err(CoreError::PreconditionFailed);
        }
        let result = ReviewRecord {
            marked: marked.unwrap_or(previous.marked),
            note: note.map(str::to_owned).unwrap_or(previous.note),
            version: expected.checked_add(1).ok_or(CoreError::Conflict)?,
        };
        sqlx::query("INSERT INTO procesos_marks (proceso_id,marked,note,version,updated_by,updated_at) VALUES (?,?,?,?,?,UTC_TIMESTAMP(6)) ON DUPLICATE KEY UPDATE marked=VALUES(marked),note=VALUES(note),version=VALUES(version),updated_by=VALUES(updated_by),updated_at=VALUES(updated_at)")
            .bind(id).bind(result.marked).bind(&result.note).bind(result.version).bind(actor).execute(&mut *tx).await?;
        audit(&mut tx,actor,"review.update",id,serde_json::json!({"marked":result.marked,"note_changed":note.is_some(),"version":result.version})).await?;
        tx.commit().await?;
        Ok(result)
    }

    pub async fn export(&self, actor: u64, filter: &ProcessFilter) -> Result<String, CoreError> {
        let mut tx = self.begin(actor, "exportacion", "ver").await?;
        let mut query = select_rows(filter, true)?;
        query
            .push(" ORDER BY r.fecha_recepcion DESC,r.proceso_id ASC LIMIT ")
            .push_bind((MAX_ROWS + 1) as u32);
        let statement = query.build_query_as::<ProcessRecord>();
        let mut records = statement.fetch(&mut *tx);
        let mut row_count = 0;
        let mut writer = csv::Writer::from_writer(Vec::new());
        writer
            .write_record(FIELDS.into_iter().chain(["de_interes", "nota"]))
            .map_err(|_| CoreError::Invalid)?;
        while let Some(record) = records.try_next().await? {
            row_count += 1;
            if row_count > MAX_ROWS {
                return Err(CoreError::Conflict);
            }
            let values = FIELDS.map(|field| safe_cell(record.source[field].as_str().unwrap_or("")));
            writer
                .write_record(values.into_iter().chain([
                    if record.marked {
                        "1".into()
                    } else {
                        "0".into()
                    },
                    safe_cell(&record.note),
                ]))
                .map_err(|_| CoreError::Invalid)?;
            if writer.get_ref().len() > 16 * 1024 * 1024 {
                return Err(CoreError::Conflict);
            }
        }
        drop(records);
        let csv = String::from_utf8(writer.into_inner().map_err(|_| CoreError::Invalid)?)
            .map_err(|_| CoreError::Invalid)?;
        if csv.len() > 16 * 1024 * 1024 {
            return Err(CoreError::Conflict);
        }
        audit(
            &mut tx,
            actor,
            "export",
            "csv",
            serde_json::json!({"rows": row_count}),
        )
        .await?;
        tx.commit().await?;
        Ok(format!("\u{feff}{csv}"))
    }
}

async fn save_batch(
    tx: &mut Transaction<'_, MySql>,
    rows: &[(&ScoutRow, String, bool)],
) -> Result<(), CoreError> {
    let mut compare = QueryBuilder::<MySql>::new("SELECT COUNT(*) FROM procesos_rows WHERE ");
    let mut comparisons = 0;
    for (row, source, same_observation) in rows {
        if !same_observation {
            continue;
        }
        if comparisons > 0 {
            compare.push(" OR ");
        }
        compare
            .push("(proceso_id=")
            .push_bind(&row.proceso_id)
            .push(" AND source_json<>CAST(")
            .push_bind(source)
            .push(" AS JSON))");
        comparisons += 1;
    }
    if comparisons > 0 {
        let conflicts: i64 = compare.build_query_scalar().fetch_one(&mut **tx).await?;
        if conflicts > 0 {
            return Err(CoreError::Conflict);
        }
    }
    let mut query = QueryBuilder::<MySql>::new(
        "INSERT INTO procesos_rows (proceso_id,entidad,expediente,fecha_recepcion,ultima_deteccion,puntaje,rubro,resumen_corto,contenido_hash,source_json) ",
    );
    query.push_values(rows, |mut fields, (row, source, _)| {
        fields
            .push_bind(&row.proceso_id)
            .push_bind(&row.entidad)
            .push_bind(&row.expediente)
            .push_bind(row.fecha_recepcion.naive_utc())
            .push_bind(row.ultima_deteccion.naive_utc())
            .push_bind(row.puntaje)
            .push_bind(&row.rubro)
            .push_bind(&row.resumen_corto)
            .push_bind(&row.contenido_hash)
            .push_bind(source);
    });
    query.push(" ON DUPLICATE KEY UPDATE entidad=VALUES(entidad),expediente=VALUES(expediente),fecha_recepcion=VALUES(fecha_recepcion),ultima_deteccion=VALUES(ultima_deteccion),puntaje=VALUES(puntaje),rubro=VALUES(rubro),resumen_corto=VALUES(resumen_corto),contenido_hash=VALUES(contenido_hash),source_json=VALUES(source_json)");
    query.build().execute(&mut **tx).await?;
    Ok(())
}

async fn audit(
    tx: &mut Transaction<'_, MySql>,
    actor: u64,
    action: &str,
    target: &str,
    details: serde_json::Value,
) -> Result<(), CoreError> {
    CoreAdmin::record_domain_event(tx, actor, "procesos", action, target, details).await
}

impl ProcessFilter {
    fn bounds(&self) -> Result<(u32, u32, u64), CoreError> {
        let page = self.page.unwrap_or(1);
        let size = self.page_size.unwrap_or(25);
        if page == 0
            || page > 100_000
            || !(1..=100).contains(&size)
            || self.q.as_ref().is_some_and(|s| s.chars().count() > 200)
            || self
                .entidad
                .as_ref()
                .is_some_and(|s| s.chars().count() > 200)
            || matches!((self.desde,self.hasta), (Some(a),Some(b)) if a>b)
        {
            return Err(CoreError::Invalid);
        }
        Ok((page, size, u64::from(page - 1) * u64::from(size)))
    }
}

fn day_start(date: NaiveDate) -> Result<DateTime<FixedOffset>, CoreError> {
    FixedOffset::west_opt(6 * 3600)
        .ok_or(CoreError::Invalid)?
        .from_local_datetime(&date.and_hms_opt(0, 0, 0).ok_or(CoreError::Invalid)?)
        .single()
        .ok_or(CoreError::Invalid)
}

fn select_rows(
    filter: &ProcessFilter,
    full_source: bool,
) -> Result<QueryBuilder<MySql>, CoreError> {
    filter.bounds()?;
    let source = if full_source {
        "r.source_json"
    } else {
        "JSON_OBJECT()"
    };
    let mut query = QueryBuilder::<MySql>::new(format!(
        "SELECT r.proceso_id,r.entidad,r.expediente,r.fecha_recepcion,r.puntaje,r.rubro,r.resumen_corto,{source} AS source,COALESCE(m.marked,0) AS marked,COALESCE(m.note,'') AS note,CAST(COALESCE(m.version,0) AS UNSIGNED) AS version FROM procesos_rows r LEFT JOIN procesos_marks m ON m.proceso_id=r.proceso_id WHERE 1=1"
    ));
    if let Some(marked) = filter.marked {
        query.push(" AND COALESCE(m.marked,0)=").push_bind(marked);
    }
    if let Some(q) = filter.q.as_deref().filter(|s| !s.trim().is_empty()) {
        let value = format!("%{}%", escape_like(q.trim()));
        query
            .push(" AND (r.expediente LIKE ")
            .push_bind(value.clone())
            .push(" OR r.resumen_corto LIKE ")
            .push_bind(value.clone())
            .push(" OR JSON_UNQUOTE(JSON_EXTRACT(r.source_json,'$.descripcion')) LIKE ")
            .push_bind(value)
            .push(")");
    }
    if let Some(entidad) = filter.entidad.as_deref().filter(|s| !s.trim().is_empty()) {
        query
            .push(" AND r.entidad LIKE ")
            .push_bind(format!("%{}%", escape_like(entidad.trim())));
    }
    if let Some(date) = filter.desde {
        query
            .push(" AND r.fecha_recepcion>=")
            .push_bind(day_start(date)?.naive_utc());
    }
    if let Some(date) = filter.hasta {
        query
            .push(" AND r.fecha_recepcion<")
            .push_bind(day_start(date.succ_opt().ok_or(CoreError::Invalid)?)?.naive_utc());
    }
    Ok(query)
}

fn escape_like(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

fn safe_cell(value: &str) -> String {
    if value.trim_start().starts_with(['=', '+', '-', '@']) || value.starts_with(['\t', '\r', '\n'])
    {
        format!("'{value}")
    } else {
        value.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honduras_date_bounds_do_not_shift_the_local_day() {
        assert_eq!(
            day_start(NaiveDate::from_ymd_opt(2026, 10, 21).unwrap())
                .unwrap()
                .naive_utc()
                .to_string(),
            "2026-10-21 06:00:00"
        );
        assert!(
            ProcessFilter {
                page: Some(0),
                ..Default::default()
            }
            .bounds()
            .is_err()
        );
        assert!(
            ProcessFilter {
                page_size: Some(101),
                ..Default::default()
            }
            .bounds()
            .is_err()
        );
    }
    #[test]
    fn export_neutralizes_formulas_without_changing_ordinary_text() {
        for value in ["=1+1", " +cmd", "\t@SUM(A1)", "-1+2", "\rtext"] {
            assert!(safe_cell(value).starts_with('\''));
        }
        assert_eq!(safe_cell("Un equipo, modelo A"), "Un equipo, modelo A");
        assert_eq!(escape_like("50%_a"), "50\\%\\_a");
    }
}
