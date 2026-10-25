#![forbid(unsafe_code)]

mod control;
mod diagnostics;
pub use control::{ScoutControl, ScoutControlStatus, ScoutJob, ScoutJobStatus, valid_scout_key};
mod local;
mod scout;
mod service;

pub use diagnostics::{
    ReportAvailability, ScoutAttempt, ScoutDiagnostics, ScoutExtraction, ScoutIssue,
    ScoutIssueCount, ScoutPhase, ScoutReport, ScoutRun,
};
pub use local::{DEFAULT_SCOUT_CSV, LocalDeliveryError, ReadyDelivery};
pub use scout::{FIELDS, MAX_CSV_BYTES, MAX_ROWS, SCOUT_FIELDS, ScoutRow, parse_csv};
pub use service::{
    ImportResult, ProcessFilter, ProcessPage, ProcessRecord, ProcessService, ProcessSort,
    ReviewRecord,
};
