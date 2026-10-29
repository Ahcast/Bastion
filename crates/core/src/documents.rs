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
const MAX_STORAGE_BYTES: usize = 20 * 1024 * 1024;
pub const MAX_PRIVATE_FILE_BYTES: usize = 100 * 1024 * 1024;
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
    #[cfg(feature = "test-support")]
    pub fn synthetic_storage(aws: PathBuf, work: PathBuf) -> Self {
        Self {
            enabled: true,
            typst: PathBuf::new(),
            aws,
            work,
            branding: None,
            bucket: "synthetic-private-bucket".into(),
            region: "us-east-1".into(),
        }
    }
    pub fn object_suffix() -> String {
        let mut random = [0u8; 24];
        rand::rngs::OsRng.fill_bytes(&mut random);
        hex::encode(random)
    }
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
        self.storage_configured() && self.typst.is_absolute() && self.typst.is_file()
    }
    pub fn storage_configured(&self) -> bool {
        self.enabled
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
        self.put_with_limit(key, bytes, MAX_STORAGE_BYTES).await
    }
    /// Upload a previously authorized PDF through an already-open regular file.
    /// The S3 SHA-256 checksum rejects changes to the file during transfer.
    /// The caller owns lifecycle, authorization, and cleanup of the object.
    pub async fn put_tender_pdf_file(
        &self,
        key: String,
        file: fs::File,
    ) -> Result<(u64, String), CoreError> {
        if !cfg!(target_os = "linux") || !key.starts_with("licitaciones/") {
            return Err(CoreError::Invalid);
        }
        validate_key(&key)?;
        let permit = storage_permit().await?;
        let this = self.clone();
        tokio::task::spawn_blocking(move || {
            use base64::{Engine as _, engine::general_purpose::STANDARD};
            use std::io::{Read, Seek, SeekFrom};
            let _permit = permit;
            let mut file = file;
            let metadata = file.metadata().map_err(|_| CoreError::Invalid)?;
            let size = metadata.len();
            if !metadata.is_file() || !(5..=1_000_000_000).contains(&size) {
                return Err(CoreError::Invalid);
            }
            file.seek(SeekFrom::Start(0))
                .map_err(|_| CoreError::Invalid)?;
            let mut header = [0u8; 5];
            file.read_exact(&mut header)
                .map_err(|_| CoreError::Invalid)?;
            if &header != b"%PDF-" {
                return Err(CoreError::Invalid);
            }
            file.seek(SeekFrom::Start(0))
                .map_err(|_| CoreError::Invalid)?;
            let mut hash = Sha256::new();
            let mut total = 0u64;
            let mut buffer = vec![0u8; 256 * 1024];
            loop {
                let n = file.read(&mut buffer).map_err(|_| CoreError::Overloaded)?;
                if n == 0 {
                    break;
                }
                total = total.checked_add(n as u64).ok_or(CoreError::Invalid)?;
                if total > size {
                    return Err(CoreError::Invalid);
                }
                hash.update(&buffer[..n]);
            }
            if total != size || file.metadata().map_err(|_| CoreError::Invalid)?.len() != size {
                return Err(CoreError::Invalid);
            }
            let digest = hash.finalize();
            let hex = hex::encode(&digest);
            let checksum = STANDARD.encode(&digest);
            file.seek(SeekFrom::Start(0))
                .map_err(|_| CoreError::Invalid)?;
            let mut command = this.storage_command("put-object", &key)?;
            command.args([
                "--body",
                "/dev/stdin",
                "--content-length",
                &size.to_string(),
                "--checksum-algorithm",
                "SHA256",
                "--checksum-sha256",
                &checksum,
                "--server-side-encryption",
                "AES256",
            ]);
            run_file_input(command, file, Duration::from_secs(600))?;
            Ok((size, hex))
        })
        .await
        .map_err(|_| CoreError::Overloaded)?
    }
    /// Stream one authorized tender PDF into an already-open private file.
    /// The caller provides the immutable database size and SHA-256 and owns
    /// file cleanup; data is never collected into a large in-memory buffer.
    pub async fn get_tender_pdf_file(
        &self,
        key: String,
        mut file: fs::File,
        expected_size: u64,
        expected_sha256: String,
    ) -> Result<fs::File, CoreError> {
        if !cfg!(target_os = "linux")
            || !key.starts_with("licitaciones/")
            || !(5..=1_000_000_000).contains(&expected_size)
            || expected_sha256.len() != 64
            || !expected_sha256
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(CoreError::Invalid);
        }
        validate_key(&key)?;
        let expected = hex::decode(expected_sha256).map_err(|_| CoreError::Invalid)?;
        let permit = storage_permit().await?;
        let this = self.clone();
        tokio::task::spawn_blocking(move || {
            use std::io::{Read, Seek, SeekFrom};
            let _permit = permit;
            if !file.metadata().map_err(|_| CoreError::Invalid)?.is_file() {
                return Err(CoreError::Invalid);
            }
            file.set_len(0).map_err(|_| CoreError::Overloaded)?;
            file.seek(SeekFrom::Start(0))
                .map_err(|_| CoreError::Overloaded)?;
            let output = file.try_clone().map_err(|_| CoreError::Overloaded)?;
            let mut command = this.aws_command()?;
            command.args([
                "s3",
                "cp",
                &format!("s3://{}/{}", this.bucket, key),
                "-",
                "--region",
                &this.region,
                "--no-progress",
                "--only-show-errors",
                "--no-cli-pager",
                "--cli-connect-timeout",
                "5",
                "--cli-read-timeout",
                "15",
            ]);
            run_bounded_download(
                command,
                output,
                expected_size,
                &expected,
                Duration::from_secs(900),
            )?;
            file.seek(SeekFrom::Start(0))
                .map_err(|_| CoreError::Overloaded)?;
            let mut header = [0u8; 5];
            file.read_exact(&mut header)
                .map_err(|_| CoreError::Overloaded)?;
            if &header != b"%PDF-" {
                return Err(CoreError::Invalid);
            }
            file.seek(SeekFrom::Start(0))
                .map_err(|_| CoreError::Overloaded)?;
            Ok(file)
        })
        .await
        .map_err(|_| CoreError::Overloaded)?
    }
    pub async fn put_with_limit(
        &self,
        key: String,
        bytes: Vec<u8>,
        limit: usize,
    ) -> Result<(), CoreError> {
        validate_key(&key)?;
        let permit = storage_permit().await?;
        if limit > MAX_PRIVATE_FILE_BYTES || bytes.len() > limit {
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
            run(
                command,
                Duration::from_secs(if limit > MAX_DOCUMENT_BYTES as usize {
                    90
                } else {
                    20
                }),
            )
        })
        .await
        .map_err(|_| CoreError::Overloaded)?
    }
    pub async fn get(&self, key: String) -> Result<Vec<u8>, CoreError> {
        self.get_with_limit(key, MAX_STORAGE_BYTES).await
    }
    pub async fn get_with_limit(&self, key: String, limit: usize) -> Result<Vec<u8>, CoreError> {
        if limit > MAX_PRIVATE_FILE_BYTES {
            return Err(CoreError::Invalid);
        }
        validate_key(&key)?;
        let permit = storage_permit().await?;
        let this = self.clone();
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            let dir = this.directory()?;
            let file = dir.0.join("object");
            let mut command = this.storage_command("get-object", &key)?;
            command.arg(&file);
            run(
                command,
                Duration::from_secs(if limit > MAX_DOCUMENT_BYTES as usize {
                    90
                } else {
                    20
                }),
            )?;
            bounded_read_limit(&file, limit as u64)
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
    /// Permanent removal including object versions and delete markers. Each
    /// bounded batch only touches the exact authorized key, never its prefix.
    pub async fn remove_all_versions(&self, key: String) -> Result<(), CoreError> {
        validate_key(&key)?;
        let permit = storage_permit().await?;
        let this = self.clone();
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            let dir = this.directory()?;
            let path = dir.0.join("versions.json");
            let mut list = this.storage_command("list-object-versions", &key)?;
            list.args(["--max-keys", "20", "--no-paginate", "--output", "json"]);
            run_output(
                list,
                Duration::from_secs(20),
                Some(fs::File::create(&path).map_err(|_| CoreError::Overloaded)?),
            )?;
            let value: serde_json::Value =
                serde_json::from_slice(&bounded_read_limit(&path, 64 * 1024)?)
                    .map_err(|_| CoreError::Overloaded)?;
            if !value.is_object() || value["IsTruncated"].as_bool().is_none() {
                return Err(CoreError::Overloaded);
            }
            let started = Instant::now();
            let mut removed = false;
            for group in ["Versions", "DeleteMarkers"] {
                if value.get(group).is_some_and(|entries| !entries.is_array()) {
                    return Err(CoreError::Overloaded);
                }
                if let Some(entries) = value[group].as_array() {
                    if entries.len() > 20 {
                        return Err(CoreError::Overloaded);
                    }
                    for entry in entries {
                        if entry["Key"].as_str() != Some(key.as_str()) {
                            continue;
                        }
                        let version = entry["VersionId"]
                            .as_str()
                            .filter(|v| !v.is_empty() && v.len() < 2048)
                            .ok_or(CoreError::Overloaded)?;
                        if started.elapsed() > Duration::from_secs(40) {
                            return Err(CoreError::Overloaded);
                        }
                        let mut remove = this.storage_command("delete-object", &key)?;
                        remove.arg(format!("--version-id={version}"));
                        run(remove, Duration::from_secs(20))?;
                        removed = true;
                    }
                }
            }
            if removed && value["IsTruncated"].as_bool() == Some(true) {
                return Err(CoreError::Overloaded);
            }
            Ok(())
        })
        .await
        .map_err(|_| CoreError::Overloaded)?
    }
    /// Detect content using the server's libmagic database, never a browser MIME.
    pub async fn detect_mime(&self, bytes: Vec<u8>) -> Result<String, CoreError> {
        if bytes.is_empty() || bytes.len() > MAX_PRIVATE_FILE_BYTES {
            return Err(CoreError::Invalid);
        }
        let permit = storage_permit().await?;
        let this = self.clone();
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            let bin = std::env::var_os("BASTION_FILE_BIN")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("/usr/bin/file"));
            if !bin.is_absolute() || !bin.is_file() {
                return Err(CoreError::Overloaded);
            }
            let dir = this.directory()?;
            let path = dir.0.join("mime-input");
            fs::write(&path, bytes).map_err(|_| CoreError::Overloaded)?;
            let output_path = dir.0.join("mime-output");
            let output = fs::File::create(&output_path).map_err(|_| CoreError::Overloaded)?;
            let mut command = Command::new(bin);
            command
                .env_clear()
                .args(["--brief", "--mime-type", "--"])
                .arg(path)
                .stdout(output);
            // run() suppresses output; use a dedicated bounded subprocess below.
            command.stdin(Stdio::null()).stderr(Stdio::null());
            let mut child = command.spawn().map_err(|_| CoreError::Overloaded)?;
            let started = Instant::now();
            loop {
                match child.try_wait() {
                    Ok(Some(status)) if status.success() => break,
                    Ok(None) if started.elapsed() < Duration::from_secs(5) => {
                        std::thread::sleep(Duration::from_millis(25))
                    }
                    _ => {
                        let _ = child.kill();
                        let _ = child.wait();
                        return Err(CoreError::Overloaded);
                    }
                }
            }
            let raw = bounded_read_limit(&output_path, 200)?;
            let mime = String::from_utf8(raw).map_err(|_| CoreError::Invalid)?;
            let mime = mime.trim();
            if !mime.contains('/') || mime.chars().any(char::is_whitespace) {
                return Err(CoreError::Invalid);
            }
            Ok(mime.to_owned())
        })
        .await
        .map_err(|_| CoreError::Overloaded)?
    }
    fn aws_command(&self) -> Result<Command, CoreError> {
        if !self.enabled
            || !self.aws.is_absolute()
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
        Ok(command)
    }
    fn storage_command(&self, operation: &str, key: &str) -> Result<Command, CoreError> {
        let mut command = self.aws_command()?;
        command.args([
            "s3api",
            operation,
            "--bucket",
            &self.bucket,
            if operation == "list-object-versions" {
                "--prefix"
            } else {
                "--key"
            },
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
    bounded_read_limit(path, MAX_DOCUMENT_BYTES)
}
fn bounded_read_limit(path: &Path, limit: u64) -> Result<Vec<u8>, CoreError> {
    use std::io::Read;
    let file = fs::File::open(path).map_err(|_| CoreError::Overloaded)?;
    let mut bytes = Vec::new();
    file.take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| CoreError::Overloaded)?;
    if bytes.len() as u64 > limit {
        return Err(CoreError::Invalid);
    }
    Ok(bytes)
}
fn run_bounded_download(
    mut command: Command,
    mut output: fs::File,
    expected_size: u64,
    expected_digest: &[u8],
    timeout: Duration,
) -> Result<(), CoreError> {
    use std::io::{Read, Write};
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let mut child = command.spawn().map_err(|_| CoreError::Overloaded)?;
    let mut stdout = child.stdout.take().ok_or(CoreError::Overloaded)?;
    let (send, receive) = std::sync::mpsc::sync_channel(1);
    let reader = std::thread::spawn(move || {
        let result = (|| -> Result<(u64, Vec<u8>), ()> {
            let mut total = 0u64;
            let mut hash = Sha256::new();
            let mut buffer = vec![0u8; 256 * 1024];
            loop {
                let n = stdout.read(&mut buffer).map_err(|_| ())?;
                if n == 0 {
                    break;
                }
                total = total
                    .checked_add(n as u64)
                    .filter(|n| *n <= expected_size)
                    .ok_or(())?;
                output.write_all(&buffer[..n]).map_err(|_| ())?;
                hash.update(&buffer[..n]);
            }
            Ok((total, hash.finalize().to_vec()))
        })();
        let _ = send.send(result);
    });
    let started = Instant::now();
    let mut result = None;
    let mut status = None;
    loop {
        if result.is_none() {
            match receive.try_recv() {
                Ok(value) => result = Some(value),
                Err(std::sync::mpsc::TryRecvError::Disconnected) => result = Some(Err(())),
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
            }
        }
        if result.as_ref().is_some_and(Result::is_err) {
            let _ = child.kill();
            let _ = child.wait();
            let _ = reader.join();
            return Err(CoreError::Invalid);
        }
        if status.is_none() {
            status = match child.try_wait() {
                Ok(status) => status,
                Err(_) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    drop(reader);
                    return Err(CoreError::Overloaded);
                }
            };
        }
        if let (Some(result), Some(status)) = (result.as_ref(), status.as_ref()) {
            let _ = reader.join();
            let (size, digest) = result.as_ref().map_err(|_| CoreError::Invalid)?;
            return if status.success()
                && *size == expected_size
                && digest.as_slice() == expected_digest
            {
                Ok(())
            } else {
                Err(CoreError::Overloaded)
            };
        }
        if started.elapsed() >= timeout {
            let _ = child.kill();
            let _ = child.wait();
            // Dropping a thread handle avoids blocking the request on an
            // unexpected descendant retaining the pipe after timeout.
            drop(reader);
            return Err(CoreError::Overloaded);
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}
fn run_file_input(
    mut command: Command,
    input: fs::File,
    timeout: Duration,
) -> Result<(), CoreError> {
    command
        .stdin(Stdio::from(input))
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
fn run(command: Command, timeout: Duration) -> Result<(), CoreError> {
    run_output(command, timeout, None)
}
fn run_output(
    mut command: Command,
    timeout: Duration,
    output: Option<fs::File>,
) -> Result<(), CoreError> {
    command
        .stdin(Stdio::null())
        .stdout(output.map(Stdio::from).unwrap_or_else(Stdio::null))
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
    if !(key.starts_with("cotizaciones/")
        || key.starts_with("colaboracion/")
        || key.starts_with("ordenes_compra/")
        || key.starts_with("facturas/")
        || key.starts_with("licitaciones/"))
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
        assert!(validate_key("ordenes_compra/12/products/abc").is_ok());
        assert!(validate_key("facturas/12/3/invoice-v1.pdf").is_ok());
        assert!(validate_key("licitaciones/12/files/abc-123").is_ok());
        assert!(validate_key("licitaciones/../secret").is_err());
        assert!(validate_key("facturas/../secret").is_err());
        assert!(validate_key("ordenes_compra/../secret").is_err());
        assert!(validate_key("colaboracion/empresa/abc-123").is_ok());
        assert!(validate_key("colaboracion/personas/12/abc-123").is_ok());
        for value in [
            "/cotizaciones/12.pdf",
            "cotizaciones/../secret",
            "cotizaciones//12",
            "cotizaciones/./12",
            "core/12",
            "colaboracion/../secret",
            "cotizaciones/12?acl=public-read",
            "cotizaciones/12\\secret",
        ] {
            assert!(validate_key(value).is_err(), "{value}");
        }
    }
    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn tender_pdf_upload_streams_and_checks_bytes_without_large_allocations() {
        use std::os::unix::fs::PermissionsExt;
        let root = std::env::temp_dir().join(format!(
            "bastion-stream-test-{}",
            DocumentService::object_suffix()
        ));
        fs::create_dir_all(&root).unwrap();
        let mock = root.join("aws");
        fs::write(
            &mock,
            br##"#!/usr/bin/python3
import sys, hashlib, base64
args=sys.argv[1:]
def opt(name): return args[args.index(name)+1]
assert args[:2]==['s3api','put-object']
assert opt('--body')=='/dev/stdin' and opt('--server-side-encryption')=='AES256'
assert opt('--checksum-algorithm')=='SHA256' and opt('--key')=='licitaciones/123/pdf/output'
h=hashlib.sha256(); length=0; first=b''
body=open(opt('--body'),'rb')
while True:
 chunk=body.read(65536)
 if not chunk:break
 if not first:first=chunk[:5]
 length+=len(chunk);h.update(chunk)
assert first==b'%PDF-' and length==int(opt('--content-length'))
assert base64.b64encode(h.digest()).decode()==opt('--checksum-sha256')
"##,
        )
        .unwrap();
        fs::set_permissions(&mock, fs::Permissions::from_mode(0o700)).unwrap();
        let service = DocumentService {
            enabled: true,
            typst: PathBuf::new(),
            aws: mock,
            work: root.clone(),
            branding: None,
            bucket: "synthetic".into(),
            region: "us-east-1".into(),
        };
        let path = root.join("document.pdf");
        let mut content = vec![b'x'; 2_000_000];
        content[..5].copy_from_slice(b"%PDF-");
        fs::write(&path, &content).unwrap();
        let (size, hash) = service
            .put_tender_pdf_file(
                "licitaciones/123/pdf/output".into(),
                fs::File::open(&path).unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(size, content.len() as u64);
        assert_eq!(hash, hex::encode(Sha256::digest(&content)));
        let invalid = root.join("invalid.pdf");
        fs::write(&invalid, b"not a pdf").unwrap();
        assert!(
            service
                .put_tender_pdf_file(
                    "licitaciones/123/pdf/output".into(),
                    fs::File::open(&invalid).unwrap()
                )
                .await
                .is_err()
        );
        let big = root.join("big.pdf");
        let file = fs::File::create(&big).unwrap();
        file.set_len(1_000_000_001).unwrap();
        drop(file);
        assert!(
            service
                .put_tender_pdf_file(
                    "licitaciones/123/pdf/output".into(),
                    fs::File::open(&big).unwrap()
                )
                .await
                .is_err()
        );
        assert!(
            service
                .put_tender_pdf_file(
                    "cotizaciones/123/pdf/output".into(),
                    fs::File::open(&path).unwrap()
                )
                .await
                .is_err()
        );
        fs::remove_dir_all(root).unwrap();
    }
    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn tender_pdf_download_is_bounded_and_checks_integrity() {
        use std::io::Read;
        use std::os::unix::fs::PermissionsExt;
        let root = std::env::temp_dir().join(format!(
            "bastion-download-test-{}",
            DocumentService::object_suffix()
        ));
        fs::create_dir_all(&root).unwrap();
        let mock = root.join("aws");
        fs::write(
            &mock,
            br##"#!/usr/bin/python3
import os, sys
a=sys.argv[1:]
assert a[:4]==['s3','cp','s3://synthetic/licitaciones/123/pdf/output','-']
assert '--only-show-errors' in a
os.write(1,b'%PDF-'+b'x'*1000000)
"##,
        )
        .unwrap();
        fs::set_permissions(&mock, fs::Permissions::from_mode(0o700)).unwrap();
        let service = DocumentService {
            enabled: true,
            typst: PathBuf::new(),
            aws: mock,
            work: root.clone(),
            branding: None,
            bucket: "synthetic".into(),
            region: "us-east-1".into(),
        };
        let mut data = vec![b'x'; 1_000_005];
        data[..5].copy_from_slice(b"%PDF-");
        let digest = hex::encode(Sha256::digest(&data));
        let open = |name: &str| {
            fs::OpenOptions::new()
                .read(true)
                .write(true)
                .create_new(true)
                .open(root.join(name))
                .unwrap()
        };
        let mut file = service
            .get_tender_pdf_file(
                "licitaciones/123/pdf/output".into(),
                open("ok"),
                data.len() as u64,
                digest.clone(),
            )
            .await
            .unwrap();
        let mut actual = Vec::new();
        file.read_to_end(&mut actual).unwrap();
        assert_eq!(actual, data);
        assert!(
            service
                .get_tender_pdf_file(
                    "licitaciones/123/pdf/output".into(),
                    open("wrong-hash"),
                    data.len() as u64,
                    "0".repeat(64)
                )
                .await
                .is_err()
        );
        assert!(
            service
                .get_tender_pdf_file(
                    "licitaciones/123/pdf/output".into(),
                    open("too-long"),
                    (data.len() - 1) as u64,
                    digest
                )
                .await
                .is_err()
        );
        assert!(fs::metadata(root.join("too-long")).unwrap().len() <= (data.len() - 1) as u64);
        assert!(
            service
                .get_tender_pdf_file(
                    "licitaciones/../secret".into(),
                    open("bad-key"),
                    data.len() as u64,
                    "0".repeat(64)
                )
                .await
                .is_err()
        );
        fs::remove_dir_all(root).unwrap();
    }
}
