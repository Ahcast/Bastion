#![forbid(unsafe_code)]

mod diagnostics;
mod local;
mod scout;
mod service;

pub use diagnostics::{ReportAvailability, ScoutDiagnostics, ScoutReport, ScoutRun};
pub use local::{DEFAULT_SCOUT_CSV, LocalDeliveryError, ReadyDelivery};
pub use scout::{FIELDS, MAX_CSV_BYTES, MAX_ROWS, SCOUT_FIELDS, ScoutRow, parse_csv};
pub use service::{
    ImportResult, ProcessFilter, ProcessPage, ProcessRecord, ProcessService, ProcessSort,
    ReviewRecord,
};
