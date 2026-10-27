use crate::{
    FileContent, FileFilter, FilePage, FilePolicy, FileRecord, UploadInput, folder, preview_mime,
};
use base64::{Engine, engine::general_purpose::STANDARD};
use bastion_core::{CoreAdmin, CoreError, Database, DocumentService};
use sha2::{Digest, Sha256};
use sqlx::{MySql, QueryBuilder, Row, Transaction};

const FIELDS: &str = "f.id,f.scope,f.id_persona,f.title,f.notes,f.filename,f.size_bytes,f.mime,f.trashed_at,f.purge_after,f.created_at";

#[derive(Clone)]
pub struct CollaborationService {
    pub(crate) database: Database,
    pub(crate) admin: CoreAdmin,
    storage: DocumentService,
}
impl CollaborationService {
    async fn policy_internal(&self) -> Result<FilePolicy, CoreError> {
        let value = self
            .admin
            .load_module_setting("colaboracion", "files.policy")
            .await?;
        let policy = match value {
            Some(value) => serde_json::from_value(value).map_err(|_| CoreError::Invalid)?,
            None => FilePolicy::default(),
        };
        policy.validate()?;
        Ok(policy)
    }
    pub async fn policy(&self, actor: u64) -> Result<FilePolicy, CoreError> {
        self.check_access(actor, "ver").await?;
        self.policy_internal().await
    }
    pub async fn save_policy(&self, actor: u64, policy: FilePolicy) -> Result<(), CoreError> {
        policy.validate()?;
        let mut tx = self.begin().await?;
        self.admin
            .save_module_setting(
                &mut tx,
                actor,
                "colaboracion",
                "files.policy",
                serde_json::to_value(policy).map_err(|_| CoreError::Invalid)?,
            )
            .await?;
        tx.commit().await?;
        Ok(())
    }
    pub fn with_storage(database: Database, admin: CoreAdmin, storage: DocumentService) -> Self {
        Self {
            database,
            admin,
            storage,
        }
    }
    pub fn new(database: Database, admin: CoreAdmin) -> Self {
        Self {
            database,
            admin,
            storage: DocumentService::from_environment(),
        }
    }
    pub async fn migrate(database: &Database) -> Result<(), CoreError> {
        let mut migrator = sqlx::migrate!("./migrations");
        migrator.dangerous_set_table_name("_colaboracion_migrations");
        migrator
            .run(database.pool())
            .await
            .map_err(CoreError::Migration)
    }
    pub async fn synchronize_release(&self, allowed: bool) -> Result<bool, CoreError> {
        let exists: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM information_schema.tables WHERE table_schema=DATABASE() AND table_name='_colaboracion_migrations'").fetch_one(self.database.pool()).await?;
        let ready = if exists > 0 {
            let applied = sqlx::query_as::<_, (i64, bool, Vec<u8>)>(
                "SELECT version,success,checksum FROM _colaboracion_migrations",
            )
            .fetch_all(self.database.pool())
            .await?;
            let expected = sqlx::migrate!("./migrations");
            applied.len() == expected.iter().count()
                && expected.iter().all(|m| {
                    applied.iter().any(|(v, ok, hash)| {
                        *v == m.version && *ok && hash.as_slice() == m.checksum.as_ref()
                    })
                })
        } else {
            false
        };
        sqlx::query("INSERT INTO module_registry (module_id,display_name,included_in_release,environment_allowed,enabled) VALUES ('colaboracion','Colaboración y archivos',1,?,0) ON DUPLICATE KEY UPDATE included_in_release=1,environment_allowed=VALUES(environment_allowed)")
            .bind(allowed && ready).execute(self.database.pool()).await?;
        Ok(ready)
    }
    pub(crate) async fn begin(&self) -> Result<Transaction<'_, MySql>, CoreError> {
        let mut tx = self.database.pool().begin().await?;
        let enabled: Option<bool> = sqlx::query_scalar("SELECT (included_in_release AND environment_allowed AND enabled) FROM module_registry WHERE module_id='colaboracion' FOR UPDATE").fetch_optional(&mut *tx).await?;
        if enabled != Some(true) {
            return Err(CoreError::NotFound);
        }
        Ok(tx)
    }
    pub async fn check_access(&self, actor: u64, capability: &str) -> Result<(), CoreError> {
        self.begin().await?.commit().await?;
        self.admin
            .allowed_module_scopes(actor, "colaboracion", capability)
            .await?;
        Ok(())
    }
    pub async fn consume_request(&self, actor: u64) -> Result<(), CoreError> {
        let mut tx = self.database.pool().begin().await?;
        sqlx::query("INSERT INTO colaboracion_actor_limits (actor_id,window_start,requests) VALUES (?,UTC_TIMESTAMP(6),1) ON DUPLICATE KEY UPDATE requests=IF(window_start<=UTC_TIMESTAMP(6)-INTERVAL 1 MINUTE,1,requests+1),window_start=IF(window_start<=UTC_TIMESTAMP(6)-INTERVAL 1 MINUTE,UTC_TIMESTAMP(6),window_start)").bind(actor).execute(&mut *tx).await?;
        let count: u32 =
            sqlx::query_scalar("SELECT requests FROM colaboracion_actor_limits WHERE actor_id=?")
                .bind(actor)
                .fetch_one(&mut *tx)
                .await?;
        tx.commit().await?;
        if count > 120 {
            return Err(CoreError::Locked);
        }
        Ok(())
    }
    pub async fn list(&self, actor: u64, filter: &FileFilter) -> Result<FilePage, CoreError> {
        self.check_access(actor, "ver").await?;
        let (page, size) = filter.bounds()?;
        if let Some(scope) = &filter.scope {
            if scope == "empresa" || filter.id_persona.is_some() {
                self.admin
                    .authorize(
                        actor,
                        "colaboracion",
                        &folder(scope, filter.id_persona)?,
                        "ver",
                    )
                    .await?;
            }
        }
        let scopes = self
            .admin
            .allowed_module_scopes(actor, "colaboracion", "ver")
            .await?;
        let mut tx = self.begin().await?;
        let mut query = QueryBuilder::<MySql>::new(format!(
            "SELECT {FIELDS} FROM colaboracion_files f WHERE f.state= "
        ));
        query
            .push_bind(if filter.trash { "trash" } else { "ready" })
            .push(" AND ");
        if scopes.iter().any(|scope| scope == "*") {
            query.push("1=1");
        } else {
            query.push("f.folder_key IN (");
            let mut values = query.separated(",");
            for scope in scopes {
                values.push_bind(scope);
            }
            values.push_unseparated(")");
        }
        if let Some(scope) = &filter.scope {
            query.push(" AND f.scope=").push_bind(scope);
        }
        if let Some(person) = filter.id_persona {
            query.push(" AND f.id_persona=").push_bind(person);
        }
        if let Some(search) = filter
            .search
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            // LOCATE is a literal search: '%' and '_' are never wildcard instructions.
            query
                .push(" AND (LOCATE(")
                .push_bind(search)
                .push(",f.title)>0 OR LOCATE(")
                .push_bind(search)
                .push(",f.filename)>0 OR LOCATE(")
                .push_bind(search)
                .push(",f.notes)>0)");
        }
        query
            .push(" ORDER BY f.id DESC LIMIT ")
            .push_bind(size + 1)
            .push(" OFFSET ")
            .push_bind(u64::from(page - 1) * u64::from(size));
        let mut items: Vec<FileRecord> = query.build_query_as().fetch_all(&mut *tx).await?;
        let has_more = items.len() > size as usize;
        items.truncate(size as usize);
        tx.commit().await?;
        Ok(FilePage {
            items,
            page,
            page_size: size,
            has_more,
        })
    }
    pub async fn upload(
        &self,
        actor: u64,
        key: &str,
        mut input: UploadInput,
    ) -> Result<FileRecord, CoreError> {
        if !(16..=100).contains(&key.len())
            || !key
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
        {
            return Err(CoreError::Invalid);
        }
        let policy = self.policy_internal().await?;
        let bytes = input.validate(policy.max_bytes())?;
        let folder_key = folder(&input.scope, input.id_persona)?;
        self.admin
            .authorize(actor, "colaboracion", &folder_key, "editar")
            .await?;
        let mime = self.storage.detect_mime(bytes.clone()).await?;
        policy.check(&input.filename, &mime, &bytes)?;
        let request_hash =
            Sha256::digest(serde_json::to_vec(&input).map_err(|_| CoreError::Invalid)?).to_vec();
        let content_hash = Sha256::digest(&bytes).to_vec();
        if let Some(person) = input.id_persona {
            self.admin.require_person_record(person).await?;
        }
        let mut tx = self.begin().await?;
        let previous = sqlx::query("SELECT id,object_key,request_hash,state FROM colaboracion_files WHERE actor_id=? AND idempotency_key=? FOR UPDATE").bind(actor).bind(key).fetch_optional(&mut *tx).await?;
        if let Some(row) = previous {
            if row.try_get::<Vec<u8>, _>("request_hash")? != request_hash {
                return Err(CoreError::Conflict);
            }
            match row.try_get::<String, _>("state")?.as_str() {
                "ready" => {
                    let record = load(&mut tx, row.try_get("id")?).await?;
                    tx.commit().await?;
                    return Ok(record);
                }
                "retry" => {
                    let id = row.try_get("id")?;
                    let object_key = row.try_get("object_key")?;
                    sqlx::query(
                        "UPDATE colaboracion_files SET state='uploading',updated_at=UTC_TIMESTAMP(6) WHERE id=?",
                    )
                    .bind(id)
                    .execute(&mut *tx)
                    .await?;
                    tx.commit().await?;
                    return self
                        .finish_upload(actor, id, &folder_key, object_key, bytes)
                        .await;
                }
                _ => return Err(CoreError::Conflict),
            }
        }
        if !self.storage.storage_configured() {
            return Err(CoreError::Overloaded);
        }
        let prefix = match input.id_persona {
            Some(id) => format!("colaboracion/personas/{id}"),
            None => "colaboracion/empresa".into(),
        };
        let object_key = format!("{prefix}/{}", DocumentService::object_suffix());
        let id = sqlx::query("INSERT INTO colaboracion_files (scope,id_persona,folder_key,title,notes,filename,size_bytes,mime,object_key,content_hash,request_hash,actor_id,idempotency_key,state) VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,'uploading')")
            .bind(&input.scope).bind(input.id_persona).bind(&folder_key).bind(&input.title).bind(&input.notes).bind(&input.filename).bind(bytes.len() as u64).bind(mime).bind(&object_key).bind(content_hash).bind(request_hash).bind(actor).bind(key).execute(&mut *tx).await?.last_insert_id();
        // Reservation precedes S3: process crashes leave a durable cleanup record.
        tx.commit().await?;
        self.finish_upload(actor, id, &folder_key, object_key, bytes)
            .await
    }
    async fn finish_upload(
        &self,
        actor: u64,
        id: u64,
        folder_key: &str,
        object_key: String,
        bytes: Vec<u8>,
    ) -> Result<FileRecord, CoreError> {
        if let Err(error) = self
            .storage
            .put_with_limit(object_key, bytes, bastion_core::MAX_PRIVATE_FILE_BYTES)
            .await
        {
            sqlx::query(
                "UPDATE colaboracion_files SET state='retry' WHERE id=? AND state='uploading'",
            )
            .bind(id)
            .execute(self.database.pool())
            .await?;
            return Err(error);
        }
        self.admin
            .authorize(actor, "colaboracion", folder_key, "editar")
            .await?;
        let mut tx = self.begin().await?;
        let result = sqlx::query(
            "UPDATE colaboracion_files SET state='ready' WHERE id=? AND state='uploading'",
        )
        .bind(id)
        .execute(&mut *tx)
        .await?;
        if result.rows_affected() == 1 {
            audit(&mut tx, Some(actor), "file.upload", id).await?;
        }
        let record = load(&mut tx, id).await?;
        tx.commit().await?;
        Ok(record)
    }
    pub async fn read(&self, actor: u64, id: u64, preview: bool) -> Result<FileContent, CoreError> {
        let mut tx = self.begin().await?;
        let record = load(&mut tx, id).await?;
        self.authorize_record(&mut tx, actor, &record, "ver")
            .await?;
        let row = sqlx::query("SELECT object_key,content_hash FROM colaboracion_files WHERE id=?")
            .bind(id)
            .fetch_one(&mut *tx)
            .await?;
        let object_key: String = row.try_get("object_key")?;
        let hash: Vec<u8> = row.try_get("content_hash")?;
        tx.commit().await?;
        let bytes = self
            .storage
            .get_with_limit(object_key, bastion_core::MAX_PRIVATE_FILE_BYTES)
            .await?;
        if Sha256::digest(&bytes).as_slice() != hash.as_slice() {
            return Err(CoreError::Overloaded);
        }
        // Recheck after object I/O so disable, delete and permission changes deny delivery.
        let mut tx = self.begin().await?;
        let current = load(&mut tx, id).await?;
        self.authorize_record(&mut tx, actor, &current, "ver")
            .await?;
        let mime = if preview {
            preview_mime(&bytes).ok_or(CoreError::Invalid)?
        } else {
            "application/octet-stream"
        };
        audit(
            &mut tx,
            Some(actor),
            if preview {
                "file.preview"
            } else {
                "file.download"
            },
            id,
        )
        .await?;
        tx.commit().await?;
        Ok(FileContent {
            filename: record.filename,
            mime,
            content_base64: STANDARD.encode(bytes),
        })
    }
    async fn authorize_record(
        &self,
        tx: &mut Transaction<'_, MySql>,
        actor: u64,
        record: &FileRecord,
        capability: &str,
    ) -> Result<(), CoreError> {
        self.admin
            .authorize_in_transaction(
                tx,
                actor,
                "colaboracion",
                &folder(&record.scope, record.id_persona)?,
                capability,
            )
            .await
            .map_err(|error| match error {
                CoreError::Forbidden => CoreError::NotFound,
                other => other,
            })
    }
    pub async fn delete(&self, actor: u64, id: u64, confirm: u64) -> Result<(), CoreError> {
        if confirm != id || id == 0 {
            return Err(CoreError::Invalid);
        }
        let policy = self.policy_internal().await?;
        let mut tx = self.begin().await?;
        let record = load(&mut tx, id).await?;
        self.authorize_record(&mut tx, actor, &record, "eliminar")
            .await?;
        sqlx::query("UPDATE colaboracion_files SET state='trash',trashed_at=UTC_TIMESTAMP(6),purge_after=DATE_ADD(UTC_TIMESTAMP(6), INTERVAL ? DAY) WHERE id=?")
            .bind(policy.trash_retention_days)
            .bind(id)
            .execute(&mut *tx)
            .await?;
        audit(&mut tx, Some(actor), "file.trash", id).await?;
        tx.commit().await?;
        Ok(())
    }
    pub async fn restore(&self, actor: u64, id: u64) -> Result<(), CoreError> {
        let mut tx = self.begin().await?;
        let record = load_state(&mut tx, id, "trash").await?;
        self.authorize_record(&mut tx, actor, &record, "eliminar")
            .await?;
        let result=sqlx::query("UPDATE colaboracion_files SET state='ready',trashed_at=NULL,purge_after=NULL WHERE id=? AND purge_after>UTC_TIMESTAMP(6)").bind(id).execute(&mut *tx).await?;
        if result.rows_affected() != 1 {
            return Err(CoreError::Conflict);
        }
        audit(&mut tx, Some(actor), "file.restore", id).await?;
        tx.commit().await?;
        Ok(())
    }
    pub async fn purge(&self, actor: u64, id: u64, confirm: u64) -> Result<(), CoreError> {
        if id == 0 || confirm != id {
            return Err(CoreError::Invalid);
        }
        let mut tx = self.begin().await?;
        let record = load_state(&mut tx, id, "trash").await?;
        self.authorize_record(&mut tx, actor, &record, "eliminar")
            .await?;
        sqlx::query("UPDATE colaboracion_files SET state='deleting' WHERE id=?")
            .bind(id)
            .execute(&mut *tx)
            .await?;
        audit(&mut tx, Some(actor), "file.purge", id).await?;
        tx.commit().await?;
        Ok(())
    }
    /// Technical recovery only: never expires ready business records. Continues
    /// when disabled, since deletion was already authorized and committed.
    pub async fn clean_one(&self) -> Result<bool, CoreError> {
        if !self.storage.storage_configured() {
            return Ok(false);
        }
        let mut tx = self.database.pool().begin().await?;
        let row=sqlx::query("SELECT id,object_key,state FROM colaboracion_files WHERE state='deleting' OR (state='trash' AND purge_after<=UTC_TIMESTAMP(6)) OR (state IN ('uploading','retry') AND updated_at<UTC_TIMESTAMP(6)-INTERVAL 1 HOUR) ORDER BY updated_at,id LIMIT 1 FOR UPDATE").fetch_optional(&mut *tx).await?;
        let Some(row) = row else {
            return Ok(false);
        };
        let id: u64 = row.try_get("id")?;
        self.storage
            .remove_all_versions(row.try_get("object_key")?)
            .await?;
        // Keep only a receipt/tombstone for replay protection, no business text.
        sqlx::query("UPDATE colaboracion_files SET state='deleted',title='',notes='',filename='',id_persona=NULL WHERE id=?").bind(id).execute(&mut *tx).await?;
        let state: String = row.try_get("state")?;
        let reason = match state.as_str() {
            "trash" => "retention",
            "deleting" => "explicit",
            _ => "abandoned_upload",
        };
        CoreAdmin::record_document_cleanup(&mut tx, "colaboracion", &id.to_string(), reason)
            .await?;
        tx.commit().await?;
        Ok(true)
    }
}
async fn load(tx: &mut Transaction<'_, MySql>, id: u64) -> Result<FileRecord, CoreError> {
    load_state(tx, id, "ready").await
}
async fn load_state(
    tx: &mut Transaction<'_, MySql>,
    id: u64,
    state: &str,
) -> Result<FileRecord, CoreError> {
    sqlx::query_as("SELECT f.id,f.scope,f.id_persona,f.title,f.notes,f.filename,f.size_bytes,f.mime,f.trashed_at,f.purge_after,f.created_at FROM colaboracion_files f WHERE f.id=? AND f.state=?").bind(id).bind(state).fetch_optional(&mut **tx).await?.ok_or(CoreError::NotFound)
}
async fn audit(
    tx: &mut Transaction<'_, MySql>,
    actor: Option<u64>,
    action: &str,
    id: u64,
) -> Result<(), CoreError> {
    match actor {
        Some(actor) => {
            CoreAdmin::record_domain_event(
                tx,
                actor,
                "colaboracion",
                action,
                &id.to_string(),
                serde_json::json!({}),
            )
            .await
        }
        None => Err(CoreError::Invalid),
    }
}
