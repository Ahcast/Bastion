//! Private document generation and object storage shared by business modules.
use crate::CoreError;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

const MAX_DOCUMENT_BYTES: u64 = 8 * 1024 * 1024;
static PDF_SLOTS: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(2);
static STORAGE_SLOTS: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(4);

#[derive(Clone, Default, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DocumentCompany {
    pub name: String,
    pub rtn: String,
    pub email: String,
    pub phone: String,
    pub address: String,
}
impl DocumentCompany {
    pub fn validate(&self) -> Result<(), CoreError> {
        if self.name.trim().is_empty()
            || self.name.chars().count() > 255
            || self.rtn.len() > 40
            || self.email.len() > 254
            || self.phone.len() > 60
            || self.address.chars().count() > 1000
            || [&self.name, &self.rtn, &self.email, &self.phone]
                .iter()
                .any(|v| v.chars().any(char::is_control))
            || (!self.email.is_empty() && self.email.parse::<lettre::Address>().is_err())
        {
            return Err(CoreError::Invalid);
        }
        Ok(())
    }
}

#[derive(Clone)]
pub struct DocumentService {
    enabled: bool,
    typst: PathBuf,
    aws: PathBuf,
    work: PathBuf,
    branding: Option<PathBuf>,
    bucket: String,
    region: String,
}
impl DocumentService {
    pub fn from_environment() -> Self {
        Self {
            enabled: std::env::var("BASTION_DOCUMENTS_ENABLED").as_deref() == Ok("true"),
            typst: std::env::var_os("BASTION_TYPST_BIN")
                .map(PathBuf::from)
                .unwrap_or_else(|| "/usr/local/bin/typst".into()),
            aws: std::env::var_os("BASTION_AWS_CLI")
                .map(PathBuf::from)
                .unwrap_or_else(|| "/usr/bin/aws".into()),
            work: std::env::var_os("BASTION_DOCUMENTS_WORK_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|| std::env::temp_dir().join("bastion-documents")),
            branding: std::env::var_os("BASTION_BRANDING_DIR").map(PathBuf::from),
            bucket: std::env::var("BASTION_DOCUMENTS_BUCKET").unwrap_or_default(),
            region: std::env::var("BASTION_DOCUMENTS_REGION").unwrap_or_default(),
        }
    }
    pub fn branding(&self) -> (bool, bool) {
        self.branding
            .as_ref()
            .map(|path| {
                (
                    path.join("logo.png").is_file(),
                    path.join("signature.png").is_file(),
                )
            })
            .unwrap_or_default()
    }
    pub fn configured(&self) -> bool {
        self.enabled
            && self.typst.is_absolute()
            && self.typst.is_file()
            && self.aws.is_absolute()
            && self.aws.is_file()
            && !self.bucket.is_empty()
            && !self.region.is_empty()
    }
    pub fn branding_fingerprint(&self) -> Result<String, CoreError> {
        let mut hash = Sha256::new();
        if let Some(branding) = &self.branding {
            for name in ["logo.png", "signature.png"] {
                let path = branding.join(name);
                if path.is_file() {
                    hash.update(name.as_bytes());
                    hash.update(bounded_read(&path)?);
                }
            }
        }
        Ok(hex::encode(hash.finalize()))
    }
    pub async fn render(
        &self,
        template: &'static str,
        payload: serde_json::Value,
    ) -> Result<Vec<u8>, CoreError> {
        let permit = tokio::time::timeout(Duration::from_secs(1), PDF_SLOTS.acquire())
            .await
            .map_err(|_| CoreError::Overloaded)?
            .map_err(|_| CoreError::Overloaded)?;
        let this = self.clone();
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            if !this.typst.is_absolute() || !this.typst.is_file() {
                return Err(CoreError::Overloaded);
            }
            let dir = this.directory()?;
            fs::write(dir.0.join("main.typ"), template).map_err(|_| CoreError::Overloaded)?;
            fs::write(
                dir.0.join("data.json"),
                serde_json::to_vec(&payload).map_err(|_| CoreError::Invalid)?,
            )
            .map_err(|_| CoreError::Overloaded)?;
            if let Some(branding) = &this.branding {
                for name in ["logo.png", "signature.png"] {
                    let source = branding.join(name);
                    if source.is_file() {
                        fs::write(dir.0.join(name), bounded_read(&source)?)
                            .map_err(|_| CoreError::Overloaded)?;
                    }
                }
            }
            let mut command = Command::new(&this.typst);
            command
                .env_clear()
                .env("HOME", &dir.0)
                .args(["compile", "--creation-timestamp"])
                .arg(
                    payload
                        .get("creation_timestamp")
                        .and_then(|value| value.as_i64())
                        .unwrap_or(0)
                        .to_string(),
                )
                .arg("--root")
                .arg(&dir.0)
                .arg(dir.0.join("main.typ"))
                .arg(dir.0.join("document.pdf"));
            run(command, Duration::from_secs(20))?;
            let bytes = bounded_read(&dir.0.join("document.pdf"))?;
            if !bytes.starts_with(b"%PDF-") {
                return Err(CoreError::Overloaded);
            }
            Ok(bytes)
        })
        .await
        .map_err(|_| CoreError::Overloaded)?
    }
    pub async fn put(&self, key: String, bytes: Vec<u8>) -> Result<(), CoreError> {
        validate_key(&key)?;
        let permit = storage_permit().await?;
        if bytes.len() as u64 > MAX_DOCUMENT_BYTES {
            return Err(CoreError::Invalid);
        }
        let this = self.clone();
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            let dir = this.directory()?;
            let file = dir.0.join("object");
            fs::write(&file, bytes).map_err(|_| CoreError::Overloaded)?;
            let mut command = this.storage_command("put-object", &key)?;
            command
                .arg("--body")
                .arg(file)
                .args(["--server-side-encryption", "AES256"]);
            run(command, Duration::from_secs(20))
        })
        .await
        .map_err(|_| CoreError::Overloaded)?
    }
    pub async fn get(&self, key: String) -> Result<Vec<u8>, CoreError> {
        validate_key(&key)?;
        let permit = storage_permit().await?;
        let this = self.clone();
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            let dir = this.directory()?;
            let file = dir.0.join("object");
            let mut command = this.storage_command("get-object", &key)?;
            command.arg(&file);
            run(command, Duration::from_secs(20))?;
            bounded_read(&file)
        })
        .await
        .map_err(|_| CoreError::Overloaded)?
    }
    pub async fn remove(&self, key: String) -> Result<(), CoreError> {
        validate_key(&key)?;
        let permit = storage_permit().await?;
        let this = self.clone();
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            run(
                this.storage_command("delete-object", &key)?,
                Duration::from_secs(20),
            )
        })
        .await
        .map_err(|_| CoreError::Overloaded)?
    }
    fn storage_command(&self, operation: &str, key: &str) -> Result<Command, CoreError> {
        if !self.aws.is_absolute()
            || !self.aws.is_file()
            || self.bucket.is_empty()
            || self.region.is_empty()
        {
            return Err(CoreError::Overloaded);
        }
        let mut command = Command::new(&self.aws);
        command
            .env_clear()
            .env("PATH", "/usr/local/bin:/usr/bin:/bin");
        // The AWS helper receives only its own credentials, never database or encryption secrets.
        for name in [
            "AWS_ACCESS_KEY_ID",
            "AWS_SECRET_ACCESS_KEY",
            "AWS_SESSION_TOKEN",
            "AWS_SHARED_CREDENTIALS_FILE",
            "AWS_PROFILE",
            "AWS_CONFIG_FILE",
            "HOME",
        ] {
            if let Some(value) = std::env::var_os(name) {
                command.env(name, value);
            }
        }
        command.args([
            "s3api",
            operation,
            "--bucket",
            &self.bucket,
            "--key",
            key,
            "--region",
            &self.region,
            "--no-cli-pager",
            "--cli-connect-timeout",
            "5",
            "--cli-read-timeout",
            "15",
        ]);
        Ok(command)
    }
    fn directory(&self) -> Result<WorkDirectory, CoreError> {
        if !self.work.is_absolute() {
            return Err(CoreError::Overloaded);
        }
        fs::create_dir_all(&self.work).map_err(|_| CoreError::Overloaded)?;
        let mut random = [0u8; 24];
        rand::rngs::OsRng.fill_bytes(&mut random);
        let path = self.work.join(hex::encode(random));
        let mut builder = fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder.create(&path).map_err(|_| CoreError::Overloaded)?;
        Ok(WorkDirectory(path))
    }
}
async fn storage_permit() -> Result<tokio::sync::SemaphorePermit<'static>, CoreError> {
    tokio::time::timeout(Duration::from_secs(1), STORAGE_SLOTS.acquire())
        .await
        .map_err(|_| CoreError::Overloaded)?
        .map_err(|_| CoreError::Overloaded)
}
struct WorkDirectory(PathBuf);
impl Drop for WorkDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn bounded_read(path: &Path) -> Result<Vec<u8>, CoreError> {
    use std::io::Read;
    let file = fs::File::open(path).map_err(|_| CoreError::Overloaded)?;
    let mut bytes = Vec::new();
    file.take(MAX_DOCUMENT_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| CoreError::Overloaded)?;
    if bytes.len() as u64 > MAX_DOCUMENT_BYTES {
        return Err(CoreError::Invalid);
    }
    Ok(bytes)
}
fn run(mut command: Command, timeout: Duration) -> Result<(), CoreError> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let mut child = command.spawn().map_err(|_| CoreError::Overloaded)?;
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                return if status.success() {
                    Ok(())
                } else {
                    Err(CoreError::Overloaded)
                };
            }
            Ok(None) if started.elapsed() < timeout => {
                std::thread::sleep(Duration::from_millis(25))
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(CoreError::Overloaded);
            }
        }
    }
}
pub(crate) fn validate_key(key: &str) -> Result<(), CoreError> {
    if !key.starts_with("cotizaciones/")
        || key.len() > 250
        || key
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
        || !key
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'/' | b'-' | b'_' | b'.'))
    {
        return Err(CoreError::Invalid);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn storage_paths_never_escape_the_module_prefix() {
        assert!(validate_key("cotizaciones/12/3/document-v1.pdf").is_ok());
        for value in [
            "/cotizaciones/12.pdf",
            "cotizaciones/../secret",
            "cotizaciones//12",
            "cotizaciones/./12",
            "core/12",
            "cotizaciones/12?acl=public-read",
            "cotizaciones/12\\secret",
        ] {
            assert!(validate_key(value).is_err(), "{value}");
        }
    }
}
