use super::*;
use base64::{Engine, engine::general_purpose::STANDARD};
use bastion_core::DocumentService;
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};

pub const MAX_ATTACHMENT_BYTES: usize = 20 * 1024 * 1024;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QuoteAttachmentInput {
    pub filename: String,
    #[serde(default)]
    pub title: String,
    pub content_base64: String,
}
impl QuoteAttachmentInput {
    pub fn validate(self) -> Result<(String, String, Vec<u8>), CoreError> {
        let filename = self.filename.trim().to_owned();
        let title = if self.title.trim().is_empty() {
            filename.clone()
        } else {
            self.title.trim().to_owned()
        };
        if filename.is_empty()
            || filename.chars().count() > 255
            || filename.contains(['/', '\\'])
            || filename.chars().any(char::is_control)
            || title.chars().count() > 255
            || title.chars().any(char::is_control)
            || self.content_base64.len() > MAX_ATTACHMENT_BYTES.div_ceil(3) * 4
        {
            return Err(CoreError::Invalid);
        }
        let bytes = STANDARD
            .decode(&self.content_base64)
            .map_err(|_| CoreError::Invalid)?;
        if bytes.is_empty() || bytes.len() > MAX_ATTACHMENT_BYTES {
            return Err(CoreError::Invalid);
        }
        Ok((filename, title, bytes))
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct QuoteAttachment {
    pub id: String,
    pub filename: String,
    pub title: String,
    pub mime: String,
    pub byte_size: u32,
    pub created_at: NaiveDateTime,
}
#[derive(Serialize)]
pub struct QuoteAttachmentPage {
    pub items: Vec<QuoteAttachment>,
    pub page: u32,
    pub has_more: bool,
    pub quote_version: u64,
}
#[derive(Serialize, Deserialize)]
pub struct QuoteAttachmentWrite {
    pub attachment: QuoteAttachment,
    pub quote_version: u64,
}
const FIELDS: &str = "attachment_id AS id,filename,title,mime,byte_size,created_at";

impl QuoteService {
    pub async fn attachments(
        &self,
        actor: u64,
        id: u64,
        page: u32,
    ) -> Result<QuoteAttachmentPage, CoreError> {
        if !(1..=10000).contains(&page) {
            return Err(CoreError::Invalid);
        }
        let mut tx = self.begin(actor, "ver").await?;
        let quote = load(&mut tx, id).await?;
        let mut items: Vec<QuoteAttachment> = sqlx::query_as(&format!("SELECT {FIELDS} FROM cotizaciones_documents WHERE quote_id=? AND kind='attachment' ORDER BY created_at DESC,attachment_id DESC LIMIT 26 OFFSET ?"))
            .bind(id).bind((page - 1) * 25).fetch_all(&mut *tx).await?;
        let has_more = items.len() > 25;
        items.truncate(25);
        tx.commit().await?;
        Ok(QuoteAttachmentPage {
            items,
            page,
            has_more,
            quote_version: quote.version,
        })
    }

    pub async fn upload_attachment(
        &self,
        actor: u64,
        id: u64,
        expected: u64,
        key: &str,
        input: QuoteAttachmentInput,
    ) -> Result<QuoteAttachmentWrite, CoreError> {
        if !(16..=128).contains(&key.len())
            || !key
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
        {
            return Err(CoreError::Invalid);
        }
        // Authorize before decoding a potentially large body or touching storage.
        self.check_access(actor, "editar").await?;
        let (filename, title, bytes) = input.validate()?;
        let sha = Sha256::digest(&bytes).to_vec();
        let request_hash = Sha256::digest(
            serde_json::to_vec(&(id, &filename, &title, &sha)).map_err(|_| CoreError::Invalid)?,
        )
        .to_vec();
        let key_hash = Sha256::digest(key.as_bytes()).to_vec();
        let mut tx = self.begin(actor, "editar").await?;
        let current = load(&mut tx, id).await?;
        if let Some(result) = attachment_receipt(&mut tx, actor, &key_hash, &request_hash).await? {
            tx.commit().await?;
            return Ok(result);
        }
        if current.version != expected {
            return Err(CoreError::PreconditionFailed);
        }
        let storage = DocumentService::from_environment();
        if !storage.configured() {
            return Err(CoreError::Overloaded);
        }
        let attachment_id = DocumentService::object_suffix();
        let object_key = format!("cotizaciones/{id}/attachments/{attachment_id}");
        // Persist cleanup BEFORE cloud IO, including process cancellation/crashes.
        sqlx::query("INSERT INTO cotizaciones_document_deletions (object_key,available_at) VALUES (?,UTC_TIMESTAMP(6)+INTERVAL 5 MINUTE)")
            .bind(&object_key).execute(&mut *tx).await?;
        tx.commit().await?;
        let byte_size = bytes.len() as u32;
        let mime = if bytes.starts_with(b"%PDF-") {
            "application/pdf"
        } else {
            "application/octet-stream"
        };
        storage.put(object_key.clone(), bytes).await?;
        let mut tx = self.begin(actor, "editar").await?;
        let current = load(&mut tx, id).await?;
        // A concurrent request owns a different object key; its loser remains queued for cleanup.
        if let Some(result) = attachment_receipt(&mut tx, actor, &key_hash, &request_hash).await? {
            tx.commit().await?;
            return Ok(result);
        }
        if current.version != expected {
            return Err(CoreError::PreconditionFailed);
        }
        let version = expected.checked_add(1).ok_or(CoreError::Invalid)?;
        sqlx::query("INSERT INTO cotizaciones_documents (object_key,quote_id,quote_version,kind,filename,mime,sha256,byte_size,created_at,attachment_id,title) VALUES (?,?,?,'attachment',?,?,?,?,UTC_TIMESTAMP(6),?,?)")
            .bind(&object_key).bind(id).bind(version).bind(&filename).bind(mime).bind(&sha).bind(byte_size).bind(&attachment_id).bind(&title).execute(&mut *tx).await?;
        bump_attachment_revision(&mut tx, actor, id, expected, version).await?;
        let attachment = sqlx::query_as(&format!(
            "SELECT {FIELDS} FROM cotizaciones_documents WHERE attachment_id=?"
        ))
        .bind(&attachment_id)
        .fetch_one(&mut *tx)
        .await?;
        let result = QuoteAttachmentWrite {
            attachment,
            quote_version: version,
        };
        sqlx::query("INSERT INTO cotizaciones_attachment_receipts (actor_id,key_hash,request_hash,quote_id,response_json) VALUES (?,?,?,?,?)")
            .bind(actor).bind(&key_hash).bind(&request_hash).bind(id).bind(serde_json::to_value(&result).map_err(|_|CoreError::Invalid)?).execute(&mut *tx).await?;
        sqlx::query("DELETE FROM cotizaciones_document_deletions WHERE object_key=?")
            .bind(&object_key)
            .execute(&mut *tx)
            .await?;
        CoreAdmin::record_domain_event(&mut tx,actor,"cotizaciones","quote.attachment.add",&id.to_string(),serde_json::json!({"attachment_id":attachment_id,"version":version,"byte_size":byte_size})).await?;
        tx.commit().await?;
        Ok(result)
    }

    pub async fn attachment_content(
        &self,
        actor: u64,
        id: u64,
        attachment: &str,
    ) -> Result<QuoteDocument, CoreError> {
        validate_id(attachment)?;
        let mut tx = self.begin(actor, "ver").await?;
        load(&mut tx, id).await?;
        let row: Option<(String,String,String,Vec<u8>,u32)> = sqlx::query_as("SELECT object_key,filename,mime,sha256,byte_size FROM cotizaciones_documents WHERE quote_id=? AND attachment_id=? AND kind='attachment'")
            .bind(id).bind(attachment).fetch_optional(&mut *tx).await?;
        let (object_key, filename, mime, hash, size) = row.ok_or(CoreError::NotFound)?;
        tx.commit().await?;
        let storage = DocumentService::from_environment();
        if !storage.configured() {
            return Err(CoreError::Overloaded);
        }
        let bytes = storage.get(object_key.clone()).await?;
        if bytes.len() != size as usize || Sha256::digest(&bytes).as_slice() != hash.as_slice() {
            return Err(CoreError::Overloaded);
        }
        let mut tx = self.begin(actor, "ver").await?;
        let exists:i64=sqlx::query_scalar("SELECT COUNT(*) FROM cotizaciones_documents WHERE quote_id=? AND attachment_id=? AND kind='attachment'").bind(id).bind(attachment).fetch_one(&mut *tx).await?;
        if exists != 1 {
            return Err(CoreError::NotFound);
        }
        tx.commit().await?;
        Ok(QuoteDocument {
            filename,
            mime,
            content_base64: STANDARD.encode(bytes),
            object_key,
        })
    }

    pub async fn remove_attachment(
        &self,
        actor: u64,
        id: u64,
        attachment: &str,
        expected: u64,
    ) -> Result<u64, CoreError> {
        validate_id(attachment)?;
        let mut tx = self.begin(actor, "editar").await?;
        let current = load(&mut tx, id).await?;
        if current.version != expected {
            return Err(CoreError::PreconditionFailed);
        }
        let key:Option<String>=sqlx::query_scalar("SELECT object_key FROM cotizaciones_documents WHERE quote_id=? AND attachment_id=? AND kind='attachment'").bind(id).bind(attachment).fetch_optional(&mut *tx).await?;
        let key = key.ok_or(CoreError::NotFound)?;
        let version = expected.checked_add(1).ok_or(CoreError::Invalid)?;
        sqlx::query("INSERT INTO cotizaciones_document_deletions (object_key,available_at) VALUES (?,UTC_TIMESTAMP(6)) ON DUPLICATE KEY UPDATE available_at=VALUES(available_at)").bind(&key).execute(&mut *tx).await?;
        sqlx::query("DELETE FROM cotizaciones_documents WHERE object_key=?")
            .bind(&key)
            .execute(&mut *tx)
            .await?;
        bump_attachment_revision(&mut tx, actor, id, expected, version).await?;
        CoreAdmin::record_domain_event(
            &mut tx,
            actor,
            "cotizaciones",
            "quote.attachment.remove",
            &id.to_string(),
            serde_json::json!({"attachment_id":attachment,"version":version}),
        )
        .await?;
        tx.commit().await?;
        Ok(version)
    }
}
async fn attachment_receipt(
    tx: &mut Transaction<'_, MySql>,
    actor: u64,
    key: &[u8],
    request: &[u8],
) -> Result<Option<QuoteAttachmentWrite>, CoreError> {
    let previous:Option<(Vec<u8>,serde_json::Value)>=sqlx::query_as("SELECT request_hash,response_json FROM cotizaciones_attachment_receipts WHERE actor_id=? AND key_hash=?")
        .bind(actor).bind(key).fetch_optional(&mut **tx).await?;
    if let Some((hash, response)) = previous {
        if hash != request {
            return Err(CoreError::Conflict);
        }
        let result: QuoteAttachmentWrite =
            serde_json::from_value(response).map_err(|_| CoreError::Invalid)?;
        let exists:i64=sqlx::query_scalar("SELECT COUNT(*) FROM cotizaciones_documents WHERE attachment_id=? AND kind='attachment'")
            .bind(&result.attachment.id).fetch_one(&mut **tx).await?;
        if exists != 1 {
            return Err(CoreError::NotFound);
        }
        return Ok(Some(result));
    }
    Ok(None)
}
async fn bump_attachment_revision(
    tx: &mut Transaction<'_, MySql>,
    actor: u64,
    id: u64,
    expected: u64,
    next: u64,
) -> Result<(), CoreError> {
    let result=sqlx::query("UPDATE cotizaciones_drafts SET version=?,updated_by=?,updated_at=UTC_TIMESTAMP(6) WHERE id=? AND version=?")
        .bind(next).bind(actor).bind(id).bind(expected).execute(&mut **tx).await?;
    if result.rows_affected() != 1 {
        return Err(CoreError::PreconditionFailed);
    }
    Ok(())
}
fn validate_id(id: &str) -> Result<(), CoreError> {
    if id.len() != 48
        || !id
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
    {
        return Err(CoreError::Invalid);
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn attachment_names_size_and_bytes_are_bounded_without_trusting_extensions() {
        let input = |name: &str, bytes: &[u8]| QuoteAttachmentInput {
            filename: name.into(),
            title: String::new(),
            content_base64: STANDARD.encode(bytes),
        };
        assert!(input("documento.pdf", b"binary").validate().is_ok());
        for name in ["", "../a", "x/y", "x\\y", "x\r\nInjected"] {
            assert!(input(name, b"x").validate().is_err());
        }
        assert!(input("empty", b"").validate().is_err());
        assert!(
            QuoteAttachmentInput {
                filename: "x".into(),
                title: String::new(),
                content_base64: "invalid!".into()
            }
            .validate()
            .is_err()
        );
        assert!(validate_id(&"a".repeat(48)).is_ok());
        assert!(validate_id("../a").is_err());
    }
}
