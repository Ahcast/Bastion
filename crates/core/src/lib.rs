#![forbid(unsafe_code)]

mod admin;
mod config;
mod dashboard;
mod database;
mod documents;
mod service;

pub use admin::{
    AuditRecord, CoreAdmin, DomainActor, MailOutboxRecord, ModuleRecord, PersonInput, PersonRecord,
    RolePermissionInput, RoleRecord, SettingRecord, UserRecord,
};
pub use config::{AppConfig, ConfigError, DatabaseConfig};
pub use dashboard::{DashboardPeriod, DashboardPoint, DashboardQuery, DashboardWindow};
pub use database::Database;
pub use documents::{DocumentCompany, DocumentService};
pub use service::{
    AccessCatalog, AuthCookies, AuthenticatedUser, BootstrapBegin, BootstrapComplete, BusinessMail,
    BusinessMailAttachment, CoreError, CoreService, InvitationDispatch, LoginStart, ModuleAccess,
    ProfileInfo, SecurityEventInfo, SessionInfo, SmtpCredentialInfo, TrustedDeviceInfo,
};
