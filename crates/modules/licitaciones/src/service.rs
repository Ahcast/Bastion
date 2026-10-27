use crate::*;
use bastion_colaboracion::{CollaborationService, PublishedForm};
use bastion_core::{CoreAdmin, CoreError, Database};
use sha2::{Digest, Sha256};
use sqlx::{MySql, Transaction};

#[derive(Clone)]
pub struct TenderService {
    database: Database,
    admin: CoreAdmin,
    library: CollaborationService,
}
impl TenderService {
    pub fn new(database: Database, admin: CoreAdmin) -> Self {
        Self {
            library: CollaborationService::new(database.clone(), admin.clone()),
            database,
            admin,
        }
    }
    pub async fn migrate(database: &Database) -> Result<(), CoreError> {
        let mut migrator = sqlx::migrate!("./migrations");
        migrator.dangerous_set_table_name("_licitaciones_migrations");
        migrator
            .run(database.pool())
            .await
            .map_err(CoreError::Migration)
    }
    pub async fn synchronize_release(&self, allowed: bool) -> Result<(), CoreError> {
        let exists:i64=sqlx::query_scalar("SELECT COUNT(*) FROM information_schema.tables WHERE table_schema=DATABASE() AND table_name='_licitaciones_migrations'").fetch_one(self.database.pool()).await?;
        let ready = if exists > 0 {
            let applied: Vec<(i64, bool, Vec<u8>)> =
                sqlx::query_as("SELECT version,success,checksum FROM _licitaciones_migrations")
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
        sqlx::query("INSERT INTO module_registry (module_id,display_name,included_in_release,environment_allowed,enabled) VALUES ('licitaciones','Licitaciones',1,?,0) ON DUPLICATE KEY UPDATE included_in_release=1,environment_allowed=VALUES(environment_allowed)")
            .bind(allowed && ready).execute(self.database.pool()).await?;
        Ok(())
    }
    async fn begin(
        &self,
        actor: u64,
        capability: &str,
    ) -> Result<Transaction<'_, MySql>, CoreError> {
        let mut tx = self.database.pool().begin().await?;
        let enabled:Option<bool>=sqlx::query_scalar("SELECT (included_in_release AND environment_allowed AND enabled) FROM module_registry WHERE module_id='licitaciones' FOR UPDATE").fetch_optional(&mut *tx).await?;
        if enabled != Some(true) {
            return Err(CoreError::NotFound);
        }
        self.admin
            .authorize_in_transaction(&mut tx, actor, "licitaciones", "expedientes", capability)
            .await?;
        Ok(tx)
    }
    pub async fn check_access(&self, actor: u64, capability: &str) -> Result<(), CoreError> {
        self.begin(actor, capability).await?.commit().await?;
        Ok(())
    }
    pub async fn consume_request(&self, actor: u64) -> Result<(), CoreError> {
        let mut tx = self.database.pool().begin().await?;
        sqlx::query("INSERT INTO licitaciones_actor_limits (actor_id,window_start,requests) VALUES (?,UTC_TIMESTAMP(6),1) ON DUPLICATE KEY UPDATE requests=IF(window_start<=UTC_TIMESTAMP(6)-INTERVAL 1 MINUTE,1,requests+1),window_start=IF(window_start<=UTC_TIMESTAMP(6)-INTERVAL 1 MINUTE,UTC_TIMESTAMP(6),window_start)")
            .bind(actor).execute(&mut *tx).await?;
        let count: u32 =
            sqlx::query_scalar("SELECT requests FROM licitaciones_actor_limits WHERE actor_id=?")
                .bind(actor)
                .fetch_one(&mut *tx)
                .await?;
        tx.commit().await?;
        if count > 120 {
            return Err(CoreError::Locked);
        }
        Ok(())
    }
    pub async fn list(&self, actor: u64, filter: &PageQuery) -> Result<ExpedientPage, CoreError> {
        let (page, size) = filter.bounds()?;
        let mut tx = self.begin(actor, "ver").await?;
        let mut items:Vec<ExpedientSummary>=sqlx::query_as("SELECT id,name,version,state FROM licitaciones_expedientes WHERE deleted_at IS NULL ORDER BY id DESC LIMIT ? OFFSET ?")
            .bind(size+1).bind((page-1)*size).fetch_all(&mut *tx).await?;
        let has_more = items.len() > size as usize;
        items.truncate(size as usize);
        tx.commit().await?;
        Ok(ExpedientPage {
            items,
            page,
            page_size: size,
            has_more,
        })
    }
    pub async fn detail(&self, actor: u64, id: u64) -> Result<Expedient, CoreError> {
        let mut tx = self.begin(actor, "ver").await?;
        let result = load(&mut tx, id).await?;
        tx.commit().await?;
        Ok(result)
    }
    pub async fn create(&self, actor: u64, key: &str) -> Result<Expedient, CoreError> {
        let key = key_hash(key)?;
        let hash = Sha256::digest(b"create-empty-expedient-v1").to_vec();
        let mut tx = self.begin(actor, "editar").await?;
        if let Some(result) = receipt(&mut tx, actor, &key, &hash).await? {
            tx.commit().await?;
            return Ok(result);
        }
        let id = sqlx::query(
            "INSERT INTO licitaciones_expedientes (created_by,updated_by) VALUES (?,?)",
        )
        .bind(actor)
        .bind(actor)
        .execute(&mut *tx)
        .await?
        .last_insert_id();
        let result = load(&mut tx, id).await?;
        save_receipt(&mut tx, actor, &key, &hash, &result).await?;
        event(&mut tx, actor, "expedient.create", id, 1).await?;
        tx.commit().await?;
        Ok(result)
    }
    pub async fn rename(
        &self,
        actor: u64,
        id: u64,
        expected: u64,
        input: NameInput,
    ) -> Result<Expedient, CoreError> {
        let name = input.normalize()?;
        let mut tx = self.begin(actor, "editar").await?;
        let current = load(&mut tx, id).await?;
        editable(&current, expected)?;
        if current.name == name {
            tx.commit().await?;
            return Ok(current);
        }
        sqlx::query("UPDATE licitaciones_expedientes SET name=? WHERE id=?")
            .bind(name).bind(id).execute(&mut *tx).await?;
        bump(&mut tx, actor, id, expected).await?;
        let result = load(&mut tx, id).await?;
        event(&mut tx, actor, "expedient.rename", id, result.version).await?;
        tx.commit().await?;
        Ok(result)
    }
    pub async fn copy_form(
        &self,
        actor: u64,
        id: u64,
        expected: u64,
        key: &str,
        input: CopyFormInput,
    ) -> Result<Expedient, CoreError> {
        if input.source_id == 0 || input.revision == 0 {
            return Err(CoreError::Invalid);
        }
        let key = key_hash(key)?;
        // Expected version is a precondition, not part of the idempotent command identity.
        let hash = Sha256::digest(
            serde_json::to_vec(&("copy", id, &input)).map_err(|_| CoreError::Invalid)?,
        )
        .to_vec();
        let mut tx = self.begin(actor, "editar").await?;
        let current = load(&mut tx, id).await?;
        if let Some(result) = receipt(&mut tx, actor, &key, &hash).await? {
            tx.commit().await?;
            return Ok(result);
        }
        editable(&current, expected)?;
        if current.forms.len() >= 100 {
            return Err(CoreError::Invalid);
        }
        let source = self
            .library
            .form_snapshot_in_transaction(&mut tx, actor, input.source_id, input.revision)
            .await?;
        sqlx::query("INSERT INTO licitaciones_formularios (expediente_id,source_json,values_json) VALUES (?,?,'{}')")
            .bind(id).bind(serde_json::to_value(source).map_err(|_|CoreError::Invalid)?).execute(&mut *tx).await?;
        bump(&mut tx, actor, id, expected).await?;
        let result = load(&mut tx, id).await?;
        save_receipt(&mut tx, actor, &key, &hash, &result).await?;
        event(&mut tx, actor, "form.copy", id, result.version).await?;
        tx.commit().await?;
        Ok(result)
    }
    pub async fn save_values(
        &self,
        actor: u64,
        id: u64,
        copy_id: u64,
        expected: u64,
        input: ValuesInput,
    ) -> Result<Expedient, CoreError> {
        let mut tx = self.begin(actor, "editar").await?;
        let current = load(&mut tx, id).await?;
        editable(&current, expected)?;
        let copy = current
            .forms
            .iter()
            .find(|c| c.id == copy_id)
            .ok_or(CoreError::NotFound)?;
        copy.source.definition.validate_values(&input.values)?;
        sqlx::query(
            "UPDATE licitaciones_formularios SET values_json=? WHERE id=? AND expediente_id=?",
        )
        .bind(serde_json::to_value(input.values).map_err(|_| CoreError::Invalid)?)
        .bind(copy_id)
        .bind(id)
        .execute(&mut *tx)
        .await?;
        bump(&mut tx, actor, id, expected).await?;
        let result = load(&mut tx, id).await?;
        event(&mut tx, actor, "form.values", id, result.version).await?;
        tx.commit().await?;
        Ok(result)
    }
    pub async fn delete(
        &self,
        actor: u64,
        id: u64,
        expected: u64,
        confirm: u64,
    ) -> Result<(), CoreError> {
        if id != confirm {
            return Err(CoreError::Invalid);
        }
        let mut tx = self.begin(actor, "eliminar").await?;
        let current = load(&mut tx, id).await?;
        editable(&current, expected)?;
        bump(&mut tx, actor, id, expected).await?;
        sqlx::query("UPDATE licitaciones_expedientes SET deleted_at=UTC_TIMESTAMP(6) WHERE id=?")
            .bind(id)
            .execute(&mut *tx)
            .await?;
        event(&mut tx, actor, "expedient.delete", id, expected + 1).await?;
        tx.commit().await?;
        Ok(())
    }
}
fn key_hash(key: &str) -> Result<Vec<u8>, CoreError> {
    if !(16..=128).contains(&key.len())
        || !key
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_'))
    {
        return Err(CoreError::Invalid);
    }
    Ok(Sha256::digest(key.as_bytes()).to_vec())
}
fn editable(e: &Expedient, version: u64) -> Result<(), CoreError> {
    if e.version != version {
        return Err(CoreError::PreconditionFailed);
    }
    if e.state != "borrador" {
        return Err(CoreError::Conflict);
    }
    Ok(())
}
async fn bump(
    tx: &mut Transaction<'_, MySql>,
    actor: u64,
    id: u64,
    expected: u64,
) -> Result<(), CoreError> {
    let next = expected.checked_add(1).ok_or(CoreError::Invalid)?;
    sqlx::query("UPDATE licitaciones_expedientes SET version=?,updated_by=?,updated_at=UTC_TIMESTAMP(6) WHERE id=? AND version=?")
        .bind(next).bind(actor).bind(id).bind(expected).execute(&mut **tx).await?;
    Ok(())
}
async fn load(tx: &mut Transaction<'_, MySql>, id: u64) -> Result<Expedient, CoreError> {
    let e:ExpedientSummary=sqlx::query_as("SELECT id,name,version,state FROM licitaciones_expedientes WHERE id=? AND deleted_at IS NULL FOR UPDATE")
        .bind(id).fetch_optional(&mut **tx).await?.ok_or(CoreError::NotFound)?;
    let rows:Vec<(u64,serde_json::Value,serde_json::Value)>=sqlx::query_as("SELECT id,source_json,values_json FROM licitaciones_formularios WHERE expediente_id=? ORDER BY id")
        .bind(id).fetch_all(&mut **tx).await?;
    let mut forms = Vec::with_capacity(rows.len());
    for (id, source, values) in rows {
        let source: PublishedForm =
            serde_json::from_value(source).map_err(|_| CoreError::Invalid)?;
        if source.definition.hash()? != source.hash {
            return Err(CoreError::Conflict);
        }
        let values = serde_json::from_value(values).map_err(|_| CoreError::Invalid)?;
        let missing_required = source.definition.validate_values(&values)?;
        forms.push(FormCopy {
            id,
            source,
            values,
            missing_required,
        });
    }
    Ok(Expedient {
        id: e.id,
        name: e.name,
        version: e.version,
        state: e.state,
        forms,
    })
}
async fn receipt(
    tx: &mut Transaction<'_, MySql>,
    actor: u64,
    key: &[u8],
    hash: &[u8],
) -> Result<Option<Expedient>, CoreError> {
    let row:Option<(Vec<u8>,u64,serde_json::Value)>=sqlx::query_as("SELECT request_hash,expediente_id,response_json FROM licitaciones_recibos WHERE actor_id=? AND key_hash=?")
        .bind(actor).bind(key).fetch_optional(&mut **tx).await?;
    match row {
        Some((old, id, value)) => {
            if old != hash {
                return Err(CoreError::Conflict);
            }
            // Never resurrect or expose a deleted record through an old receipt.
            let exists: Option<u64> = sqlx::query_scalar(
                "SELECT id FROM licitaciones_expedientes WHERE id=? AND deleted_at IS NULL",
            )
            .bind(id)
            .fetch_optional(&mut **tx)
            .await?;
            if exists.is_none() {
                return Err(CoreError::NotFound);
            }
            Ok(Some(
                serde_json::from_value(value).map_err(|_| CoreError::Invalid)?,
            ))
        }
        None => Ok(None),
    }
}
async fn save_receipt(
    tx: &mut Transaction<'_, MySql>,
    actor: u64,
    key: &[u8],
    hash: &[u8],
    result: &Expedient,
) -> Result<(), CoreError> {
    sqlx::query("INSERT INTO licitaciones_recibos (actor_id,key_hash,request_hash,expediente_id,response_json) VALUES (?,?,?,?,?)")
        .bind(actor).bind(key).bind(hash).bind(result.id).bind(serde_json::to_value(result).map_err(|_|CoreError::Invalid)?).execute(&mut **tx).await?;
    Ok(())
}
async fn event(
    tx: &mut Transaction<'_, MySql>,
    actor: u64,
    action: &str,
    id: u64,
    version: u64,
) -> Result<(), CoreError> {
    CoreAdmin::record_domain_event(
        tx,
        actor,
        "licitaciones",
        action,
        &id.to_string(),
        serde_json::json!({"version":version}),
    )
    .await
}
