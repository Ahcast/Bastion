#![forbid(unsafe_code)]

mod service;
mod spreadsheet;

pub use service::ReportService;
pub use spreadsheet::{SpreadsheetCell, workbook};
