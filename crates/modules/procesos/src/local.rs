use std::{
    collections::BTreeMap,
    fs::{self, OpenOptions},
    io::Read,
    path::Path,
};

use crate::{FIELDS, MAX_CSV_BYTES, MAX_ROWS, SCOUT_FIELDS, parse_csv};
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
    pub(crate) source_total: usize,
    pub(crate) deferred: usize,
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
        let partial = manifest.status == "parcial" && !manifest.output_ready;
        let complete = manifest.status == "exitoso" && manifest.output_ready;
        let accepted_stage = |name: &str| {
            matches!(
                manifest.stages.get(name).map(String::as_str),
                Some("exitoso")
            ) || (partial && manifest.stages.get(name).map(String::as_str) == Some("parcial"))
        };
        if (!partial && !complete)
            || manifest.stages.get("combiner").map(String::as_str) != Some("exitoso")
            || !accepted_stage("scout")
            || !accepted_stage("clasificador")
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
        let source_csv = String::from_utf8(bytes).map_err(|_| LocalDeliveryError::Invalid)?;
        let (csv, ready) = if partial {
            analyzed_subset(&source_csv, total)?
        } else {
            (source_csv, total)
        };
        if parse_csv(&csv)
            .map_err(|_| LocalDeliveryError::Invalid)?
            .len()
            != ready
        {
            return Err(LocalDeliveryError::Invalid);
        }
        Ok(Self {
            csv,
            run_id: manifest.run_id,
            total: ready,
            source_total: total,
            deferred: total - ready,
            sha256: hash,
        })
    }
}

fn analyzed_subset(source: &str, expected: usize) -> Result<(String, usize), LocalDeliveryError> {
    if source.contains('\0') {
        return Err(LocalDeliveryError::Invalid);
    }
    let mut reader = csv::Reader::from_reader(source.trim_start_matches('\u{feff}').as_bytes());
    let headers = reader
        .headers()
        .map_err(|_| LocalDeliveryError::Invalid)?
        .clone();
    if !headers.iter().eq(FIELDS) && !headers.iter().eq(SCOUT_FIELDS) {
        return Err(LocalDeliveryError::Invalid);
    }
    let mut writer = csv::Writer::from_writer(Vec::new());
    writer
        .write_record(&headers)
        .map_err(|_| LocalDeliveryError::Invalid)?;
    let mut total = 0;
    let mut ready = 0;
    for record in reader.records() {
        total += 1;
        if total > MAX_ROWS {
            return Err(LocalDeliveryError::Invalid);
        }
        let record = record.map_err(|_| LocalDeliveryError::Invalid)?;
        // Both supported schemas have identical status/hash column positions.
        if record.get(10) == Some("completo")
            && record.get(20) == Some("analizado")
            && record.get(11).is_some_and(|hash| !hash.is_empty())
            && record.get(19) == record.get(11)
            && record.get(23) == Some("")
        {
            writer
                .write_record(&record)
                .map_err(|_| LocalDeliveryError::Invalid)?;
            ready += 1;
        }
    }
    if total != expected {
        return Err(LocalDeliveryError::Invalid);
    }
    if ready == 0 {
        return Err(LocalDeliveryError::Pending);
    }
    // Preserve the exact CSV and its existing receipt when every row is already ready.
    if ready == total {
        return Ok((source.to_owned(), ready));
    }
    let selected = writer
        .into_inner()
        .map_err(|_| LocalDeliveryError::Invalid)?;
    Ok((
        String::from_utf8(selected).map_err(|_| LocalDeliveryError::Invalid)?,
        ready,
    ))
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

        fn publish_partial(&self, csv: &str, total: usize) {
            self.publish(csv, total, false);
            let path = self.0.join("main.json");
            let mut manifest: serde_json::Value =
                serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
            manifest["status"] = serde_json::json!("parcial");
            manifest["stages"]["scout"] = serde_json::json!("parcial");
            manifest["stages"]["clasificador"] = serde_json::json!("parcial");
            fs::write(path, manifest.to_string()).unwrap();
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

    fn mixed(fixtures: &[String]) -> String {
        let mut writer = csv::Writer::from_writer(Vec::new());
        writer.write_record(FIELDS).unwrap();
        for (index, fixture) in fixtures.iter().enumerate() {
            let mut reader =
                csv::Reader::from_reader(fixture.trim_start_matches('\u{feff}').as_bytes());
            for record in reader.records() {
                let mut fields = record
                    .unwrap()
                    .iter()
                    .map(str::to_owned)
                    .collect::<Vec<_>>();
                fields[1] = format!("hc:{index:064x}");
                writer.write_record(fields).unwrap();
            }
        }
        String::from_utf8(writer.into_inner().unwrap()).unwrap()
    }

    #[test]
    fn partial_snapshot_imports_only_ready_rows_without_changing_the_source() {
        let files = Files::new();
        let ready = crate::scout::tests::fixture(None);
        let pending = crate::scout::tests::fixture(Some(("analisis_estado", "pendiente")));
        let incomplete = crate::scout::tests::fixture(Some(("extraccion_estado", "incompleto")));
        let stale = crate::scout::tests::fixture(Some(("hash_analizado", &"c".repeat(64))));
        let error = crate::scout::tests::fixture(Some(("analisis_error", "error")));
        let source = mixed(&[ready.clone(), pending, incomplete, stale, error]);
        files.publish_partial(&source, 5);
        let delivery = ReadyDelivery::read(&files.0.join("maestro.csv")).unwrap();
        assert_eq!(delivery.total, 1);
        assert_eq!(delivery.source_total, 5);
        assert_eq!(delivery.deferred, 4);
        assert_eq!(parse_csv(&delivery.csv).unwrap().len(), 1);
        assert_eq!(
            fs::read_to_string(files.0.join("maestro.csv")).unwrap(),
            source
        );
        assert_eq!(
            delivery.sha256(),
            hex::encode(Sha256::digest(source.as_bytes()))
        );

        // An analysis finishes on the next run; the new eligible row joins the delivery.
        let next = mixed(&[ready.clone(), ready]);
        files.publish_partial(&next, 2);
        let updated = ReadyDelivery::read(&files.0.join("maestro.csv")).unwrap();
        assert_eq!(updated.total, 2);
        assert_eq!(updated.deferred, 0);
        assert_eq!(updated.csv, next);
    }

    #[test]
    fn partial_delivery_still_requires_integrity_and_rejects_invalid_ready_rows() {
        let files = Files::new();
        let path = files.0.join("maestro.csv");
        let csv = crate::scout::tests::fixture(None);
        files.publish_partial(&csv, 2);
        assert!(matches!(
            ReadyDelivery::read(&path),
            Err(LocalDeliveryError::Invalid)
        ));
        files.publish_partial(&csv, 1);
        fs::write(&path, csv.clone() + "\n").unwrap();
        assert!(matches!(
            ReadyDelivery::read(&path),
            Err(LocalDeliveryError::Changed)
        ));
        let bad = crate::scout::tests::fixture(Some(("puntaje", "101")));
        files.publish_partial(&bad, 1);
        assert!(matches!(
            ReadyDelivery::read(&path),
            Err(LocalDeliveryError::Invalid)
        ));
        let pending = crate::scout::tests::fixture(Some(("analisis_estado", "pendiente")));
        files.publish_partial(&pending, 1);
        assert!(matches!(
            ReadyDelivery::read(&path),
            Err(LocalDeliveryError::Pending)
        ));
        files.publish_partial(&csv, 1);
        let manifest_path = files.0.join("main.json");
        let mut manifest: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&manifest_path).unwrap()).unwrap();
        manifest["status"] = serde_json::json!("ejecutando");
        fs::write(&manifest_path, manifest.to_string()).unwrap();
        assert!(matches!(
            ReadyDelivery::read(&path),
            Err(LocalDeliveryError::Pending)
        ));
    }

    #[test]
    fn changes_to_pending_rows_preserve_the_selected_receipt_and_multiline_values() {
        let files = Files::new();
        let ready = crate::scout::tests::fixture(Some((
            "descripcion",
            "Texto, con comillas \"y\"\nsegunda línea",
        )));
        let pending = crate::scout::tests::fixture(Some(("analisis_estado", "pendiente")));
        let error = crate::scout::tests::fixture(Some(("analisis_estado", "error")));
        files.publish_partial(&mixed(&[ready.clone(), pending]), 2);
        let first = ReadyDelivery::read(&files.0.join("maestro.csv")).unwrap();
        files.publish_partial(&mixed(&[ready, error]), 2);
        let next = ReadyDelivery::read(&files.0.join("maestro.csv")).unwrap();
        assert_ne!(first.sha256(), next.sha256());
        assert_eq!(first.csv, next.csv);
        assert!(first.csv.contains("segunda línea"));
        assert_eq!(parse_csv(&next.csv).unwrap().len(), 1);
    }
}
