#![forbid(unsafe_code)]

mod admin;
mod config;
mod database;
mod service;

pub use admin::{
    AuditRecord, CoreAdmin, MailOutboxRecord, ModuleRecord, PersonInput, PersonRecord,
    RolePermissionInput, RoleRecord, SettingRecord, UserRecord,
};
pub use config::{AppConfig, ConfigError, DatabaseConfig};
pub use database::Database;
pub use service::{
    AccessCatalog, AuthCookies, AuthenticatedUser, BootstrapBegin, BootstrapComplete, CoreError,
    CoreService, InvitationDispatch, LoginStart, ModuleAccess, ProfileInfo, SecurityEventInfo,
    SessionInfo, SmtpCredentialInfo, TrustedDeviceInfo,
};
