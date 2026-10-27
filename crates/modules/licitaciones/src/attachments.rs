use crate::service::{bump, editable, event, key_hash, load};
use crate::{Expedient, MAX_INPUT_BYTES, TenderService, validate_input_budget};
use base64::{Engine, engine::general_purpose::STANDARD};
use bastion_colaboracion::FilePolicy;
use bastion_core::{CoreAdmin, CoreError, DocumentService};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{MySql, Transaction};

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttachmentSection {
    Legal,
    Tecnica,
}
impl AttachmentSection {
    pub fn key(self) -> &'static str {
        match self {
            Self::Legal => "legal",
            Self::Tecnica => "tecnica",
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct Attachment {
    pub id: String,
    pub section: String,
    pub filename: String,
    pub mime: String,
    pub byte_size: u64,
    pub sha256: String,
    pub source_id: Option<u64>,
    pub source_sha256: Option<String>,
    pub created_at: chrono::NaiveDateTime,
}
#[derive(Serialize)]
pub struct AttachmentList {
    pub items: Vec<Attachment>,
    pub expedient_version: u64,
    pub retained_bytes: u64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AttachmentUpload {
    pub section: AttachmentSection,
    pub filename: String,
    pub content_base64: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AttachmentCopy {
    pub section: AttachmentSection,
    pub source_id: u64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AttachmentDelete {
    pub confirm_id: String,
}
#[derive(Serialize)]
pub struct AttachmentContent {
    pub filename: String,
    pub mime: String,
    pub content_base64: String,
}

impl AttachmentUpload {
    pub fn decode(self) -> Result<(AttachmentSection, String, Vec<u8>), CoreError> {
        if self.content_base64.len() > (MAX_INPUT_BYTES as usize).div_ceil(3) * 4 {
            return Err(CoreError::Invalid);
        }
        let bytes = STANDARD
            .decode(self.content_base64)
            .map_err(|_| CoreError::Invalid)?;
        validate_input_budget(0, bytes.len() as u64)?;
        Ok((self.section, filename(&self.filename)?, bytes))
    }
}
fn filename(value: &str) -> Result<String, CoreError> {
    if value
        .chars()
        .any(|c| c.is_control() || matches!(c, '/' | '\\' | ':'))
    {
        return Err(CoreError::Invalid);
    }
    let name = value.trim();
    if name.is_empty() || name.chars().count() > 255 || matches!(name, "." | "..") {
        return Err(CoreError::Invalid);
    }
    Ok(name.into())
}
fn policy() -> FilePolicy {
    FilePolicy {
        allowed_extensions: [
            "pdf", "png", "jpg", "jpeg", "gif", "webp", "bmp", "tif", "tiff",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect(),
        ..FilePolicy::default()
    }
}
fn attachment_id(id: &str) -> Result<(), CoreError> {
    if id.len() != 48
        || !id
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(CoreError::Invalid);
    }
    Ok(())
}
async fn retained(tx: &mut Transaction<'_, MySql>, id: u64) -> Result<u64, CoreError> {
    Ok(sqlx::query_scalar("SELECT CAST(COALESCE((SELECT SUM(byte_size) FROM licitaciones_archivos WHERE expediente_id=?),0)+COALESCE((SELECT SUM(byte_size) FROM licitaciones_archivo_limpieza WHERE expediente_id=?),0) AS UNSIGNED)")
        .bind(id).bind(id).fetch_one(&mut **tx).await?)
}
async fn replay(
    tx: &mut Transaction<'_, MySql>,
    actor: u64,
    key: &[u8],
    hash: &[u8],
) -> Result<bool, CoreError> {
    let old: Option<Vec<u8>> = sqlx::query_scalar(
        "SELECT request_hash FROM licitaciones_archivo_recibos WHERE actor_id=? AND key_hash=?",
    )
    .bind(actor)
    .bind(key)
    .fetch_optional(&mut **tx)
    .await?;
    if old.as_ref().is_some_and(|v| v.as_slice() != hash) {
        return Err(CoreError::Conflict);
    }
    Ok(old.is_some())
}
impl TenderService {
    pub async fn attachments(&self, actor: u64, id: u64) -> Result<AttachmentList, CoreError> {
        let mut tx = self.begin(actor, "ver").await?;
        let expedient = load(&mut tx, id).await?;
        let items=sqlx::query_as("SELECT id,section,filename,mime,byte_size,LOWER(HEX(sha256)) AS sha256,source_id,LOWER(HEX(source_sha256)) AS source_sha256,created_at FROM licitaciones_archivos WHERE expediente_id=? ORDER BY created_at,id LIMIT 1000")
            .bind(id).fetch_all(&mut *tx).await?;
        let retained_bytes = retained(&mut tx, id).await?;
        tx.commit().await?;
        Ok(AttachmentList {
            items,
            expedient_version: expedient.version,
            retained_bytes,
        })
    }
    pub async fn upload_attachment(
        &self,
        actor: u64,
        id: u64,
        expected: u64,
        key: &str,
        input: AttachmentUpload,
    ) -> Result<Expedient, CoreError> {
        self.check_access(actor, "editar").await?;
        let (section, filename, bytes) = input.decode()?;
        let hash = Sha256::digest(
            serde_json::to_vec(&(
                "upload",
                id,
                section,
                &filename,
                Sha256::digest(&bytes).to_vec(),
            ))
            .map_err(|_| CoreError::Invalid)?,
        )
        .to_vec();
        self.store_attachment(
            actor,
            id,
            expected,
            key,
            StoredInput {
                section,
                filename,
                bytes,
                source: None,
                request_hash: hash,
            },
        )
        .await
    }
    pub async fn copy_attachment(
        &self,
        actor: u64,
        id: u64,
        expected: u64,
        key: &str,
        input: AttachmentCopy,
    ) -> Result<Expedient, CoreError> {
        if input.source_id == 0 {
            return Err(CoreError::Invalid);
        }
        let hash = Sha256::digest(
            serde_json::to_vec(&("library", id, input.section, input.source_id))
                .map_err(|_| CoreError::Invalid)?,
        )
        .to_vec();
        let mut tx = self.begin(actor, "editar").await?;
        let current = load(&mut tx, id).await?;
        if replay(&mut tx, actor, &key_hash(key)?, &hash).await? {
            tx.commit().await?;
            return Ok(current);
        }
        editable(&current, expected)?;
        tx.commit().await?;
        let snapshot = self
            .library
            .file_snapshot(actor, input.source_id, MAX_INPUT_BYTES as usize)
            .await?;
        self.store_attachment(
            actor,
            id,
            expected,
            key,
            StoredInput {
                section: input.section,
                filename: snapshot.filename,
                bytes: snapshot.bytes,
                source: Some((snapshot.id, snapshot.sha256)),
                request_hash: hash,
            },
        )
        .await
    }
    async fn store_attachment(
        &self,
        actor: u64,
        id: u64,
        expected: u64,
        key: &str,
        input: StoredInput,
    ) -> Result<Expedient, CoreError> {
        let key = key_hash(key)?;
        let mut tx = self.begin(actor, "editar").await?;
        let current = load(&mut tx, id).await?;
        if replay(&mut tx, actor, &key, &input.request_hash).await? {
            tx.commit().await?;
            return Ok(current);
        }
        editable(&current, expected)?;
        tx.commit().await?;
        let filename = filename(&input.filename)?;
        validate_input_budget(0, input.bytes.len() as u64)?;
        let mime = self.storage.detect_mime(input.bytes.clone()).await?;
        policy().check(&filename, &mime, &input.bytes)?;
        if !self.storage.storage_configured() {
            return Err(CoreError::Overloaded);
        }
        let size = input.bytes.len() as u64;
        let sha = Sha256::digest(&input.bytes).to_vec();
        if input.source.as_ref().is_some_and(|(_, hash)| *hash != sha) {
            return Err(CoreError::Conflict);
        }
        let file_id = DocumentService::object_suffix();
        let object_key = format!("licitaciones/{id}/files/{file_id}");
        let mut tx = self.begin(actor, "editar").await?;
        let current = load(&mut tx, id).await?;
        if replay(&mut tx, actor, &key, &input.request_hash).await? {
            tx.commit().await?;
            return Ok(current);
        }
        editable(&current, expected)?;
        validate_input_budget(retained(&mut tx, id).await?, size)?;
        let count:i64=sqlx::query_scalar("SELECT (SELECT COUNT(*) FROM licitaciones_archivos WHERE expediente_id=?)+(SELECT COUNT(*) FROM licitaciones_archivo_limpieza WHERE expediente_id=?)")
            .bind(id).bind(id).fetch_one(&mut *tx).await?;
        if count >= 1000 {
            return Err(CoreError::Invalid);
        }
        sqlx::query("INSERT INTO licitaciones_archivo_limpieza(object_key,expediente_id,byte_size,state,reason,available_at) VALUES (?,?,?,'reserved','abandoned_upload',UTC_TIMESTAMP(6)+INTERVAL 15 MINUTE)")
            .bind(&object_key).bind(id).bind(size).execute(&mut *tx).await?;
        tx.commit().await?;
        self.storage
            .put_with_limit(object_key.clone(), input.bytes, MAX_INPUT_BYTES as usize)
            .await?;
        let mut tx = self.begin(actor, "editar").await?;
        let current = load(&mut tx, id).await?;
        if replay(&mut tx, actor, &key, &input.request_hash).await? {
            tx.commit().await?;
            return Ok(current);
        }
        editable(&current, expected)?;
        let reserved:Option<String>=sqlx::query_scalar("SELECT state FROM licitaciones_archivo_limpieza WHERE object_key=? AND available_at>UTC_TIMESTAMP(6) FOR UPDATE").bind(&object_key).fetch_optional(&mut *tx).await?;
        if reserved.as_deref() != Some("reserved") {
            return Err(CoreError::Conflict);
        }
        if let Some((source, hash)) = &input.source {
            self.library
                .validate_file_snapshot_in_transaction(&mut tx, actor, *source, hash)
                .await?;
        }
        sqlx::query("INSERT INTO licitaciones_archivos(id,expediente_id,section,filename,mime,byte_size,sha256,source_id,source_sha256,object_key) VALUES (?,?,?,?,?,?,?,?,?,?)")
            .bind(&file_id).bind(id).bind(input.section.key()).bind(filename).bind(mime).bind(size).bind(sha).bind(input.source.as_ref().map(|(id,_)|*id)).bind(input.source.as_ref().map(|(_,hash)|hash)).bind(&object_key).execute(&mut *tx).await?;
        bump(&mut tx, actor, id, expected).await?;
        sqlx::query("INSERT INTO licitaciones_archivo_recibos(actor_id,key_hash,request_hash,expediente_id) VALUES (?,?,?,?)").bind(actor).bind(key).bind(input.request_hash).bind(id).execute(&mut *tx).await?;
        sqlx::query("DELETE FROM licitaciones_archivo_limpieza WHERE object_key=?")
            .bind(object_key)
            .execute(&mut *tx)
            .await?;
        let result = load(&mut tx, id).await?;
        event(&mut tx, actor, "file.attach", id, result.version).await?;
        tx.commit().await?;
        Ok(result)
    }
    pub async fn read_attachment(
        &self,
        actor: u64,
        id: u64,
        file: &str,
        preview: bool,
    ) -> Result<AttachmentContent, CoreError> {
        attachment_id(file)?;
        let mut tx = self.begin(actor, "ver").await?;
        load(&mut tx, id).await?;
        let record:Attachment=sqlx::query_as("SELECT id,section,filename,mime,byte_size,LOWER(HEX(sha256)) AS sha256,source_id,LOWER(HEX(source_sha256)) AS source_sha256,created_at FROM licitaciones_archivos WHERE id=? AND expediente_id=?").bind(file).bind(id).fetch_optional(&mut *tx).await?.ok_or(CoreError::NotFound)?;
        let key: String = sqlx::query_scalar(
            "SELECT object_key FROM licitaciones_archivos WHERE id=? AND expediente_id=?",
        )
        .bind(file)
        .bind(id)
        .fetch_one(&mut *tx)
        .await?;
        tx.commit().await?;
        let bytes = self
            .storage
            .get_with_limit(key.clone(), MAX_INPUT_BYTES as usize)
            .await?;
        if bytes.len() as u64 != record.byte_size
            || format!("{:x}", Sha256::digest(&bytes)) != record.sha256
        {
            return Err(CoreError::Overloaded);
        }
        let mut tx = self.begin(actor, "ver").await?;
        let expedient = load(&mut tx, id).await?;
        let current: Option<String> = sqlx::query_scalar(
            "SELECT object_key FROM licitaciones_archivos WHERE id=? AND expediente_id=?",
        )
        .bind(file)
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?;
        if current.as_ref() != Some(&key) {
            return Err(CoreError::NotFound);
        }
        event(
            &mut tx,
            actor,
            if preview {
                "file.preview"
            } else {
                "file.download"
            },
            id,
            expedient.version,
        )
        .await?;
        tx.commit().await?;
        Ok(AttachmentContent {
            filename: record.filename,
            mime: if preview {
                record.mime
            } else {
                "application/octet-stream".into()
            },
            content_base64: STANDARD.encode(bytes),
        })
    }
    pub async fn remove_attachment(
        &self,
        actor: u64,
        id: u64,
        file: &str,
        expected: u64,
        confirm: &str,
    ) -> Result<Expedient, CoreError> {
        attachment_id(file)?;
        if file != confirm {
            return Err(CoreError::Invalid);
        }
        let mut tx = self.begin(actor, "eliminar").await?;
        let current = load(&mut tx, id).await?;
        editable(&current, expected)?;
        let exists: Option<String> = sqlx::query_scalar(
            "SELECT id FROM licitaciones_archivos WHERE id=? AND expediente_id=?",
        )
        .bind(file)
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?;
        if exists.is_none() {
            return Err(CoreError::NotFound);
        }
        sqlx::query("INSERT INTO licitaciones_archivo_limpieza(object_key,expediente_id,byte_size,state,reason,available_at) SELECT object_key,expediente_id,byte_size,'queued','explicit',UTC_TIMESTAMP(6) FROM licitaciones_archivos WHERE id=? AND expediente_id=?")
            .bind(file).bind(id).execute(&mut *tx).await?;
        sqlx::query("DELETE FROM licitaciones_archivos WHERE id=? AND expediente_id=?")
            .bind(file)
            .bind(id)
            .execute(&mut *tx)
            .await?;
        bump(&mut tx, actor, id, expected).await?;
        let result = load(&mut tx, id).await?;
        event(&mut tx, actor, "file.remove", id, result.version).await?;
        tx.commit().await?;
        Ok(result)
    }
    pub async fn clean_one_attachment(&self) -> Result<bool, CoreError> {
        if !self.storage.storage_configured() {
            return Ok(false);
        }
        let mut tx = self.database.pool().begin().await?;
        sqlx::query(
            "SELECT module_id FROM module_registry WHERE module_id='licitaciones' FOR UPDATE",
        )
        .fetch_optional(&mut *tx)
        .await?;
        let row:Option<(String,u64,String)>=sqlx::query_as("SELECT object_key,expediente_id,reason FROM licitaciones_archivo_limpieza WHERE available_at<=UTC_TIMESTAMP(6) ORDER BY available_at LIMIT 1 FOR UPDATE").fetch_optional(&mut *tx).await?;
        let Some((key, id, reason)) = row else {
            tx.commit().await?;
            return Ok(false);
        };
        let live: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM licitaciones_archivos WHERE object_key=?")
                .bind(&key)
                .fetch_one(&mut *tx)
                .await?;
        if live > 0 {
            sqlx::query("DELETE FROM licitaciones_archivo_limpieza WHERE object_key=?")
                .bind(key)
                .execute(&mut *tx)
                .await?;
            tx.commit().await?;
            return Ok(true);
        }
        sqlx::query("UPDATE licitaciones_archivo_limpieza SET state='cleaning',available_at=UTC_TIMESTAMP(6)+INTERVAL 15 MINUTE WHERE object_key=?").bind(&key).execute(&mut *tx).await?;
        tx.commit().await?;
        self.storage.remove_all_versions(key.clone()).await?;
        let mut tx = self.database.pool().begin().await?;
        sqlx::query(
            "SELECT module_id FROM module_registry WHERE module_id='licitaciones' FOR UPDATE",
        )
        .fetch_optional(&mut *tx)
        .await?;
        sqlx::query(
            "DELETE FROM licitaciones_archivo_limpieza WHERE object_key=? AND state='cleaning'",
        )
        .bind(key)
        .execute(&mut *tx)
        .await?;
        CoreAdmin::record_document_cleanup(&mut tx, "licitaciones", &id.to_string(), &reason)
            .await?;
        tx.commit().await?;
        Ok(true)
    }
}
struct StoredInput {
    section: AttachmentSection,
    filename: String,
    bytes: Vec<u8>,
    source: Option<(u64, Vec<u8>)>,
    request_hash: Vec<u8>,
}
pub(crate) async fn enqueue_expedient_deletion(
    tx: &mut Transaction<'_, MySql>,
    id: u64,
) -> Result<(), CoreError> {
    sqlx::query("INSERT INTO licitaciones_archivo_limpieza(object_key,expediente_id,byte_size,state,reason,available_at) SELECT object_key,expediente_id,byte_size,'queued','explicit',UTC_TIMESTAMP(6) FROM licitaciones_archivos WHERE expediente_id=?").bind(id).execute(&mut **tx).await?;
    sqlx::query("DELETE FROM licitaciones_archivos WHERE expediente_id=?")
        .bind(id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn attachments_reject_paths_active_formats_and_unapproved_sections() {
        for name in ["../a.pdf", "a\\b.png", "a:pdf", "a\npdf", ""] {
            assert!(filename(name).is_err());
        }
        assert!(serde_json::from_str::<AttachmentSection>("\"economica\"").is_err());
        assert!(policy().check("a.svg", "image/svg+xml", b"<svg/>").is_err());
        assert!(
            policy()
                .check("a.jpg", "application/pdf", b"%PDF-1.7")
                .is_err()
        );
        assert!(
            AttachmentUpload {
                section: AttachmentSection::Legal,
                filename: "a.pdf".into(),
                content_base64: STANDARD.encode([])
            }
            .decode()
            .is_err()
        );
        assert!(
            policy()
                .check("a.pdf", "application/pdf", b"%PDF-1.7")
                .is_ok()
        );
    }
}
