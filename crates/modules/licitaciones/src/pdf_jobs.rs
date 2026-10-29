//! Persisted PDF queue; source content and credentials are never stored here.
use crate::{
    TenderService,
    service::{event, load},
};
use bastion_core::{CoreError, DocumentService};
use serde::Serialize;

#[derive(Clone, Serialize, sqlx::FromRow)]
pub struct PdfJob {
    pub id: String,
    pub expediente_id: u64,
    pub expediente_version: u64,
    pub state: String,
    pub byte_size: Option<u64>,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

fn job_id(id: &str) -> Result<(), CoreError> {
    if id.len() != 48
        || !id
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(CoreError::Invalid);
    }
    Ok(())
}

impl TenderService {
    /// Queue one version of an expediente. A row lock serializes duplicate
    /// requests; the worker will recheck permissions and the source version.
    pub async fn create_pdf_job(
        &self,
        actor: u64,
        id: u64,
        expected: u64,
    ) -> Result<PdfJob, CoreError> {
        let mut tx = self.begin(actor, "editar").await?;
        let exp = load(&mut tx, id).await?;
        if exp.version != expected {
            return Err(CoreError::PreconditionFailed);
        }
        let current:Option<PdfJob>=sqlx::query_as("SELECT id,expediente_id,expediente_version,state,byte_size,created_at,updated_at FROM licitaciones_pdf_jobs WHERE expediente_id=? AND state IN ('queued','running','uploading') ORDER BY created_at DESC LIMIT 1")
            .bind(id).fetch_optional(&mut *tx).await?;
        if let Some(job) = current {
            if job.expediente_version != expected {
                return Err(CoreError::Conflict);
            }
            tx.commit().await?;
            return Ok(job);
        }
        let job = DocumentService::object_suffix();
        let key = format!("licitaciones/{id}/pdf/{job}.pdf");
        sqlx::query("INSERT INTO licitaciones_pdf_jobs(id,expediente_id,actor_id,expediente_version,state,object_key) VALUES (?,?,?,?,'queued',?)")
            .bind(&job).bind(id).bind(actor).bind(expected).bind(key).execute(&mut *tx).await?;
        let result:PdfJob=sqlx::query_as("SELECT id,expediente_id,expediente_version,state,byte_size,created_at,updated_at FROM licitaciones_pdf_jobs WHERE id=?")
            .bind(&job).fetch_one(&mut *tx).await?;
        event(&mut tx, actor, "pdf.queue", id, expected).await?;
        tx.commit().await?;
        Ok(result)
    }
    pub async fn pdf_job(&self, actor: u64, id: u64, job: &str) -> Result<PdfJob, CoreError> {
        job_id(job)?;
        let mut tx = self.begin(actor, "ver").await?;
        load(&mut tx, id).await?;
        let result=sqlx::query_as("SELECT id,expediente_id,expediente_version,state,byte_size,created_at,updated_at FROM licitaciones_pdf_jobs WHERE id=? AND expediente_id=?")
            .bind(job).bind(id).fetch_optional(&mut *tx).await?.ok_or(CoreError::NotFound)?;
        tx.commit().await?;
        Ok(result)
    }
}

struct QueuedPdf {
    id: String,
    expediente_id: u64,
    actor_id: u64,
    expediente_version: u64,
    object_key: String,
}

#[cfg(target_os = "linux")]
struct PrivateJobDir(std::path::PathBuf);
#[cfg(target_os = "linux")]
impl Drop for PrivateJobDir {
    fn drop(&mut self) {
        // Only a fixed incoming directory plus a validated random identifier is
        // ever supplied. A moved job may already be absent; recovery owns it.
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

impl TenderService {
    pub fn pdf_generation_enabled() -> bool {
        cfg!(target_os = "linux")
            && std::env::var("BASTION_TENDER_PDF_ENABLED").as_deref() == Ok("true")
    }

    pub async fn latest_pdf_job(&self, actor: u64, id: u64) -> Result<Option<PdfJob>, CoreError> {
        let mut tx = self.begin(actor, "ver").await?;
        load(&mut tx, id).await?;
        let result=sqlx::query_as("SELECT id,expediente_id,expediente_version,state,byte_size,created_at,updated_at FROM licitaciones_pdf_jobs WHERE expediente_id=? ORDER BY created_at DESC,id DESC LIMIT 1")
            .bind(id).fetch_optional(&mut *tx).await?;
        tx.commit().await?;
        Ok(result)
    }

    async fn authorized_pdf(
        &self,
        actor: u64,
        id: u64,
        job: &str,
    ) -> Result<(u64, String, u64, String), CoreError> {
        job_id(job)?;
        let mut tx = self.begin(actor, "ver").await?;
        let exp = load(&mut tx, id).await?;
        let row:Option<(u64,String,Option<u64>,Option<String>,Option<serde_json::Value>)>=sqlx::query_as("SELECT expediente_version,object_key,byte_size,sha256,references_json FROM licitaciones_pdf_jobs WHERE id=? AND expediente_id=? AND state='ready'")
            .bind(job).bind(id).fetch_optional(&mut *tx).await?;
        let (version, key, size, hash, refs) = row.ok_or(CoreError::NotFound)?;
        if exp.version != version {
            return Err(CoreError::PreconditionFailed);
        }
        let references: Vec<(u64, Vec<u8>)> =
            serde_json::from_value(refs.ok_or(CoreError::Invalid)?)
                .map_err(|_| CoreError::Invalid)?;
        for (source, hash) in references {
            if hash.len() != 32 {
                return Err(CoreError::Invalid);
            }
            self.library
                .validate_file_snapshot_in_transaction(&mut tx, actor, source, &hash)
                .await?;
        }
        let result = (
            version,
            key,
            size.ok_or(CoreError::Invalid)?,
            hash.ok_or(CoreError::Invalid)?,
        );
        tx.commit().await?;
        Ok(result)
    }

    /// Recheck access, the expediente version and every referenced source both
    /// before and after the bounded download. The returned file is unlinked.
    pub async fn download_pdf_job(
        &self,
        actor: u64,
        id: u64,
        job: &str,
    ) -> Result<(std::fs::File, u64), CoreError> {
        let metadata = self.authorized_pdf(actor, id, job).await?;
        let file = self
            .storage
            .open_tender_pdf(metadata.1.clone(), metadata.2, metadata.3.clone())
            .await?;
        if self.authorized_pdf(actor, id, job).await? != metadata {
            return Err(CoreError::Conflict);
        }
        Ok((file, metadata.2))
    }

    /// One long-lived server task calls this serially. A database row is
    /// claimed before work and sources are reauthorized before publication.
    pub async fn process_one_pdf_job(&self) -> Result<bool, CoreError> {
        if !Self::pdf_generation_enabled() {
            return Ok(false);
        }
        let mut tx = self.database.pool().begin().await?;
        let enabled: Option<bool> = sqlx::query_scalar("SELECT (included_in_release AND environment_allowed AND enabled) FROM module_registry WHERE module_id='licitaciones' FOR UPDATE").fetch_optional(&mut *tx).await?;
        if enabled != Some(true) {
            tx.commit().await?;
            return Ok(false);
        }
        sqlx::query("UPDATE licitaciones_pdf_jobs SET state='failed',cleanup_pending=1,updated_at=UTC_TIMESTAMP(6) WHERE state IN ('running','uploading') AND updated_at<UTC_TIMESTAMP(6)-INTERVAL 6 HOUR").execute(&mut *tx).await?;
        let next:Option<(String,u64,u64,u64,String)>=sqlx::query_as("SELECT id,expediente_id,actor_id,expediente_version,object_key FROM licitaciones_pdf_jobs WHERE state='queued' ORDER BY created_at LIMIT 1 FOR UPDATE").fetch_optional(&mut *tx).await?;
        let Some((id, expediente_id, actor_id, expediente_version, object_key)) = next else {
            tx.commit().await?;
            return Ok(false);
        };
        job_id(&id)?;
        sqlx::query("UPDATE licitaciones_pdf_jobs SET state='running',updated_at=UTC_TIMESTAMP(6) WHERE id=? AND state='queued'").bind(&id).execute(&mut *tx).await?;
        tx.commit().await?;
        let job = QueuedPdf {
            id,
            expediente_id,
            actor_id,
            expediente_version,
            object_key,
        };
        if self.run_pdf_job(&job).await.is_err() {
            sqlx::query("UPDATE licitaciones_pdf_jobs SET state='failed',cleanup_pending=1,updated_at=UTC_TIMESTAMP(6) WHERE id=? AND state IN ('running','uploading')")
                .bind(&job.id).execute(self.database.pool()).await?;
        }
        Ok(true)
    }

    #[cfg(not(target_os = "linux"))]
    async fn run_pdf_job(&self, _job: &QueuedPdf) -> Result<(), CoreError> {
        Err(CoreError::Overloaded)
    }

    #[cfg(target_os = "linux")]
    async fn run_pdf_job(&self, job: &QueuedPdf) -> Result<(), CoreError> {
        use std::{fs, io::Write, os::unix::fs::DirBuilderExt, path::PathBuf};
        job_id(&job.id)?;
        let base = PathBuf::from("/var/lib/bastion-tender/incoming");
        let root = base.join(&job.id);
        let mut builder = fs::DirBuilder::new();
        builder.mode(0o700);
        builder.create(&root).map_err(|_| CoreError::Overloaded)?;
        let _cleanup = PrivateJobDir(root.clone());
        let template_root = std::env::var_os("BASTION_TENDER_TEMPLATE_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/opt/bastion-pdf/private-templates-20261029"));
        let bundle = crate::TenderTemplateBundle::from_private_directory(&template_root)?;
        let request = self
            .stage_pdf_inputs(
                job.actor_id,
                job.expediente_id,
                job.expediente_version,
                root.clone(),
                &bundle,
            )
            .await?;
        let bytes = serde_json::to_vec(&request).map_err(|_| CoreError::Invalid)?;
        if bytes.len() > 2_000_000 {
            return Err(CoreError::Invalid);
        }
        let mut f = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(root.join("request.json"))
            .map_err(|_| CoreError::Overloaded)?;
        f.write_all(&bytes).map_err(|_| CoreError::Overloaded)?;
        drop(f);
        let identifier = job.id.clone();
        tokio::task::spawn_blocking(move || {
            use std::{
                io::{Read, Write},
                os::unix::net::UnixStream,
                time::Duration,
            };
            let mut stream = UnixStream::connect("/run/bastion-tender-launcher.sock")
                .map_err(|_| CoreError::Overloaded)?;
            stream
                .set_read_timeout(Some(Duration::from_secs(1850)))
                .map_err(|_| CoreError::Overloaded)?;
            stream
                .set_write_timeout(Some(Duration::from_secs(10)))
                .map_err(|_| CoreError::Overloaded)?;
            stream
                .write_all(format!("{identifier}\n").as_bytes())
                .map_err(|_| CoreError::Overloaded)?;
            let mut response = [0u8; 3];
            stream
                .read_exact(&mut response)
                .map_err(|_| CoreError::Overloaded)?;
            if &response != b"ok\n" {
                return Err(CoreError::Overloaded);
            }
            Ok(())
        })
        .await
        .map_err(|_| CoreError::Overloaded)??;
        let result = root.join("result.pdf");
        if !fs::symlink_metadata(&result)
            .map_err(|_| CoreError::Invalid)?
            .file_type()
            .is_file()
        {
            return Err(CoreError::Invalid);
        }
        let file = fs::File::open(&result).map_err(|_| CoreError::Overloaded)?;
        self.check_pdf_job_sources(job, &request.reference_checks, "running", "uploading")
            .await?;
        let (size, hash) = self
            .storage
            .put_tender_pdf_file(job.object_key.clone(), file)
            .await?;
        let mut tx = self.begin(job.actor_id, "editar").await?;
        let current = load(&mut tx, job.expediente_id).await?;
        if current.version != job.expediente_version {
            return Err(CoreError::PreconditionFailed);
        }
        for (id, hash) in &request.reference_checks {
            self.library
                .validate_file_snapshot_in_transaction(&mut tx, job.actor_id, *id, hash)
                .await?;
        }
        let references =
            serde_json::to_value(&request.reference_checks).map_err(|_| CoreError::Invalid)?;
        let result=sqlx::query("UPDATE licitaciones_pdf_jobs SET state='ready',byte_size=?,sha256=?,references_json=?,updated_at=UTC_TIMESTAMP(6) WHERE id=? AND state='uploading'")
            .bind(size).bind(hash).bind(references).bind(&job.id).execute(&mut *tx).await?;
        if result.rows_affected() != 1 {
            return Err(CoreError::Conflict);
        }
        sqlx::query("UPDATE licitaciones_pdf_jobs SET state='expired',cleanup_pending=1,cleanup_after=NULL,updated_at=UTC_TIMESTAMP(6) WHERE expediente_id=? AND id<>? AND state='ready'")
            .bind(job.expediente_id).bind(&job.id).execute(&mut *tx).await?;
        event(
            &mut tx,
            job.actor_id,
            "pdf.generate",
            job.expediente_id,
            job.expediente_version,
        )
        .await?;
        tx.commit().await?;
        Ok(())
    }

    #[cfg(target_os = "linux")]
    async fn check_pdf_job_sources(
        &self,
        job: &QueuedPdf,
        references: &[(u64, Vec<u8>)],
        from: &str,
        to: &str,
    ) -> Result<(), CoreError> {
        let mut tx = self.begin(job.actor_id, "editar").await?;
        let current = load(&mut tx, job.expediente_id).await?;
        if current.version != job.expediente_version {
            return Err(CoreError::PreconditionFailed);
        }
        for (id, hash) in references {
            self.library
                .validate_file_snapshot_in_transaction(&mut tx, job.actor_id, *id, hash)
                .await?;
        }
        let result=sqlx::query("UPDATE licitaciones_pdf_jobs SET state=?,updated_at=UTC_TIMESTAMP(6) WHERE id=? AND state=?")
            .bind(to).bind(&job.id).bind(from).execute(&mut *tx).await?;
        if result.rows_affected() != 1 {
            return Err(CoreError::Conflict);
        }
        tx.commit().await?;
        Ok(())
    }

    /// Delete abandoned or superseded PDF objects through the same storage
    /// service; the lease makes cleanup retryable after a server restart.
    pub async fn clean_one_pdf_job(&self) -> Result<bool, CoreError> {
        let mut tx = self.database.pool().begin().await?;
        let row:Option<(String,String)>=sqlx::query_as("SELECT id,object_key FROM licitaciones_pdf_jobs WHERE cleanup_pending=1 AND (cleanup_after IS NULL OR cleanup_after<=UTC_TIMESTAMP(6)) ORDER BY updated_at LIMIT 1 FOR UPDATE").fetch_optional(&mut *tx).await?;
        let Some((id, key)) = row else {
            tx.commit().await?;
            return Ok(false);
        };
        job_id(&id)?;
        sqlx::query("UPDATE licitaciones_pdf_jobs SET cleanup_after=UTC_TIMESTAMP(6)+INTERVAL 15 MINUTE WHERE id=?").bind(&id).execute(&mut *tx).await?;
        tx.commit().await?;
        self.storage.remove_all_versions(key).await?;
        #[cfg(target_os = "linux")]
        {
            let path = std::path::Path::new("/var/lib/bastion-tender/incoming").join(&id);
            match std::fs::remove_dir_all(path) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(_) => return Err(CoreError::Overloaded),
            }
        }
        sqlx::query("UPDATE licitaciones_pdf_jobs SET cleanup_pending=0,cleanup_after=NULL WHERE id=? AND cleanup_pending=1").bind(id).execute(self.database.pool()).await?;
        Ok(true)
    }
}
