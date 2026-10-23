use std::{
    collections::BTreeMap,
    fs::{self, OpenOptions},
    io::Read,
    path::Path,
};

use crate::{MAX_CSV_BYTES, MAX_ROWS, parse_csv};
use serde::Deserialize;
use sha2::{Digest, Sha256};

pub const DEFAULT_SCOUT_CSV: &str = "/var/lib/bastion-scout/maestro.csv";
const MAX_MANIFEST_BYTES: usize = 32 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalDeliveryError {
    Missing,
    Pending,
    Invalid,
    Changed,
    Io,
}

impl LocalDeliveryError {
    pub fn code(self) -> &'static str {
        match self {
            Self::Missing => "missing",
            Self::Pending => "pending",
            Self::Invalid => "invalid",
            Self::Changed => "changed",
            Self::Io => "io",
        }
    }
}

#[derive(Deserialize)]
struct Manifest {
    status: String,
    output_ready: bool,
    run_id: String,
    maestro_sha256: Option<String>,
    total: Option<usize>,
    stages: BTreeMap<String, String>,
}

/// A bounded, verified snapshot. File paths never come from HTTP/MCP input.
pub struct ReadyDelivery {
    pub(crate) csv: String,
    pub(crate) run_id: String,
    pub(crate) total: usize,
    sha256: String,
}

impl ReadyDelivery {
    pub fn sha256(&self) -> &str {
        &self.sha256
    }

    pub fn read(csv_path: &Path) -> Result<Self, LocalDeliveryError> {
        let manifest_path = csv_path
            .parent()
            .ok_or(LocalDeliveryError::Invalid)?
            .join("main.json");
        let before = read_regular(&manifest_path, MAX_MANIFEST_BYTES)?;
        let manifest: Manifest =
            serde_json::from_slice(&before).map_err(|_| LocalDeliveryError::Invalid)?;
        if !manifest.output_ready || manifest.status != "exitoso" {
            return Err(LocalDeliveryError::Pending);
        }
        if ["scout", "combiner", "clasificador"]
            .iter()
            .any(|stage| manifest.stages.get(*stage).map(String::as_str) != Some("exitoso"))
        {
            return Err(LocalDeliveryError::Pending);
        }
        if manifest.run_id.trim().is_empty() || manifest.run_id.len() > 120 {
            return Err(LocalDeliveryError::Invalid);
        }
        let hash = manifest.maestro_sha256.ok_or(LocalDeliveryError::Invalid)?;
        if hash.len() != 64
            || !hash
                .bytes()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
        {
            return Err(LocalDeliveryError::Invalid);
        }
        let total = manifest
            .total
            .filter(|n| *n <= MAX_ROWS)
            .ok_or(LocalDeliveryError::Invalid)?;
        let bytes = read_regular(csv_path, MAX_CSV_BYTES)?;
        if hex::encode(Sha256::digest(&bytes)) != hash {
            return Err(LocalDeliveryError::Changed);
        }
        if read_regular(&manifest_path, MAX_MANIFEST_BYTES)? != before {
            return Err(LocalDeliveryError::Changed);
        }
        let csv = String::from_utf8(bytes).map_err(|_| LocalDeliveryError::Invalid)?;
        if parse_csv(&csv)
            .map_err(|_| LocalDeliveryError::Invalid)?
            .len()
            != total
        {
            return Err(LocalDeliveryError::Invalid);
        }
        Ok(Self {
            csv,
            run_id: manifest.run_id,
            total,
            sha256: hash,
        })
    }
}

pub(crate) fn read_regular(path: &Path, limit: usize) -> Result<Vec<u8>, LocalDeliveryError> {
    let metadata = fs::symlink_metadata(path).map_err(file_error)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() > limit as u64 {
        return Err(LocalDeliveryError::Invalid);
    }
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let file = options.open(path).map_err(file_error)?;
    let opened = file.metadata().map_err(file_error)?;
    if !opened.is_file() || opened.len() > limit as u64 {
        return Err(LocalDeliveryError::Invalid);
    }
    let mut bytes = Vec::new();
    file.take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(file_error)?;
    if bytes.len() > limit {
        return Err(LocalDeliveryError::Invalid);
    }
    Ok(bytes)
}

fn file_error(error: std::io::Error) -> LocalDeliveryError {
    if error.kind() == std::io::ErrorKind::NotFound {
        LocalDeliveryError::Missing
    } else {
        LocalDeliveryError::Io
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Files(PathBuf);
    use std::path::PathBuf;
    impl Files {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "bastion-delivery-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
        fn publish(&self, csv: &str, total: usize, ready: bool) {
            fs::write(self.0.join("maestro.csv"), csv).unwrap();
            let manifest = serde_json::json!({"status":"exitoso","output_ready":ready,"run_id":"2026-10-22","maestro_sha256":hex::encode(Sha256::digest(csv.as_bytes())),"total":total,"stages":{"scout":"exitoso","combiner":"exitoso","clasificador":"exitoso"}});
            fs::write(self.0.join("main.json"), manifest.to_string()).unwrap();
        }
    }
    impl Drop for Files {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn only_complete_matching_snapshots_are_accepted_and_source_is_unchanged() {
        let files = Files::new();
        let path = files.0.join("maestro.csv");
        let csv = crate::scout::tests::fixture(None);
        assert!(matches!(
            ReadyDelivery::read(&path),
            Err(LocalDeliveryError::Missing)
        ));
        files.publish(&csv, 1, false);
        assert!(matches!(
            ReadyDelivery::read(&path),
            Err(LocalDeliveryError::Pending)
        ));
        files.publish(&csv, 2, true);
        assert!(matches!(
            ReadyDelivery::read(&path),
            Err(LocalDeliveryError::Invalid)
        ));
        files.publish(&csv, 1, true);
        let delivery = ReadyDelivery::read(&path).unwrap();
        assert_eq!(delivery.csv, csv);
        assert_eq!(fs::read_to_string(&path).unwrap(), csv);
        fs::write(&path, csv.clone() + "\n").unwrap();
        assert!(matches!(
            ReadyDelivery::read(&path),
            Err(LocalDeliveryError::Changed)
        ));
        files.publish(&csv, 1, true);
        let partial = crate::scout::tests::fixture(Some(("analisis_estado", "pendiente")));
        files.publish(&partial, 1, true);
        assert!(matches!(
            ReadyDelivery::read(&path),
            Err(LocalDeliveryError::Invalid)
        ));
        fs::write(
            files.0.join("main.json"),
            "x".repeat(MAX_MANIFEST_BYTES + 1),
        )
        .unwrap();
        assert!(matches!(
            ReadyDelivery::read(&path),
            Err(LocalDeliveryError::Invalid)
        ));
        files.publish(&csv, 1, true);
        fs::write(&path, vec![b'x'; MAX_CSV_BYTES + 1]).unwrap();
        assert!(matches!(
            ReadyDelivery::read(&path),
            Err(LocalDeliveryError::Invalid)
        ));
    }

    #[cfg(unix)]
    #[test]
    fn symbolic_links_cannot_redirect_the_reader() {
        let files = Files::new();
        let csv = crate::scout::tests::fixture(None);
        files.publish(&csv, 1, true);
        let path = files.0.join("maestro.csv");
        fs::rename(&path, files.0.join("other.csv")).unwrap();
        std::os::unix::fs::symlink(files.0.join("other.csv"), &path).unwrap();
        assert!(matches!(
            ReadyDelivery::read(&path),
            Err(LocalDeliveryError::Invalid)
        ));
    }
}
