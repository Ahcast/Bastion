use std::{collections::BTreeMap, path::Path};

use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::Value;

use crate::{LocalDeliveryError, local::read_regular};

const REPORTS: [&str; 4] = ["main", "scout", "combiner", "clasificador"];
const MAX_STAGE_BYTES: usize = 512 * 1024;

#[derive(Serialize)]
pub struct ScoutDiagnostics {
    pub checked_at: DateTime<Utc>,
    pub reports: Vec<ScoutReport>,
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReportAvailability {
    Available,
    Missing,
    Unreadable,
    Invalid,
    Changed,
}

#[derive(Serialize)]
pub struct ScoutReport {
    pub stage: &'static str,
    pub availability: ReportAvailability,
    pub matches_current_run: Option<bool>,
    pub run: Option<ScoutRun>,
}

#[derive(Serialize)]
pub struct ScoutRun {
    pub run_id: String,
    pub status: &'static str,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub deadline: Option<DateTime<Utc>>,
    pub duration_seconds: Option<u64>,
    pub output_ready: Option<bool>,
    pub reason: Option<&'static str>,
    pub retry_stages: Vec<&'static str>,
    pub metrics: BTreeMap<&'static str, u64>,
    pub error_reported: bool,
}

impl ScoutDiagnostics {
    /// Fixed sibling names only; never accepts report names or paths from HTTP/MCP.
    pub fn read(csv_path: &Path) -> Self {
        let main_path = csv_path
            .parent()
            .map(|directory| directory.join("main.json"));
        let before = main_path
            .as_ref()
            .and_then(|path| read_regular(path, 32 * 1024).ok());
        let reports = REPORTS
            .iter()
            .map(|stage| {
                let result = csv_path
                    .parent()
                    .ok_or(LocalDeliveryError::Invalid)
                    .and_then(|directory| read_report(directory, stage));
                match result {
                    Ok(run) => ScoutReport {
                        stage,
                        availability: ReportAvailability::Available,
                        matches_current_run: None,
                        run: Some(run),
                    },
                    Err(error) => ScoutReport {
                        stage,
                        availability: availability(error),
                        matches_current_run: None,
                        run: None,
                    },
                }
            })
            .collect::<Vec<_>>();
        let mut result = Self {
            checked_at: Utc::now(),
            reports,
        };
        let after = main_path
            .as_ref()
            .and_then(|path| read_regular(path, 32 * 1024).ok());
        if result.reports[0].availability == ReportAvailability::Available
            && (before.is_none() || before != after)
        {
            result.reports[0].availability = ReportAvailability::Changed;
            result.reports[0].run = None;
        }
        // Separate stages may be from an older cycle, especially during retries.
        let current = result.reports[0].run.as_ref().map(|run| run.run_id.clone());
        for report in &mut result.reports {
            report.matches_current_run = current
                .as_ref()
                .zip(report.run.as_ref())
                .map(|(id, run)| *id == run.run_id);
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Files(std::path::PathBuf);
    impl Files {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "bastion-scout-status-{}-{}",
                std::process::id(),
                Utc::now().timestamp_nanos_opt().unwrap()
            ));
            std::fs::create_dir(&path).unwrap();
            Self(path)
        }
        fn write(&self, stage: &str, extra: Value) {
            let mut value = serde_json::json!({"stage":stage,"run_id":"2026-10-22","status":"exitoso","started_at":"2026-10-22T19:00:00-06:00","finished_at":"2026-10-22T19:02:00-06:00"});
            value
                .as_object_mut()
                .unwrap()
                .extend(extra.as_object().unwrap().clone());
            std::fs::write(self.0.join(format!("{stage}.json")), value.to_string()).unwrap();
        }
        fn read(&self) -> ScoutDiagnostics {
            ScoutDiagnostics::read(&self.0.join("maestro.csv"))
        }
    }
    impl Drop for Files {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn reports_stages_missing_data_and_other_cycles_without_reading_or_changing_csv() {
        let files = Files::new();
        assert!(
            files
                .read()
                .reports
                .iter()
                .all(|r| r.availability == ReportAvailability::Missing)
        );
        files.write(
            "main",
            serde_json::json!({"output_ready":true,"total":20,"retry_stages":["scout"]}),
        );
        files.write("scout",serde_json::json!({"run_id":"2026-10-21","pages":3,"pending":[{"url":"PRIVATE_URL"}],"detail_errors":{"PRIVATE_URL":"PRIVATE_ERROR"}}));
        files.write("clasificador",serde_json::json!({"analyzed":0,"pending":2,"usage":{"input_tokens":100,"output_tokens":20}}));
        let before = std::fs::read(files.0.join("main.json")).unwrap();
        let report = files.read();
        assert_eq!(report.reports[0].matches_current_run, Some(true));
        assert_eq!(report.reports[1].matches_current_run, Some(false));
        assert!(report.reports[2].run.is_none());
        let ai = report.reports[3].run.as_ref().unwrap();
        assert_eq!(ai.metrics["analyzed"], 0);
        assert_eq!(ai.metrics["input_tokens"], 100);
        assert!(!ai.metrics.contains_key("reused"));
        assert_eq!(ai.duration_seconds, Some(120));
        assert_eq!(std::fs::read(files.0.join("main.json")).unwrap(), before);
        assert!(!files.0.join("maestro.csv").exists());
        let text = serde_json::to_string(&report).unwrap();
        assert!(!text.contains("PRIVATE"));
    }

    #[test]
    fn invalid_fields_unknown_reasons_and_free_form_errors_cannot_leak() {
        let files = Files::new();
        files.write("main",serde_json::json!({"status":"error","reason":"SECRET_REASON","error":"SECRET_EXCEPTION","api_key":"SECRET_KEY","output_ready":false}));
        files.write("scout", serde_json::json!({"pages":-1}));
        std::fs::write(files.0.join("combiner.json"), "{").unwrap();
        let report = files.read();
        let main = report.reports[0].run.as_ref().unwrap();
        assert!(main.error_reported);
        assert_eq!(main.reason, Some("other"));
        assert!(report.reports[1..].iter().all(|r| r.run.is_none()));
        assert!(!serde_json::to_string(&report).unwrap().contains("SECRET"));
        files.write("main", serde_json::json!({"stage":"scout"}));
        assert!(files.read().reports[0].run.is_none());
        files.write("main", serde_json::json!({"started_at":"not-a-date"}));
        assert!(files.read().reports[0].run.is_none());
    }

    #[test]
    fn oversize_files_and_symlinks_are_not_reports() {
        let files = Files::new();
        std::fs::write(files.0.join("main.json"), vec![b' '; 32 * 1024 + 1]).unwrap();
        std::fs::write(files.0.join("scout.json"), vec![b' '; MAX_STAGE_BYTES + 1]).unwrap();
        assert!(
            files.read().reports[0..2]
                .iter()
                .all(|r| r.availability == ReportAvailability::Invalid)
        );
        #[cfg(unix)]
        {
            files.write("main", serde_json::json!({}));
            std::os::unix::fs::symlink(files.0.join("main.json"), files.0.join("combiner.json"))
                .unwrap();
            assert!(files.read().reports[2].run.is_none());
        }
    }
}

fn availability(error: LocalDeliveryError) -> ReportAvailability {
    match error {
        LocalDeliveryError::Missing => ReportAvailability::Missing,
        LocalDeliveryError::Io => ReportAvailability::Unreadable,
        LocalDeliveryError::Changed => ReportAvailability::Changed,
        _ => ReportAvailability::Invalid,
    }
}

fn read_report(directory: &Path, stage: &'static str) -> Result<ScoutRun, LocalDeliveryError> {
    let path = directory.join(format!("{stage}.json"));
    let limit = if stage == "main" {
        32 * 1024
    } else {
        MAX_STAGE_BYTES
    };
    let bytes = read_regular(&path, limit)?;
    let value: Value = serde_json::from_slice(&bytes).map_err(|_| LocalDeliveryError::Invalid)?;
    let run = project(&value, stage)?;
    if read_regular(&path, limit)? != bytes {
        return Err(LocalDeliveryError::Changed);
    }
    Ok(run)
}

fn state(value: &str) -> &'static str {
    match value {
        "ejecutando" => "ejecutando",
        "exitoso" => "exitoso",
        "parcial" => "parcial",
        "error" => "error",
        _ => "desconocido",
    }
}

fn reason(value: &str) -> &'static str {
    match value {
        "ciclo_diario_completo" => "ciclo_diario_completo",
        "etapas_pendientes" => "etapas_pendientes",
        "recuperacion_anterior_pendiente" => "recuperacion_anterior_pendiente",
        "combinacion_pendiente" => "combinacion_pendiente",
        "error_pipeline" => "error_pipeline",
        "fuera_de_ventana" => "fuera_de_ventana",
        "recuperado_desde_journal" => "recuperado_desde_journal",
        "10_vencidos_consecutivos" => "10_vencidos_consecutivos",
        "fin_resultados_verificado" => "fin_resultados_verificado",
        "limite_horario" => "limite_horario",
        "error_escaneo" => "error_escaneo",
        "maestro_actualizado" => "maestro_actualizado",
        "error_combinacion" => "error_combinacion",
        "analisis_completo" => "analisis_completo",
        "analisis_pendiente" => "analisis_pendiente",
        "error_clasificacion" => "error_clasificacion",
        _ => "other",
    }
}

fn project(value: &Value, stage: &'static str) -> Result<ScoutRun, LocalDeliveryError> {
    if value.get("stage").and_then(Value::as_str) != Some(stage) {
        return Err(LocalDeliveryError::Invalid);
    }
    let id = value
        .get("run_id")
        .and_then(Value::as_str)
        .ok_or(LocalDeliveryError::Invalid)?;
    if id.is_empty()
        || id.len() > 120
        || !id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"-_.".contains(&c))
    {
        return Err(LocalDeliveryError::Invalid);
    }
    let status = value
        .get("status")
        .and_then(Value::as_str)
        .map(state)
        .ok_or(LocalDeliveryError::Invalid)?;
    let date = |key| -> Result<Option<DateTime<Utc>>, LocalDeliveryError> {
        match value.get(key) {
            None | Some(Value::Null) => Ok(None),
            Some(Value::String(text)) if text.len() <= 64 => DateTime::parse_from_rfc3339(text)
                .map(|d| Some(d.with_timezone(&Utc)))
                .map_err(|_| LocalDeliveryError::Invalid),
            _ => Err(LocalDeliveryError::Invalid),
        }
    };
    let started_at = date("started_at")?;
    let finished_at = date("finished_at")?;
    let duration_seconds = started_at
        .zip(finished_at)
        .and_then(|(start, end)| u64::try_from((end - start).num_seconds()).ok());
    let mut metrics = BTreeMap::new();
    let keys: &[&'static str] = match stage {
        "main" => &["total"],
        "scout" => &[
            "pages",
            "extracted",
            "expired",
            "consecutive_expired",
            "known_rechecked",
            "http_requests",
            "http_retries",
        ],
        "combiner" => &[
            "total",
            "inserted",
            "changed",
            "unchanged",
            "removed_expired",
            "removed_retention",
            "suppressed_retired",
        ],
        _ => &[
            "total",
            "analyzed",
            "reused",
            "unchanged",
            "pending",
            "errors",
        ],
    };
    for key in keys {
        if let Some(value) = value.get(key).filter(|v| !v.is_null()) {
            metrics.insert(*key, value.as_u64().ok_or(LocalDeliveryError::Invalid)?);
        }
    }
    if stage == "scout" {
        if let Some(pending) = value.get("pending").and_then(Value::as_array) {
            metrics.insert("pending", pending.len() as u64);
        }
        if let Some(errors) = value.get("detail_errors").and_then(Value::as_object) {
            metrics.insert("detail_errors", errors.len() as u64);
        }
    }
    if stage == "clasificador" {
        for key in ["input_tokens", "output_tokens"] {
            if let Some(tokens) = value
                .get("usage")
                .and_then(|v| v.get(key))
                .filter(|v| !v.is_null())
            {
                metrics.insert(key, tokens.as_u64().ok_or(LocalDeliveryError::Invalid)?);
            }
        }
    }
    let retry_stages = REPORTS[1..]
        .iter()
        .filter(|name| {
            value
                .get("retry_stages")
                .and_then(Value::as_array)
                .is_some_and(|array| array.iter().any(|item| item.as_str() == Some(**name)))
        })
        .copied()
        .collect();
    let error_reported = ["error", "stop_reason"].iter().any(|key| {
        value
            .get(key)
            .is_some_and(|v| !v.is_null() && v.as_str() != Some(""))
    }) || metrics.get("errors").is_some_and(|n| *n > 0)
        || metrics.get("detail_errors").is_some_and(|n| *n > 0);
    Ok(ScoutRun {
        run_id: id.into(),
        status,
        started_at,
        finished_at,
        deadline: date("deadline")?,
        duration_seconds,
        output_ready: if stage == "main" {
            value.get("output_ready").and_then(Value::as_bool)
        } else {
            None
        },
        reason: value.get("reason").and_then(Value::as_str).map(reason),
        retry_stages,
        metrics,
        error_reported,
    })
}
