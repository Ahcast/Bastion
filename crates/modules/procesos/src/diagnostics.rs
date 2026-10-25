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
    pub extraction: Option<ScoutExtraction>,
}

#[derive(Serialize)]
pub struct ScoutIssue {
    pub code: &'static str,
    pub http_status: Option<u16>,
}

#[derive(Serialize)]
pub struct ScoutIssueCount {
    pub issue: ScoutIssue,
    pub count: u64,
}

#[derive(Serialize)]
pub struct ScoutPhase {
    pub phase: &'static str,
    pub status: &'static str,
    pub next_retry: Option<DateTime<Utc>>,
    pub issue: Option<ScoutIssue>,
}

#[derive(Serialize)]
pub struct ScoutAttempt {
    pub last_attempted_page: Option<u64>,
    pub last_saved_page: Option<u64>,
    pub last_failed_page: Option<u64>,
    pub last_failure: Option<ScoutIssue>,
    pub metrics: BTreeMap<&'static str, u64>,
}

#[derive(Serialize)]
pub struct ScoutExtraction {
    pub next_page: Option<u64>,
    pub discovery_complete: Option<bool>,
    pub failure: Option<ScoutIssue>,
    pub phases: Vec<ScoutPhase>,
    pub invocation: Option<ScoutAttempt>,
    pub detail_issues: Vec<ScoutIssueCount>,
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
    fn extraction_projects_resume_page_failure_and_separate_invocation_counts() {
        let files = Files::new();
        files.write("scout", serde_json::json!({"reason":"fases_pendientes","pages":5,"extracted":501,
            "next_page":6,"discovery_complete":false,"error":"presupuesto_descubrimiento",
            "phases":{"discovery":{"status":"pausado","next_retry":"2026-10-25T06:00:00Z","error":"presupuesto_descubrimiento"},"extraction":{"status":"pendiente"}},
            "invocation":{"last_attempted_page":6,"last_saved_page":5,"last_failed_page":6,"last_error":"HonduCompras HTTP 500; PRIVATE_EXCEPTION","extracted":9,"failed":2,"incomplete":0},
            "detail_errors":{"PRIVATE_URL_1":"Detalle sin campos obligatorios o contenido útil; grilla=vacia","PRIVATE_URL_2":"HonduCompras HTTP 500","PRIVATE_URL_3":"SECRET"}}));
        let reports = files.read();
        let run = reports.reports[1].run.as_ref().unwrap();
        assert_eq!(run.reason, Some("fases_pendientes"));
        assert_eq!(run.metrics["extracted"], 501);
        let detail = run.extraction.as_ref().unwrap();
        assert_eq!(detail.next_page, Some(6));
        assert_eq!(detail.failure.as_ref().unwrap().code, "discovery_budget");
        assert_eq!(detail.phases[0].status, "pausado");
        assert!(detail.phases[0].next_retry.is_some());
        let attempt = detail.invocation.as_ref().unwrap();
        assert_eq!(attempt.last_saved_page, Some(5));
        assert_eq!(attempt.last_failed_page, Some(6));
        assert_eq!(
            attempt.last_failure.as_ref().unwrap().http_status,
            Some(500)
        );
        assert_eq!(attempt.metrics["extracted"], 9);
        assert_eq!(attempt.metrics["incomplete"], 0);
        assert_eq!(detail.detail_issues.len(), 3);
        let text = serde_json::to_string(&reports).unwrap();
        assert!(!text.contains("PRIVATE"));
        assert!(!text.contains("SECRET"));
    }

    #[test]
    fn old_extraction_reports_do_not_invent_invocation_metrics_or_error_pages() {
        let files = Files::new();
        files.write("scout", serde_json::json!({"pages":5,"extracted":501,"next_page":6,"error":"presupuesto_descubrimiento"}));
        let reports = files.read();
        let detail = reports.reports[1]
            .run
            .as_ref()
            .unwrap()
            .extraction
            .as_ref()
            .unwrap();
        assert!(detail.invocation.is_none());
        assert_eq!(detail.next_page, Some(6));
        assert_eq!(detail.failure.as_ref().unwrap().code, "discovery_budget");
        files.write("scout", serde_json::json!({"invocation":{"last_saved_page":-1,"extracted":"SECRET"},"phases":{"discovery":{"status":"SECRET","next_retry":"not-date","error":"SECRET"}}}));
        let reports = files.read();
        let detail = reports.reports[1]
            .run
            .as_ref()
            .unwrap()
            .extraction
            .as_ref()
            .unwrap();
        assert!(
            detail
                .invocation
                .as_ref()
                .unwrap()
                .last_saved_page
                .is_none()
        );
        assert!(detail.invocation.as_ref().unwrap().metrics.is_empty());
        assert!(!serde_json::to_string(&reports).unwrap().contains("SECRET"));
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
        "fases_pendientes" => "fases_pendientes",
        "heuristica_10_vencidos" => "heuristica_10_vencidos",
        "heuristica_10_vencidos_auditada" => "heuristica_10_vencidos_auditada",
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

// Project known diagnostics only. Free-form exceptions, URLs and private paths stay in Scout.
fn issue(value: Option<&Value>) -> Option<ScoutIssue> {
    let text = value
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty())?;
    let text = text.to_lowercase();
    let http_status = text.split_once("http ").and_then(|(_, tail)| {
        let token = tail
            .split_whitespace()
            .next()?
            .trim_matches(|c: char| !c.is_ascii_digit());
        if token.len() != 3 {
            return None;
        }
        token
            .parse::<u16>()
            .ok()
            .filter(|n| (400..=599).contains(n))
    });
    let code = if http_status.is_some() {
        "http"
    } else if text == "presupuesto_descubrimiento" {
        "discovery_budget"
    } else if text == "limite_horario_o_presupuesto_invocacion" {
        "invocation_budget"
    } else if text.contains("timeout") || text.contains("timed out") {
        "timeout"
    } else if text.contains("connectionerror")
        || text.contains("connecterror")
        || text.contains("no respondió")
    {
        "connection"
    } else if text.contains("no contiene la tabla de resultados")
        || text.contains("tabla sin enlaces")
        || text.contains("formulario asp.net")
    {
        "invalid_listing"
    } else if text.contains("no contiene el detalle del proceso") {
        "invalid_detail"
    } else if text.contains("detalle sin campos obligatorios") {
        "incomplete_detail"
    } else if text.contains("paginación repetida") || text.contains("no avanzó de página") {
        "repeated_page"
    } else if text == "fallos consecutivos de detalles" {
        "consecutive_details"
    } else {
        "unrecognized"
    };
    Some(ScoutIssue { code, http_status })
}

fn extraction(value: &Value) -> ScoutExtraction {
    let page =
        |object: &Value, key: &str| object.get(key).and_then(Value::as_u64).filter(|n| *n > 0);
    let phases = ["discovery", "extraction"]
        .into_iter()
        .filter_map(|phase| {
            let data = value.get("phases")?.get(phase)?.as_object()?;
            let status = match data.get("status").and_then(Value::as_str) {
                Some("completo") => "completo",
                Some("pausado") => "pausado",
                Some("pendiente") => "pendiente",
                _ => "desconocido",
            };
            Some(ScoutPhase {
                phase,
                status,
                next_retry: data
                    .get("next_retry")
                    .and_then(Value::as_str)
                    .and_then(|text| DateTime::parse_from_rfc3339(text).ok())
                    .map(|date| date.with_timezone(&Utc)),
                issue: issue(data.get("error")),
            })
        })
        .collect();
    let invocation = value
        .get("invocation")
        .filter(|data| data.is_object())
        .map(|data| {
            let mut metrics = BTreeMap::new();
            for key in [
                "pages_read",
                "details_attempted",
                "extracted",
                "incomplete",
                "expired",
                "failed",
                "interrupted",
            ] {
                if let Some(number) = data.get(key).and_then(Value::as_u64) {
                    metrics.insert(key, number);
                }
            }
            ScoutAttempt {
                last_attempted_page: page(data, "last_attempted_page"),
                last_saved_page: page(data, "last_saved_page"),
                last_failed_page: page(data, "last_failed_page"),
                last_failure: issue(data.get("last_error")),
                metrics,
            }
        });
    let mut groups = BTreeMap::new();
    if let Some(errors) = value.get("detail_errors").and_then(Value::as_object) {
        for error in errors.values().filter_map(|data| issue(Some(data))) {
            *groups
                .entry((error.code, error.http_status))
                .or_insert(0u64) += 1;
        }
    }
    ScoutExtraction {
        next_page: page(value, "next_page").or_else(|| {
            value
                .get("phases")
                .and_then(|data| data.get("discovery"))
                .and_then(|data| page(data, "next_page"))
        }),
        discovery_complete: value.get("discovery_complete").and_then(Value::as_bool),
        failure: issue(value.get("error"))
            .or_else(|| issue(value.get("stop_reason")).filter(|item| item.code != "unrecognized")),
        phases,
        invocation,
        detail_issues: groups
            .into_iter()
            .map(|((code, http_status), count)| ScoutIssueCount {
                issue: ScoutIssue { code, http_status },
                count,
            })
            .collect(),
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
        extraction: (stage == "scout").then(|| extraction(value)),
    })
}
