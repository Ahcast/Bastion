use super::*;
use base64::{Engine, engine::general_purpose::STANDARD};
use bastion_core::DocumentService;
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
pub const MAX_DOCUMENT_BYTES: usize = 10 * 1024 * 1024;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OrderDocumentInput {
    pub filename: String,
    pub content_base64: String,
}
impl OrderDocumentInput {
    fn validate(self) -> Result<(String, Vec<u8>), CoreError> {
        let filename = self.filename.trim().to_owned();
        if self.filename.chars().any(char::is_control)
            || filename.is_empty()
            || filename.chars().count() > 255
            || filename.chars().any(|c| c == '/' || u32::from(c) == 92)
            || filename.chars().any(char::is_control)
            || self.content_base64.len() > MAX_DOCUMENT_BYTES.div_ceil(3) * 4
        {
            return Err(CoreError::Invalid);
        }
        let bytes = STANDARD
            .decode(self.content_base64)
            .map_err(|_| CoreError::Invalid)?;
        if bytes.is_empty() || bytes.len() > MAX_DOCUMENT_BYTES {
            return Err(CoreError::Invalid);
        }
        Ok((filename, bytes))
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct OrderDocument {
    pub id: String,
    pub line_id: String,
    pub filename: String,
    pub mime: String,
    pub byte_size: u32,
    pub created_at: NaiveDateTime,
}
#[derive(Serialize)]
pub struct OrderDocuments {
    pub items: Vec<OrderDocument>,
    pub order_version: u64,
}
#[derive(Serialize, Deserialize)]
pub struct OrderDocumentWrite {
    pub attachment: OrderDocument,
    pub order_version: u64,
}
#[derive(Serialize)]
pub struct OrderDocumentContent {
    pub filename: String,
    pub mime: String,
    pub content_base64: String,
}
fn validate_id(id: &str) -> Result<(), CoreError> {
    if id.len() != 48
        || !id
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(CoreError::Invalid);
    }
    Ok(())
}
impl OrderService {
    pub async fn documents(&self, actor: u64, id: u64) -> Result<OrderDocuments, CoreError> {
        let mut tx = self.begin(actor, "documentos", "ver").await?;
        let order = load(&mut tx, id).await?;
        let items=sqlx::query_as("SELECT attachment_id AS id,line_id,filename,mime,byte_size,created_at FROM ordenes_documents WHERE order_id=? ORDER BY line_id LIMIT 200").bind(id).fetch_all(&mut *tx).await?;
        tx.commit().await?;
        Ok(OrderDocuments {
            items,
            order_version: order.version,
        })
    }
    pub async fn upload_document(
        &self,
        actor: u64,
        id: u64,
        line: &str,
        expected: u64,
        key: &str,
        input: OrderDocumentInput,
    ) -> Result<OrderDocumentWrite, CoreError> {
        actions::validate_key(key)?;
        validate_id(line)?;
        self.check_access(actor, "documentos", "editar").await?;
        let (filename, bytes) = input.validate()?;
        let sha = Sha256::digest(&bytes).to_vec();
        let request_hash = Sha256::digest(
            serde_json::to_vec(&(id, line, expected, &filename, &sha))
                .map_err(|_| CoreError::Invalid)?,
        )
        .to_vec();
        let key_hash = Sha256::digest(key.as_bytes()).to_vec();
        let mut tx = self.begin(actor, "documentos", "editar").await?;
        let order = load(&mut tx, id).await?;
        if let Some(result) = receipt(&mut tx, actor, &key_hash, &request_hash).await? {
            tx.commit().await?;
            return Ok(result);
        }
        check_line(&order, line, expected)?;
        let existing: Option<String> = sqlx::query_scalar(
            "SELECT object_key FROM ordenes_documents WHERE order_id=? AND line_id=?",
        )
        .bind(id)
        .bind(line)
        .fetch_optional(&mut *tx)
        .await?;
        if existing.is_some() {
            self.admin
                .authorize(actor, "ordenes_compra", "documentos", "eliminar")
                .await?;
        }
        let storage = DocumentService::from_environment();
        if !storage.storage_configured() {
            return Err(CoreError::Overloaded);
        }
        let attachment_id = DocumentService::object_suffix();
        let object_key = format!("ordenes_compra/{id}/products/{attachment_id}");
        sqlx::query("INSERT INTO ordenes_document_deletions(object_key,available_at) VALUES (?,UTC_TIMESTAMP(6)+INTERVAL 5 MINUTE)").bind(&object_key).execute(&mut *tx).await?;
        tx.commit().await?;
        let size = bytes.len() as u32;
        let mime = if bytes.starts_with(b"%PDF-") {
            "application/pdf"
        } else {
            "application/octet-stream"
        };
        storage
            .put_with_limit(object_key.clone(), bytes, MAX_DOCUMENT_BYTES)
            .await?;
        let mut tx = self.begin(actor, "documentos", "editar").await?;
        let order = load(&mut tx, id).await?;
        if let Some(result) = receipt(&mut tx, actor, &key_hash, &request_hash).await? {
            tx.commit().await?;
            return Ok(result);
        }
        check_line(&order, line, expected)?;
        if let Some(old) = existing {
            self.admin
                .authorize(actor, "ordenes_compra", "documentos", "eliminar")
                .await?;
            sqlx::query("INSERT INTO ordenes_document_deletions(object_key,available_at) VALUES (?,UTC_TIMESTAMP(6)) ON DUPLICATE KEY UPDATE available_at=VALUES(available_at)").bind(&old).execute(&mut *tx).await?;
            sqlx::query("DELETE FROM ordenes_documents WHERE object_key=?")
                .bind(old)
                .execute(&mut *tx)
                .await?;
        }
        sqlx::query("INSERT INTO ordenes_documents(attachment_id,order_id,line_id,object_key,filename,mime,sha256,byte_size,created_at) VALUES (?,?,?,?,?,?,?,?,UTC_TIMESTAMP(6))")
            .bind(&attachment_id).bind(id).bind(line).bind(&object_key).bind(filename).bind(mime).bind(sha).bind(size).execute(&mut *tx).await?;
        let version = bump(&mut tx, actor, id, expected).await?;
        let attachment=sqlx::query_as("SELECT attachment_id AS id,line_id,filename,mime,byte_size,created_at FROM ordenes_documents WHERE attachment_id=?").bind(&attachment_id).fetch_one(&mut *tx).await?;
        let result = OrderDocumentWrite {
            attachment,
            order_version: version,
        };
        sqlx::query("INSERT INTO ordenes_document_receipts(actor_id,key_hash,request_hash,response_json) VALUES (?,?,?,?)").bind(actor).bind(key_hash).bind(request_hash).bind(serde_json::to_value(&result).map_err(|_|CoreError::Invalid)?).execute(&mut *tx).await?;
        sqlx::query("DELETE FROM ordenes_document_deletions WHERE object_key=?")
            .bind(object_key)
            .execute(&mut *tx)
            .await?;
        CoreAdmin::record_domain_event(&mut tx,actor,"ordenes_compra","order.document.save",&id.to_string(),serde_json::json!({"attachment_id":attachment_id,"line_id":line,"version":version,"byte_size":size})).await?;
        tx.commit().await?;
        Ok(result)
    }
    pub async fn document_content(
        &self,
        actor: u64,
        id: u64,
        attachment: &str,
    ) -> Result<OrderDocumentContent, CoreError> {
        validate_id(attachment)?;
        let mut tx = self.begin(actor, "documentos", "ver").await?;
        load(&mut tx, id).await?;
        let row:Option<(String,String,String,Vec<u8>,u32)>=sqlx::query_as("SELECT object_key,filename,mime,sha256,byte_size FROM ordenes_documents WHERE order_id=? AND attachment_id=?").bind(id).bind(attachment).fetch_optional(&mut *tx).await?;
        let (key, filename, mime, sha, size) = row.ok_or(CoreError::NotFound)?;
        tx.commit().await?;
        let storage = DocumentService::from_environment();
        if !storage.storage_configured() {
            return Err(CoreError::Overloaded);
        }
        let bytes = storage.get_with_limit(key, MAX_DOCUMENT_BYTES).await?;
        if bytes.len() != size as usize || Sha256::digest(&bytes).as_slice() != sha.as_slice() {
            return Err(CoreError::Overloaded);
        }
        let mut tx = self.begin(actor, "documentos", "ver").await?;
        let exists: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM ordenes_documents WHERE order_id=? AND attachment_id=?",
        )
        .bind(id)
        .bind(attachment)
        .fetch_one(&mut *tx)
        .await?;
        if exists != 1 {
            return Err(CoreError::NotFound);
        }
        CoreAdmin::record_domain_event(
            &mut tx,
            actor,
            "ordenes_compra",
            "order.document.download",
            &id.to_string(),
            serde_json::json!({"attachment_id":attachment}),
        )
        .await?;
        tx.commit().await?;
        Ok(OrderDocumentContent {
            filename,
            mime,
            content_base64: STANDARD.encode(bytes),
        })
    }
    pub async fn remove_document(
        &self,
        actor: u64,
        id: u64,
        attachment: &str,
        expected: u64,
    ) -> Result<u64, CoreError> {
        validate_id(attachment)?;
        let mut tx = self.begin(actor, "documentos", "eliminar").await?;
        let order = load(&mut tx, id).await?;
        if order.version != expected {
            return Err(CoreError::PreconditionFailed);
        }
        let key: Option<String> = sqlx::query_scalar(
            "SELECT object_key FROM ordenes_documents WHERE order_id=? AND attachment_id=?",
        )
        .bind(id)
        .bind(attachment)
        .fetch_optional(&mut *tx)
        .await?;
        let key = key.ok_or(CoreError::NotFound)?;
        sqlx::query("INSERT INTO ordenes_document_deletions(object_key,available_at) VALUES (?,UTC_TIMESTAMP(6)) ON DUPLICATE KEY UPDATE available_at=VALUES(available_at)").bind(&key).execute(&mut *tx).await?;
        sqlx::query("DELETE FROM ordenes_documents WHERE object_key=?")
            .bind(key)
            .execute(&mut *tx)
            .await?;
        let version = bump(&mut tx, actor, id, expected).await?;
        CoreAdmin::record_domain_event(
            &mut tx,
            actor,
            "ordenes_compra",
            "order.document.remove",
            &id.to_string(),
            serde_json::json!({"attachment_id":attachment,"version":version}),
        )
        .await?;
        tx.commit().await?;
        Ok(version)
    }
    pub async fn clean_one_document(&self) -> Result<bool, CoreError> {
        let mut tx = self.database.pool().begin().await?;
        let key:Option<String>=sqlx::query_scalar("SELECT object_key FROM ordenes_document_deletions WHERE available_at<=UTC_TIMESTAMP(6) ORDER BY available_at LIMIT 1 FOR UPDATE").fetch_optional(&mut *tx).await?;
        let Some(key) = key else {
            tx.commit().await?;
            return Ok(false);
        };
        sqlx::query("UPDATE ordenes_document_deletions SET available_at=UTC_TIMESTAMP(6)+INTERVAL 5 MINUTE WHERE object_key=?").bind(&key).execute(&mut *tx).await?;
        tx.commit().await?;
        let live: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM ordenes_documents WHERE object_key=?")
                .bind(&key)
                .fetch_one(self.database.pool())
                .await?;
        if live == 0 {
            DocumentService::from_environment()
                .remove(key.clone())
                .await?;
        }
        sqlx::query("DELETE FROM ordenes_document_deletions WHERE object_key=?")
            .bind(key)
            .execute(self.database.pool())
            .await?;
        Ok(true)
    }
}
fn check_line(order: &PurchaseOrder, line: &str, expected: u64) -> Result<(), CoreError> {
    if order.version != expected {
        return Err(CoreError::PreconditionFailed);
    }
    if !order.data.products.iter().any(|p| p.line_id == line) {
        return Err(CoreError::NotFound);
    }
    Ok(())
}
async fn bump(
    tx: &mut Transaction<'_, MySql>,
    actor: u64,
    id: u64,
    expected: u64,
) -> Result<u64, CoreError> {
    let next = expected.checked_add(1).ok_or(CoreError::Invalid)?;
    let r=sqlx::query("UPDATE ordenes_orders SET version=?,updated_by=?,updated_at=UTC_TIMESTAMP(6) WHERE id=? AND version=?").bind(next).bind(actor).bind(id).bind(expected).execute(&mut **tx).await?;
    if r.rows_affected() != 1 {
        return Err(CoreError::PreconditionFailed);
    }
    Ok(next)
}
async fn receipt(
    tx: &mut Transaction<'_, MySql>,
    actor: u64,
    key: &[u8],
    request: &[u8],
) -> Result<Option<OrderDocumentWrite>, CoreError> {
    let previous:Option<(Vec<u8>,serde_json::Value)>=sqlx::query_as("SELECT request_hash,response_json FROM ordenes_document_receipts WHERE actor_id=? AND key_hash=?").bind(actor).bind(key).fetch_optional(&mut **tx).await?;
    if let Some((hash, response)) = previous {
        if hash != request {
            return Err(CoreError::Conflict);
        }
        let result: OrderDocumentWrite =
            serde_json::from_value(response).map_err(|_| CoreError::Invalid)?;
        let exists: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM ordenes_documents WHERE attachment_id=?")
                .bind(&result.attachment.id)
                .fetch_one(&mut **tx)
                .await?;
        if exists != 1 {
            return Err(CoreError::NotFound);
        }
        return Ok(Some(result));
    }
    Ok(None)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validates_names_limits_and_content() {
        let input = |name: &str, bytes: &[u8]| OrderDocumentInput {
            filename: name.into(),
            content_base64: STANDARD.encode(bytes),
        };
        for name in ["", "../secret", "a/b", "a\\b", "a\r\n"] {
            assert!(input(name, b"x").validate().is_err());
        }
        assert!(input("factura.pdf", b"%PDF-1.7").validate().is_ok());
        assert!(input("empty", b"").validate().is_err());
        assert!(
            input("large", &vec![0; MAX_DOCUMENT_BYTES + 1])
                .validate()
                .is_err()
        );
        assert!(validate_id("../key").is_err());
    }
}
