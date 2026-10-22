use std::{
    fmt,
    net::{IpAddr, Ipv4Addr},
    sync::Arc,
    time::Duration,
};

use aes_gcm::{Aes256Gcm, KeyInit, Nonce, aead::Aead};
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier, password_hash::SaltString};
use chrono::{NaiveDateTime, Utc};
use data_encoding::BASE32_NOPAD;
use hmac::{Hmac, Mac};
use lettre::{
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor, message::Mailbox,
    transport::smtp::authentication::Credentials,
};
use rand::{RngCore, rngs::OsRng};
use serde::Serialize;
use sha1::Sha1;
use sha2::{Digest, Sha256};
use sqlx::{FromRow, MySql, Transaction};
use subtle::ConstantTimeEq;
use tokio::sync::Semaphore;

use crate::{CoreAdmin, Database, admin::validate_person};

type HmacSha1 = Hmac<Sha1>;

#[derive(Debug)]
pub enum CoreError {
    NotFound,
    Invalid,
    Unauthorized,
    Forbidden,
    Conflict,
    Locked,
    ReauthenticationRequired,
    SessionReplaced,
    Overloaded,
    Internal(sqlx::Error),
    Migration(sqlx::migrate::MigrateError),
}

impl fmt::Display for CoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => f.write_str("resource not found"),
            Self::Invalid => f.write_str("invalid request"),
            Self::Unauthorized => f.write_str("authentication required"),
            Self::Forbidden => f.write_str("operation not allowed"),
            Self::Conflict => f.write_str("resource conflict"),
            Self::Locked => f.write_str("authentication temporarily unavailable"),
            Self::ReauthenticationRequired => f.write_str("additional authentication required"),
            Self::SessionReplaced => f.write_str("session was replaced by another login"),
            Self::Overloaded => f.write_str("service is temporarily busy"),
            Self::Internal(_) => f.write_str("internal service error"),
            Self::Migration(_) => f.write_str("database migration failed"),
        }
    }
}

impl std::error::Error for CoreError {}

impl From<sqlx::Error> for CoreError {
    fn from(value: sqlx::Error) -> Self {
        Self::Internal(value)
    }
}

#[derive(Clone)]
pub struct CoreService {
    database: Database,
    bootstrap_secret: String,
    public_app_origin: String,
    cipher: Aes256Gcm,
    dummy_password_hash: String,
    password_kdf_semaphore: Arc<Semaphore>,
}

#[derive(Debug, Serialize)]
pub struct BootstrapBegin {
    pub setup_token: String,
    pub totp_secret: String,
    pub otpauth_uri: String,
    pub expires_at: NaiveDateTime,
}

#[derive(Debug, Serialize)]
pub struct BootstrapComplete {
    pub recovery_codes: Vec<String>,
    pub status: &'static str,
}

pub enum LoginStart {
    TotpRequired { challenge_token: String },
    Authenticated(AuthCookies),
}

#[derive(Debug)]
pub struct AuthCookies {
    pub session_token: String,
    pub csrf_token: String,
    pub device_token: String,
    pub expires_at: Option<NaiveDateTime>,
    pub keep_logged_in: bool,
}

#[derive(Debug, Serialize)]
pub struct AuthenticatedUser {
    pub id: u64,
    pub person_id: u64,
    pub name: String,
    pub email: String,
    pub role: String,
}

#[derive(Debug, Serialize, FromRow)]
pub struct SessionInfo {
    pub id: u64,
    pub device: String,
    pub ip_address: String,
    pub started_at: NaiveDateTime,
    pub last_activity_at: NaiveDateTime,
    pub current: bool,
}

#[derive(Debug, Serialize, FromRow)]
pub struct TrustedDeviceInfo {
    pub id: u64,
    pub device: String,
    pub ip_address: String,
    pub created_at: NaiveDateTime,
    pub last_used_at: NaiveDateTime,
    pub expires_at: NaiveDateTime,
}

#[derive(Debug, Serialize, FromRow)]
pub struct SecurityEventInfo {
    pub event_type: String,
    pub result: String,
    pub ip_address: String,
    pub device: String,
    pub created_at: NaiveDateTime,
}

#[derive(Debug, Serialize, FromRow)]
pub struct ProfileInfo {
    pub person_id: u64,
    pub full_name: String,
    pub email: String,
    pub dni: Option<String>,
    pub country_code: Option<String>,
    pub gender: Option<String>,
}

#[derive(Debug, Serialize, FromRow)]
pub struct SmtpCredentialInfo {
    pub smtp_email: String,
    pub configured: bool,
    pub updated_at: NaiveDateTime,
}

#[derive(Debug, Serialize, FromRow)]
pub struct ModuleAccess {
    pub module_id: String,
    pub display_name: String,
}

#[derive(Debug, Serialize)]
pub struct AccessCatalog {
    pub is_system_administrator: bool,
    pub modules: Vec<ModuleAccess>,
    pub permissions: Vec<crate::RolePermissionInput>,
}

#[derive(Debug, Serialize)]
pub struct InvitationDispatch {
    pub user: crate::UserRecord,
    pub delivery_status: &'static str,
}

#[derive(Debug, FromRow)]
struct PendingMail {
    id: u64,
    sender_user_id: u64,
    recipient_email: String,
    subject: String,
    body_ciphertext: Vec<u8>,
    account_token_id: Option<u64>,
}

#[derive(Debug, FromRow)]
struct AuthUser {
    id: u64,
    person_id: u64,
    email: String,
    password_hash: Option<String>,
    status: String,
    totp_enabled: bool,
    totp_secret_enc: Option<Vec<u8>>,
    last_totp_counter: Option<i64>,
    failed_password_count: u32,
    lock_level: u8,
    locked_until: Option<NaiveDateTime>,
    incident_alerted_at: Option<NaiveDateTime>,
}

#[derive(Debug, FromRow)]
struct ChallengeRow {
    id: u64,
    user_id: u64,
    client_ip: String,
    user_agent: String,
    keep_logged_in: bool,
    attempts: u8,
}

#[derive(Debug, FromRow)]
struct SessionRow {
    id: u64,
    user_id: u64,
    person_id: u64,
    email: String,
    status: String,
    role_name: String,
    keep_logged_in: bool,
    last_totp_at: NaiveDateTime,
    last_activity_at: NaiveDateTime,
    expires_at: Option<NaiveDateTime>,
    revoked_at: Option<NaiveDateTime>,
    revocation_reason: Option<String>,
}

impl CoreService {
    pub fn database(&self) -> Database {
        self.database.clone()
    }

    pub fn admin(&self) -> CoreAdmin {
        CoreAdmin::new(self.database.clone())
    }

    pub fn new(
        database: Database,
        bootstrap_secret: String,
        data_encryption_key: [u8; 32],
    ) -> Result<Self, CoreError> {
        let salt = SaltString::generate(&mut OsRng);
        let dummy_password_hash = Argon2::default()
            .hash_password(b"constant-time-dummy-password", &salt)
            .map_err(|_| CoreError::Invalid)?
            .to_string();
        let cipher =
            Aes256Gcm::new_from_slice(&data_encryption_key).map_err(|_| CoreError::Invalid)?;
        Ok(Self {
            database,
            bootstrap_secret,
            public_app_origin: "https://app.castilloit.net".to_owned(),
            cipher,
            dummy_password_hash,
            password_kdf_semaphore: Arc::new(Semaphore::new(4)),
        })
    }

    pub fn with_public_app_origin(mut self, origin: String) -> Self {
        self.public_app_origin = origin;
        self
    }

    pub async fn migrate(&self) -> Result<(), CoreError> {
        self.database.migrate().await.map_err(CoreError::Migration)
    }

    pub async fn check_ready(&self) -> Result<(), CoreError> {
        self.database.check_ready().await.map_err(Into::into)
    }

    pub async fn bootstrap_status(&self) -> Result<bool, CoreError> {
        let state = sqlx::query_as::<_, (Option<NaiveDateTime>, Option<NaiveDateTime>)>(
            "SELECT completed_at,pending_expires_at FROM core_bootstrap_state WHERE id=1",
        )
        .fetch_one(&self.database.pool)
        .await?;
        let now = Utc::now().naive_utc();
        Ok(state.0.is_none() && state.1.is_none_or(|expiry| expiry <= now))
    }

    pub async fn begin_bootstrap(
        &self,
        provided_secret: &str,
        full_name: &str,
        email: &str,
        password: &str,
    ) -> Result<BootstrapBegin, CoreError> {
        if !constant_time_text_eq(&self.bootstrap_secret, provided_secret)
            || !self.bootstrap_status().await?
        {
            return Err(CoreError::Forbidden);
        }
        let full_name = full_name.trim();
        let email = normalize_email(email)?;
        validate_password(password)?;
        if full_name.is_empty() || full_name.chars().count() > 180 {
            return Err(CoreError::Invalid);
        }

        let seed = random_bytes::<20>();
        let totp_secret = BASE32_NOPAD.encode(&seed);
        let encrypted_seed = self.encrypt(&totp_secret)?;
        let setup_token = random_token();
        let now = Utc::now().naive_utc();
        let expires_at = now + chrono::Duration::minutes(10);
        let password_hash = hash_password(password)?;
        let mut tx = self.database.pool.begin().await?;
        let state=sqlx::query_as::<_,(Option<u64>,Option<NaiveDateTime>,Option<NaiveDateTime>)>("SELECT pending_user_id,pending_expires_at,completed_at FROM core_bootstrap_state WHERE id=1 FOR UPDATE")
            .fetch_one(&mut *tx).await?;
        if state.2.is_some() || state.1.is_some_and(|expiry| expiry > now) {
            return Err(CoreError::Forbidden);
        }
        if let Some(previous_user) = state.0 {
            let previous_person: Option<u64> = sqlx::query_scalar(
                "SELECT person_id FROM users WHERE id=? AND status='setup_pending'",
            )
            .bind(previous_user)
            .fetch_optional(&mut *tx)
            .await?;
            if let Some(person_id) = previous_person {
                sqlx::query("DELETE FROM users WHERE id=?")
                    .bind(previous_user)
                    .execute(&mut *tx)
                    .await?;
                sqlx::query("DELETE FROM persons WHERE id=?")
                    .bind(person_id)
                    .execute(&mut *tx)
                    .await?;
            }
        }
        let role_id: u64 = sqlx::query_scalar(
            "SELECT id FROM roles WHERE system_key='system_administrator' FOR UPDATE",
        )
        .fetch_one(&mut *tx)
        .await?;
        let person_id = sqlx::query("INSERT INTO persons (full_name) VALUES (?)")
            .bind(full_name)
            .execute(&mut *tx)
            .await?
            .last_insert_id();
        let user_id = sqlx::query("INSERT INTO users (person_id, role_id, email, password_hash, status, totp_secret_enc) VALUES (?, ?, ?, ?, 'setup_pending', ?)")
            .bind(person_id).bind(role_id).bind(&email).bind(password_hash).bind(encrypted_seed)
            .execute(&mut *tx).await?.last_insert_id();
        sqlx::query(
            "INSERT INTO bootstrap_setups (user_id, token_hash, expires_at) VALUES (?, ?, ?)",
        )
        .bind(user_id)
        .bind(token_hash(&setup_token).to_vec())
        .bind(expires_at)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "UPDATE core_bootstrap_state SET pending_user_id=?,pending_expires_at=? WHERE id=1",
        )
        .bind(user_id)
        .bind(expires_at)
        .execute(&mut *tx)
        .await?;
        insert_audit(
            &mut tx,
            Some(user_id),
            "core",
            "bootstrap.begin",
            "user",
            Some(user_id.to_string()),
            "success",
        )
        .await?;
        tx.commit().await?;

        let label = url::form_urlencoded::byte_serialize(email.as_bytes()).collect::<String>();
        Ok(BootstrapBegin {
            setup_token,
            totp_secret: totp_secret.clone(),
            otpauth_uri: format!(
                "otpauth://totp/Bastion:{label}?secret={totp_secret}&issuer=Bastion&algorithm=SHA1&digits=6&period=30"
            ),
            expires_at,
        })
    }

    pub async fn complete_bootstrap(
        &self,
        setup_token: &str,
        code: &str,
    ) -> Result<BootstrapComplete, CoreError> {
        let now = Utc::now().naive_utc();
        let mut tx = self.database.pool.begin().await?;
        let state = sqlx::query_as::<_, (Option<u64>, Option<NaiveDateTime>)>(
            "SELECT pending_user_id,completed_at FROM core_bootstrap_state WHERE id=1 FOR UPDATE",
        )
        .fetch_one(&mut *tx)
        .await?;
        let row = sqlx::query_as::<_, (u64, Vec<u8>, Option<i64>)>(
            "SELECT s.user_id, u.totp_secret_enc, u.last_totp_counter FROM bootstrap_setups s JOIN users u ON u.id=s.user_id WHERE s.token_hash=? AND s.consumed_at IS NULL AND s.expires_at>? FOR UPDATE"
        ).bind(token_hash(setup_token).to_vec()).bind(now).fetch_optional(&mut *tx).await?.ok_or(CoreError::Unauthorized)?;
        if state.0 != Some(row.0) || state.1.is_some() {
            return Err(CoreError::Unauthorized);
        }
        let seed_text = self.decrypt(&row.1)?;
        let accepted = verify_totp(&seed_text, code, row.2.unwrap_or(-1));
        let Some(counter) = accepted else {
            return Err(CoreError::Unauthorized);
        };

        let recovery_codes = (0..10).map(|_| recovery_code()).collect::<Vec<_>>();
        for code in &recovery_codes {
            sqlx::query("INSERT INTO totp_recovery_codes (user_id, code_hash) VALUES (?, ?)")
                .bind(row.0)
                .bind(token_hash(code).to_vec())
                .execute(&mut *tx)
                .await?;
        }
        sqlx::query("UPDATE users SET status='active', totp_enabled=1, last_totp_counter=? WHERE id=? AND status='setup_pending'")
            .bind(counter as i64).bind(row.0).execute(&mut *tx).await?;
        sqlx::query("UPDATE bootstrap_setups SET consumed_at=? WHERE token_hash=?")
            .bind(now)
            .bind(token_hash(setup_token).to_vec())
            .execute(&mut *tx)
            .await?;
        sqlx::query("UPDATE core_bootstrap_state SET pending_user_id=NULL,pending_expires_at=NULL,completed_at=? WHERE id=1")
            .bind(now).execute(&mut *tx).await?;
        insert_audit(
            &mut tx,
            Some(row.0),
            "core",
            "bootstrap.complete",
            "user",
            Some(row.0.to_string()),
            "success",
        )
        .await?;
        tx.commit().await?;
        Ok(BootstrapComplete {
            recovery_codes,
            status: "active",
        })
    }

    pub async fn start_login(
        &self,
        email: &str,
        password: &str,
        ip: &str,
        user_agent: &str,
        keep_logged_in: bool,
        device_token: Option<&str>,
    ) -> Result<LoginStart, CoreError> {
        let _kdf_permit = self
            .password_kdf_semaphore
            .acquire()
            .await
            .map_err(|_| CoreError::Overloaded)?;
        let email = normalize_email(email)?;
        validate_password_input(password)?;
        let mut tx = self.database.pool.begin().await?;
        let user = sqlx::query_as::<_, AuthUser>(
            "SELECT u.id,u.person_id,u.email,u.password_hash,u.status,u.totp_enabled,u.totp_secret_enc,u.last_totp_counter,u.failed_password_count,u.lock_level,u.locked_until,u.incident_alerted_at FROM users u WHERE u.email=? FOR UPDATE"
        ).bind(&email).fetch_optional(&mut *tx).await?;
        let Some(user) = user else {
            tx.commit().await?;
            let _ = verify_password(password, &self.dummy_password_hash);
            self.log_security(None, "password_login", "failed", ip, safe_agent(user_agent))
                .await?;
            return Err(CoreError::Unauthorized);
        };
        if user.status != "active" {
            tx.commit().await?;
            let _ = verify_password(password, &self.dummy_password_hash);
            self.log_security(
                Some(user.id),
                "password_login",
                "failed",
                ip,
                safe_agent(user_agent),
            )
            .await?;
            return Err(CoreError::Unauthorized);
        }
        if user
            .locked_until
            .is_some_and(|until| until > Utc::now().naive_utc())
        {
            tx.commit().await?;
            return Err(CoreError::Locked);
        }
        let valid_password = user
            .password_hash
            .as_deref()
            .is_some_and(|hash| verify_password(password, hash));
        if !valid_password {
            self.record_password_failure(&mut tx, &user, ip).await?;
            tx.commit().await?;
            return Err(CoreError::Unauthorized);
        }
        if !user.totp_enabled || user.totp_secret_enc.is_none() {
            tx.commit().await?;
            return Err(CoreError::Forbidden);
        }
        sqlx::query("UPDATE users SET failed_password_count=0, lock_level=0, locked_until=NULL, incident_alerted_at=NULL WHERE id=?")
            .bind(user.id).execute(&mut *tx).await?;
        tx.commit().await?;

        let now = Utc::now().naive_utc();
        let known = if let Some(raw) = device_token.filter(|t| !t.is_empty()) {
            sqlx::query_as::<_, (u64, String, NaiveDateTime, NaiveDateTime)>(
                "SELECT id,ip_address,expires_at,last_totp_at FROM trusted_devices WHERE user_id=? AND token_hash=? AND revoked_at IS NULL AND expires_at>?"
            ).bind(user.id).bind(token_hash(raw).to_vec()).bind(now).fetch_optional(&self.database.pool).await?
        } else {
            None
        };
        let recognized = known
            .as_ref()
            .is_some_and(|(_, trusted_ip, expires, last_totp)| {
                trusted_ip == ip && *expires > now && *last_totp > now - chrono::Duration::days(15)
            });
        let safe_agent = safe_agent(user_agent);
        if recognized {
            let (device_id, _, _, last_totp_at) =
                known.as_ref().expect("recognized trusted device");
            sqlx::query("UPDATE trusted_devices SET last_used_at=? WHERE id=?")
                .bind(now)
                .bind(device_id)
                .execute(&self.database.pool)
                .await?;
            let cookies = self
                .create_session(
                    user.id,
                    ip,
                    &safe_agent,
                    keep_logged_in,
                    now,
                    *last_totp_at,
                    Some(device_token.unwrap_or_default().to_owned()),
                )
                .await?;
            self.log_security(Some(user.id), "password_login", "success", ip, safe_agent)
                .await?;
            Ok(LoginStart::Authenticated(cookies))
        } else {
            let challenge_token = random_token();
            let expires = now + chrono::Duration::minutes(5);
            sqlx::query("INSERT INTO otp_challenges (user_id,challenge_hash,client_ip,user_agent,keep_logged_in,expires_at) VALUES (?,?,?,?,?,?)")
                .bind(user.id).bind(token_hash(&challenge_token).to_vec()).bind(ip).bind(&safe_agent).bind(keep_logged_in).bind(expires)
                .execute(&self.database.pool).await?;
            self.log_security(Some(user.id), "totp_required", "pending", ip, safe_agent)
                .await?;
            Ok(LoginStart::TotpRequired { challenge_token })
        }
    }

    pub async fn verify_login_totp(
        &self,
        challenge_token: &str,
        code: &str,
        device_token: Option<&str>,
    ) -> Result<AuthCookies, CoreError> {
        let now = Utc::now().naive_utc();
        let mut tx = self.database.pool.begin().await?;
        let challenge = sqlx::query_as::<_, ChallengeRow>(
            "SELECT id,user_id,client_ip,user_agent,keep_logged_in,attempts FROM otp_challenges WHERE challenge_hash=? AND consumed_at IS NULL AND expires_at>? FOR UPDATE"
        ).bind(token_hash(challenge_token).to_vec()).bind(now).fetch_optional(&mut *tx).await?.ok_or(CoreError::Unauthorized)?;
        if self
            .is_totp_limited(challenge.user_id, &challenge.client_ip)
            .await?
        {
            return Err(CoreError::Locked);
        }
        let auth = sqlx::query_as::<_, (Option<Vec<u8>>, Option<i64>)>("SELECT totp_secret_enc,last_totp_counter FROM users WHERE id=? AND status='active' AND totp_enabled=1 FOR UPDATE")
            .bind(challenge.user_id).fetch_optional(&mut *tx).await?.ok_or(CoreError::Unauthorized)?;
        let seed = auth.0.ok_or(CoreError::Unauthorized)?;
        let seed = self.decrypt(&seed)?;
        let accepted = verify_totp(&seed, code, auth.1.unwrap_or(-1));
        let Some(counter) = accepted else {
            let attempts = challenge.attempts.saturating_add(1);
            let consumed = attempts >= 3;
            sqlx::query("UPDATE otp_challenges SET attempts=?, consumed_at=IF(?, ?, consumed_at) WHERE id=?")
                .bind(attempts).bind(consumed).bind(now).bind(challenge.id).execute(&mut *tx).await?;
            sqlx::query("INSERT INTO security_events (user_id,event_type,result,ip_address,device_label,created_at) VALUES (?,'totp_login','failed',?,?,?)")
                .bind(challenge.user_id).bind(&challenge.client_ip).bind(&challenge.user_agent).bind(now).execute(&mut *tx).await?;
            tx.commit().await?;
            self.record_totp_failure(challenge.user_id, &challenge.client_ip)
                .await?;
            return Err(CoreError::Unauthorized);
        };
        sqlx::query("UPDATE users SET last_totp_counter=? WHERE id=? AND (last_totp_counter IS NULL OR last_totp_counter<?)")
            .bind(counter as i64).bind(challenge.user_id).bind(counter as i64).execute(&mut *tx).await?;
        sqlx::query("UPDATE otp_challenges SET consumed_at=? WHERE id=?")
            .bind(now)
            .bind(challenge.id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("INSERT INTO security_events (user_id,event_type,result,ip_address,device_label,created_at) VALUES (?,'totp_login','success',?,?,?)")
            .bind(challenge.user_id).bind(&challenge.client_ip).bind(&challenge.user_agent).bind(now).execute(&mut *tx).await?;
        tx.commit().await?;

        let (device, is_new_device) = self
            .trust_device(
                challenge.user_id,
                device_token,
                &challenge.client_ip,
                &challenge.user_agent,
                now,
            )
            .await?;
        let mut cookies = self
            .create_session(
                challenge.user_id,
                &challenge.client_ip,
                &challenge.user_agent,
                challenge.keep_logged_in,
                now,
                now,
                None,
            )
            .await?;
        cookies.device_token = device;
        if is_new_device {
            self.notify_new_device(challenge.user_id).await;
        }
        self.audit(
            Some(challenge.user_id),
            "core",
            "auth.totp",
            "user",
            challenge.user_id.to_string(),
            "success",
        )
        .await?;
        Ok(cookies)
    }

    pub async fn verify_login_recovery_code(
        &self,
        challenge_token: &str,
        code: &str,
        device_token: Option<&str>,
    ) -> Result<AuthCookies, CoreError> {
        if code.len() != 16 || !code.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(CoreError::Unauthorized);
        }
        let now = Utc::now().naive_utc();
        let mut tx = self.database.pool.begin().await?;
        let challenge=sqlx::query_as::<_,ChallengeRow>("SELECT id,user_id,client_ip,user_agent,keep_logged_in,attempts FROM otp_challenges WHERE challenge_hash=? AND consumed_at IS NULL AND expires_at>? FOR UPDATE")
            .bind(token_hash(challenge_token).to_vec()).bind(now).fetch_optional(&mut *tx).await?.ok_or(CoreError::Unauthorized)?;
        if self
            .is_totp_limited(challenge.user_id, &challenge.client_ip)
            .await?
        {
            return Err(CoreError::Locked);
        }
        let active: Option<u64> = sqlx::query_scalar(
            "SELECT id FROM users WHERE id=? AND status='active' AND totp_enabled=1 FOR UPDATE",
        )
        .bind(challenge.user_id)
        .fetch_optional(&mut *tx)
        .await?;
        if active.is_none() {
            return Err(CoreError::Unauthorized);
        }
        let code_hash = token_hash(&code.to_ascii_uppercase()).to_vec();
        let recovery=sqlx::query_scalar::<_,u64>("SELECT id FROM totp_recovery_codes WHERE user_id=? AND code_hash=? AND consumed_at IS NULL FOR UPDATE")
            .bind(challenge.user_id).bind(&code_hash).fetch_optional(&mut *tx).await?;
        let Some(recovery_id) = recovery else {
            let attempts = challenge.attempts.saturating_add(1);
            let consumed = attempts >= 3;
            sqlx::query(
                "UPDATE otp_challenges SET attempts=?,consumed_at=IF(?, ?, consumed_at) WHERE id=?",
            )
            .bind(attempts)
            .bind(consumed)
            .bind(now)
            .bind(challenge.id)
            .execute(&mut *tx)
            .await?;
            sqlx::query("INSERT INTO security_events (user_id,event_type,result,ip_address,device_label,created_at) VALUES (?,'recovery_login','failed',?,?,?)").bind(challenge.user_id).bind(&challenge.client_ip).bind(&challenge.user_agent).bind(now).execute(&mut *tx).await?;
            tx.commit().await?;
            self.record_totp_failure(challenge.user_id, &challenge.client_ip)
                .await?;
            return Err(CoreError::Unauthorized);
        };
        sqlx::query(
            "UPDATE totp_recovery_codes SET consumed_at=? WHERE id=? AND consumed_at IS NULL",
        )
        .bind(now)
        .bind(recovery_id)
        .execute(&mut *tx)
        .await?;
        sqlx::query("UPDATE otp_challenges SET consumed_at=? WHERE id=?")
            .bind(now)
            .bind(challenge.id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("INSERT INTO security_events (user_id,event_type,result,ip_address,device_label,created_at) VALUES (?,'recovery_login','success',?,?,?)").bind(challenge.user_id).bind(&challenge.client_ip).bind(&challenge.user_agent).bind(now).execute(&mut *tx).await?;
        tx.commit().await?;
        let (device, is_new_device) = self
            .trust_device(
                challenge.user_id,
                device_token,
                &challenge.client_ip,
                &challenge.user_agent,
                now,
            )
            .await?;
        let mut cookies = self
            .create_session(
                challenge.user_id,
                &challenge.client_ip,
                &challenge.user_agent,
                challenge.keep_logged_in,
                now,
                now,
                None,
            )
            .await?;
        cookies.device_token = device;
        if is_new_device {
            self.notify_new_device(challenge.user_id).await;
        }
        self.audit(
            Some(challenge.user_id),
            "core",
            "auth.recovery_code",
            "user",
            challenge.user_id.to_string(),
            "success",
        )
        .await?;
        Ok(cookies)
    }

    pub async fn authenticate(&self, session_token: &str) -> Result<AuthenticatedUser, CoreError> {
        if session_token.is_empty() {
            return Err(CoreError::Unauthorized);
        }
        let now = Utc::now().naive_utc();
        let hash = token_hash(session_token).to_vec();
        let row = sqlx::query_as::<_, SessionRow>(
            "SELECT s.id,s.user_id,u.person_id,u.email,u.status,r.name AS role_name,s.keep_logged_in,s.last_totp_at,s.last_activity_at,s.expires_at,s.revoked_at,s.revocation_reason FROM sessions s JOIN users u ON u.id=s.user_id JOIN roles r ON r.id=u.role_id WHERE s.token_hash=?"
        ).bind(&hash).fetch_optional(&self.database.pool).await?.ok_or(CoreError::Unauthorized)?;
        if row.revoked_at.is_some() {
            return if row.revocation_reason.as_deref() == Some("replaced") {
                Err(CoreError::SessionReplaced)
            } else {
                Err(CoreError::Unauthorized)
            };
        }
        if row.status != "active"
            || row.expires_at.is_some_and(|e| e <= now)
            || (!row.keep_logged_in && row.last_activity_at + chrono::Duration::hours(24) <= now)
        {
            sqlx::query("UPDATE sessions SET revoked_at=?,revocation_reason='expired' WHERE id=? AND revoked_at IS NULL").bind(now).bind(row.id).execute(&self.database.pool).await?;
            return Err(CoreError::Unauthorized);
        }
        if row.last_totp_at + chrono::Duration::days(15) <= now {
            return Err(CoreError::ReauthenticationRequired);
        }
        sqlx::query("UPDATE sessions SET last_activity_at=? WHERE id=? AND revoked_at IS NULL")
            .bind(now)
            .bind(row.id)
            .execute(&self.database.pool)
            .await?;
        Ok(AuthenticatedUser {
            id: row.user_id,
            person_id: row.person_id,
            name: String::new(),
            email: row.email,
            role: row.role_name,
        })
    }

    pub async fn current_user(&self, session_token: &str) -> Result<AuthenticatedUser, CoreError> {
        let mut user = self.authenticate(session_token).await?;
        let person_name: String = sqlx::query_scalar("SELECT full_name FROM persons WHERE id=?")
            .bind(user.person_id)
            .fetch_one(&self.database.pool)
            .await?;
        user.name = person_name;
        Ok(user)
    }

    pub async fn profile(&self, session_token: &str) -> Result<ProfileInfo, CoreError> {
        let user = self.authenticate(session_token).await?;
        sqlx::query_as::<_,ProfileInfo>("SELECT p.id AS person_id,p.full_name,u.email,p.dni,p.country_code,p.gender FROM persons p JOIN users u ON u.person_id=p.id WHERE u.id=? AND u.status='active'")
            .bind(user.id).fetch_optional(&self.database.pool).await?.ok_or(CoreError::Unauthorized)
    }

    pub async fn access_catalog(&self, session_token: &str) -> Result<AccessCatalog, CoreError> {
        let user = self.authenticate(session_token).await?;
        let system_key:Option<String>=sqlx::query_scalar("SELECT r.system_key FROM users u JOIN roles r ON r.id=u.role_id WHERE u.id=? AND u.status='active'")
            .bind(user.id).fetch_optional(&self.database.pool).await?;
        let is_admin = system_key.as_deref() == Some("system_administrator");
        let modules = if is_admin {
            sqlx::query_as::<_,ModuleAccess>("SELECT m.module_id,m.display_name FROM module_registry m WHERE m.included_in_release=1 AND m.environment_allowed=1 AND m.enabled=1 ORDER BY m.display_name,m.module_id")
                .fetch_all(&self.database.pool).await?
        } else {
            sqlx::query_as::<_,ModuleAccess>("SELECT DISTINCT m.module_id,m.display_name FROM module_registry m JOIN role_permissions rp ON rp.module_id=m.module_id JOIN users u ON u.role_id=rp.role_id WHERE u.id=? AND u.status='active' AND m.included_in_release=1 AND m.environment_allowed=1 AND m.enabled=1 ORDER BY m.display_name,m.module_id")
                .bind(user.id).fetch_all(&self.database.pool).await?
        };
        let permissions = if is_admin {
            Vec::new()
        } else {
            sqlx::query_as::<_,crate::RolePermissionInput>("SELECT rp.module_id,rp.scope_key,rp.capability FROM role_permissions rp JOIN users u ON u.role_id=rp.role_id WHERE u.id=? AND u.status='active' ORDER BY rp.module_id,rp.scope_key,rp.capability")
                .bind(user.id).fetch_all(&self.database.pool).await?
        };
        Ok(AccessCatalog {
            is_system_administrator: is_admin,
            modules,
            permissions,
        })
    }

    pub async fn update_profile(
        &self,
        session_token: &str,
        input: crate::PersonInput,
    ) -> Result<ProfileInfo, CoreError> {
        let user = self.authenticate(session_token).await?;
        let input = validate_person(input)?;
        let mut tx = self.database.pool.begin().await?;
        let current: Option<u64> = sqlx::query_scalar(
            "SELECT person_id FROM users WHERE id=? AND status='active' FOR UPDATE",
        )
        .bind(user.id)
        .fetch_optional(&mut *tx)
        .await?;
        let person_id = current.ok_or(CoreError::Unauthorized)?;
        let result =
            sqlx::query("UPDATE persons SET full_name=?,dni=?,country_code=?,gender=? WHERE id=?")
                .bind(&input.full_name)
                .bind(&input.dni)
                .bind(&input.country_code)
                .bind(&input.gender)
                .bind(person_id)
                .execute(&mut *tx)
                .await;
        match result {
            Ok(r) if r.rows_affected() > 0 => {}
            Ok(_) => return Err(CoreError::Conflict),
            Err(error) => return Err(map_unique_conflict(error)),
        }
        insert_audit(
            &mut tx,
            Some(user.id),
            "core",
            "account.profile.update",
            "person",
            Some(person_id.to_string()),
            "success",
        )
        .await?;
        let profile = sqlx::query_as::<_,ProfileInfo>("SELECT p.id AS person_id,p.full_name,u.email,p.dni,p.country_code,p.gender FROM persons p JOIN users u ON u.person_id=p.id WHERE u.id=?")
            .bind(user.id).fetch_one(&mut *tx).await?;
        tx.commit().await?;
        Ok(profile)
    }

    pub async fn change_password(
        &self,
        session_token: &str,
        current_password: &str,
        new_password: &str,
        totp_code: &str,
        ip: &str,
        user_agent: &str,
    ) -> Result<(), CoreError> {
        validate_password(new_password)?;
        validate_password_input(current_password)?;
        let _permit = self
            .password_kdf_semaphore
            .acquire()
            .await
            .map_err(|_| CoreError::Overloaded)?;
        let now = Utc::now().naive_utc();
        let mut tx = self.database.pool.begin().await?;
        let row = sqlx::query_as::<_,(u64,u64,Option<String>,Option<Vec<u8>>,Option<i64>)>(
            "SELECT s.id,u.id,u.password_hash,u.totp_secret_enc,u.last_totp_counter FROM sessions s JOIN users u ON u.id=s.user_id WHERE s.token_hash=? AND s.revoked_at IS NULL AND u.status='active' AND (s.expires_at IS NULL OR s.expires_at>?) AND (s.keep_logged_in=1 OR s.last_activity_at>?-INTERVAL 24 HOUR) FOR UPDATE"
        ).bind(token_hash(session_token).to_vec()).bind(now).bind(now).fetch_optional(&mut *tx).await?.ok_or(CoreError::Unauthorized)?;
        if self.is_totp_limited(row.1, ip).await? {
            return Err(CoreError::Locked);
        }
        if !row
            .2
            .as_deref()
            .is_some_and(|hash| verify_password(current_password, hash))
        {
            sqlx::query("INSERT INTO security_events (user_id,event_type,result,ip_address,device_label,created_at) VALUES (?,'password_change','failed',?,?,?)")
                .bind(row.1).bind(ip).bind(safe_agent(user_agent)).bind(now).execute(&mut *tx).await?;
            tx.commit().await?;
            return Err(CoreError::Unauthorized);
        }
        let secret = self.decrypt(row.3.as_deref().ok_or(CoreError::Unauthorized)?)?;
        let Some(counter) = verify_totp(&secret, totp_code, row.4.unwrap_or(-1)) else {
            sqlx::query("INSERT INTO security_events (user_id,event_type,result,ip_address,device_label,created_at) VALUES (?,'totp_sensitive_action','failed',?,?,?)")
                .bind(row.1).bind(ip).bind(safe_agent(user_agent)).bind(now).execute(&mut *tx).await?;
            tx.commit().await?;
            self.record_totp_failure(row.1, ip).await?;
            return Err(CoreError::Unauthorized);
        };
        let password_hash = hash_password(new_password)?;
        sqlx::query("UPDATE users SET password_hash=?,last_totp_counter=? WHERE id=?")
            .bind(password_hash)
            .bind(counter as i64)
            .bind(row.1)
            .execute(&mut *tx)
            .await?;
        sqlx::query("UPDATE sessions SET revoked_at=?,revocation_reason='password_changed' WHERE user_id=? AND revoked_at IS NULL")
            .bind(now).bind(row.1).execute(&mut *tx).await?;
        sqlx::query(
            "UPDATE trusted_devices SET revoked_at=? WHERE user_id=? AND revoked_at IS NULL",
        )
        .bind(now)
        .bind(row.1)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "UPDATE otp_challenges SET consumed_at=? WHERE user_id=? AND consumed_at IS NULL",
        )
        .bind(now)
        .bind(row.1)
        .execute(&mut *tx)
        .await?;
        sqlx::query("INSERT INTO security_events (user_id,event_type,result,ip_address,device_label,created_at) VALUES (?,'password_change','success',?,?,?)")
            .bind(row.1).bind(ip).bind(safe_agent(user_agent)).bind(now).execute(&mut *tx).await?;
        insert_audit(
            &mut tx,
            Some(row.1),
            "core",
            "account.password.change",
            "user",
            Some(row.1.to_string()),
            "success",
        )
        .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn smtp_credentials(
        &self,
        session_token: &str,
    ) -> Result<Option<SmtpCredentialInfo>, CoreError> {
        let user = self.authenticate(session_token).await?;
        sqlx::query_as::<_,SmtpCredentialInfo>("SELECT smtp_email,1 AS configured,updated_at FROM user_smtp_credentials WHERE user_id=?")
            .bind(user.id).fetch_optional(&self.database.pool).await.map_err(Into::into)
    }

    pub async fn save_smtp_credentials(
        &self,
        session_token: &str,
        smtp_email: &str,
        smtp_password: Option<&str>,
    ) -> Result<SmtpCredentialInfo, CoreError> {
        let user = self.authenticate(session_token).await?;
        let email = normalize_email(smtp_email)?;
        if let Some(password) = smtp_password {
            if password.is_empty() || password.len() > 1024 {
                return Err(CoreError::Invalid);
            }
            let ciphertext = self.encrypt(password)?;
            sqlx::query("INSERT INTO user_smtp_credentials (user_id,smtp_email,password_ciphertext) VALUES (?,?,?) ON DUPLICATE KEY UPDATE smtp_email=VALUES(smtp_email),password_ciphertext=VALUES(password_ciphertext)")
                .bind(user.id).bind(email).bind(ciphertext).execute(&self.database.pool).await?;
        } else {
            let exists: Option<u64> =
                sqlx::query_scalar("SELECT user_id FROM user_smtp_credentials WHERE user_id=?")
                    .bind(user.id)
                    .fetch_optional(&self.database.pool)
                    .await?;
            if exists.is_none() {
                return Err(CoreError::Invalid);
            }
            sqlx::query("UPDATE user_smtp_credentials SET smtp_email=? WHERE user_id=?")
                .bind(email)
                .bind(user.id)
                .execute(&self.database.pool)
                .await?;
        }
        self.audit(
            Some(user.id),
            "core",
            "account.smtp.update",
            "user",
            user.id.to_string(),
            "success",
        )
        .await?;
        sqlx::query_as::<_,SmtpCredentialInfo>("SELECT smtp_email,1 AS configured,updated_at FROM user_smtp_credentials WHERE user_id=?")
            .bind(user.id).fetch_one(&self.database.pool).await.map_err(Into::into)
    }

    pub async fn create_user_with_invitation(
        &self,
        administrator_id: u64,
        person_id: u64,
        role_id: u64,
        email: &str,
    ) -> Result<InvitationDispatch, CoreError> {
        let user = self
            .admin()
            .create_user(administrator_id, person_id, role_id, email)
            .await?;
        self.dispatch_invitation(administrator_id, user.id).await?;
        Ok(InvitationDispatch {
            user,
            delivery_status: "queued",
        })
    }

    pub async fn dispatch_invitation(
        &self,
        administrator_id: u64,
        user_id: u64,
    ) -> Result<(), CoreError> {
        self.require_system_administrator(administrator_id).await?;
        let user =
            sqlx::query_as::<_, (String, String)>("SELECT email,status FROM users WHERE id=?")
                .bind(user_id)
                .fetch_optional(&self.database.pool)
                .await?
                .ok_or(CoreError::Invalid)?;
        if !matches!(user.1.as_str(), "invitation_pending" | "totp_setup_pending") {
            return Err(CoreError::Conflict);
        }
        if user.1 == "totp_setup_pending" {
            let mut tx = self.database.pool.begin().await?;
            let now = Utc::now().naive_utc();
            sqlx::query("UPDATE totp_enrollment_tokens SET consumed_at=? WHERE user_id=? AND consumed_at IS NULL").bind(now).bind(user_id).execute(&mut *tx).await?;
            sqlx::query("UPDATE users SET status='invitation_pending',password_hash=NULL,totp_secret_enc=NULL,totp_enabled=0 WHERE id=?").bind(user_id).execute(&mut *tx).await?;
            tx.commit().await?;
        }
        self.enqueue_account_email(administrator_id,user_id,"invitation",None,&user.0,"Activa tu cuenta de Bastion","Se creó una cuenta para ti. Actívala y configura tu autenticador TOTP desde el siguiente enlace. El enlace vence 24 horas después de su envío.").await
    }

    pub async fn request_password_reset(&self, email: &str, ip: &str) -> Result<(), CoreError> {
        let Ok(email) = normalize_email(email) else {
            return Ok(());
        };
        let now = Utc::now().naive_utc();
        let since = now - chrono::Duration::minutes(15);
        let requests:i64=sqlx::query_scalar("SELECT COUNT(*) FROM security_events WHERE ip_address=? AND event_type='password_reset_request' AND created_at>?").bind(ip).bind(since).fetch_one(&self.database.pool).await?;
        sqlx::query("INSERT INTO security_events (user_id,event_type,result,ip_address,device_label,created_at) VALUES (NULL,'password_reset_request','received',?,'',?)").bind(ip).bind(now).execute(&self.database.pool).await?;
        if requests >= 5 {
            return Ok(());
        }
        let user = sqlx::query_as::<_, (u64, Option<u64>)>(
            "SELECT id,created_by FROM users WHERE email=? AND status='active'",
        )
        .bind(email)
        .fetch_optional(&self.database.pool)
        .await?;
        let Some((user_id, Some(sender_id))) = user else {
            return Ok(());
        };
        let address: String = sqlx::query_scalar("SELECT email FROM users WHERE id=?")
            .bind(user_id)
            .fetch_one(&self.database.pool)
            .await?;
        self.enqueue_account_email(sender_id,user_id,"password_reset",None,&address,"Restablece tu contraseña de Bastion","Recibimos una solicitud para cambiar la contraseña de tu cuenta. Si la hiciste, continúa desde este enlace. Vence una hora después de su envío. Si no solicitaste el cambio, ignora este mensaje.").await?;
        Ok(())
    }

    pub async fn complete_password_reset(
        &self,
        token: &str,
        new_password: &str,
        ip: &str,
    ) -> Result<(), CoreError> {
        validate_password(new_password)?;
        let _permit = self
            .password_kdf_semaphore
            .acquire()
            .await
            .map_err(|_| CoreError::Overloaded)?;
        let now = Utc::now().naive_utc();
        let mut tx = self.database.pool.begin().await?;
        let row=sqlx::query_as::<_,(u64,u64)>("SELECT t.id,t.user_id FROM account_tokens t JOIN users u ON u.id=t.user_id WHERE t.token_hash=? AND t.token_type='password_reset' AND t.mailed_at IS NOT NULL AND t.expires_at>? AND t.consumed_at IS NULL AND u.status='active' FOR UPDATE")
            .bind(token_hash(token).to_vec()).bind(now).fetch_optional(&mut *tx).await?.ok_or(CoreError::Unauthorized)?;
        let password_hash = hash_password(new_password)?;
        sqlx::query("UPDATE users SET password_hash=? WHERE id=? AND status='active'")
            .bind(password_hash)
            .bind(row.1)
            .execute(&mut *tx)
            .await?;
        sqlx::query("UPDATE account_tokens SET consumed_at=? WHERE id=?")
            .bind(now)
            .bind(row.0)
            .execute(&mut *tx)
            .await?;
        revoke_user_access(&mut tx, row.1, now, "password_changed").await?;
        sqlx::query("INSERT INTO security_events (user_id,event_type,result,ip_address,device_label,created_at) VALUES (?,'password_reset','success',?,'',?)").bind(row.1).bind(ip).bind(now).execute(&mut *tx).await?;
        insert_audit(
            &mut tx,
            Some(row.1),
            "core",
            "account.password.reset",
            "user",
            Some(row.1.to_string()),
            "success",
        )
        .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn begin_invitation_activation(
        &self,
        invitation_token: &str,
        password: &str,
    ) -> Result<BootstrapBegin, CoreError> {
        validate_password(password)?;
        let _permit = self
            .password_kdf_semaphore
            .acquire()
            .await
            .map_err(|_| CoreError::Overloaded)?;
        let password_hash = hash_password(password)?;
        let now = Utc::now().naive_utc();
        let seed = BASE32_NOPAD.encode(&random_bytes::<20>());
        let encrypted_seed = self.encrypt(&seed)?;
        let setup_token = random_token();
        let expires_at = now + chrono::Duration::minutes(10);
        let mut tx = self.database.pool.begin().await?;
        let row=sqlx::query_as::<_,(u64,String)>("SELECT t.id,u.email FROM account_tokens t JOIN users u ON u.id=t.user_id WHERE t.token_hash=? AND t.token_type='invitation' AND t.mailed_at IS NOT NULL AND t.expires_at>? AND t.consumed_at IS NULL AND u.status='invitation_pending' FOR UPDATE")
            .bind(token_hash(invitation_token).to_vec()).bind(now).fetch_optional(&mut *tx).await?.ok_or(CoreError::Unauthorized)?;
        let user_id: u64 = sqlx::query_scalar("SELECT user_id FROM account_tokens WHERE id=?")
            .bind(row.0)
            .fetch_one(&mut *tx)
            .await?;
        sqlx::query("UPDATE account_tokens SET consumed_at=? WHERE id=?")
            .bind(now)
            .bind(row.0)
            .execute(&mut *tx)
            .await?;
        sqlx::query("UPDATE users SET password_hash=?,totp_secret_enc=?,totp_enabled=0,status='totp_setup_pending' WHERE id=?")
            .bind(password_hash).bind(&encrypted_seed).bind(user_id).execute(&mut *tx).await?;
        sqlx::query("INSERT INTO totp_enrollment_tokens (user_id,token_hash,secret_ciphertext,token_type,expires_at) VALUES (?,?,?,'invitation',?)")
            .bind(user_id).bind(token_hash(&setup_token).to_vec()).bind(&encrypted_seed).bind(expires_at).execute(&mut *tx).await?;
        insert_audit(
            &mut tx,
            Some(user_id),
            "core",
            "account.invitation.accept",
            "user",
            Some(user_id.to_string()),
            "success",
        )
        .await?;
        tx.commit().await?;
        let label = url::form_urlencoded::byte_serialize(row.1.as_bytes()).collect::<String>();
        Ok(BootstrapBegin {
            setup_token,
            totp_secret: seed.clone(),
            otpauth_uri: format!(
                "otpauth://totp/Bastion:{label}?secret={seed}&issuer=Bastion&algorithm=SHA1&digits=6&period=30"
            ),
            expires_at,
        })
    }

    pub async fn complete_invitation_activation(
        &self,
        setup_token: &str,
        code: &str,
    ) -> Result<BootstrapComplete, CoreError> {
        let now = Utc::now().naive_utc();
        let mut tx = self.database.pool.begin().await?;
        let setup=sqlx::query_as::<_,(u64,u64,Vec<u8>,u8)>("SELECT id,user_id,secret_ciphertext,failed_attempts FROM totp_enrollment_tokens WHERE token_hash=? AND token_type='invitation' AND consumed_at IS NULL AND expires_at>? FOR UPDATE")
            .bind(token_hash(setup_token).to_vec()).bind(now).fetch_optional(&mut *tx).await?.ok_or(CoreError::Unauthorized)?;
        let last: Option<i64> = sqlx::query_scalar::<_, Option<i64>>(
            "SELECT last_totp_counter FROM users WHERE id=? AND status='totp_setup_pending'",
        )
        .bind(setup.1)
        .fetch_one(&mut *tx)
        .await?;
        let secret = self.decrypt(&setup.2)?;
        let Some(counter) = verify_totp(&secret, code, last.unwrap_or(-1)) else {
            let attempts = setup.3.saturating_add(1);
            sqlx::query("UPDATE totp_enrollment_tokens SET failed_attempts=?,consumed_at=IF(?, ?, consumed_at) WHERE id=?")
                .bind(attempts).bind(attempts>=3).bind(now).bind(setup.0).execute(&mut *tx).await?;
            if attempts >= 3 {
                sqlx::query("UPDATE users SET status='invitation_pending',password_hash=NULL,totp_secret_enc=NULL,totp_enabled=0 WHERE id=? AND status='totp_setup_pending'")
                    .bind(setup.1).execute(&mut *tx).await?;
            }
            sqlx::query("INSERT INTO security_events (user_id,event_type,result,ip_address,device_label,created_at) VALUES (?,'totp_enrollment','failed','','',?)")
                .bind(setup.1).bind(now).execute(&mut *tx).await?;
            tx.commit().await?;
            return Err(CoreError::Unauthorized);
        };
        let recovery_codes = (0..10).map(|_| recovery_code()).collect::<Vec<_>>();
        for recovery_code in &recovery_codes {
            sqlx::query("INSERT INTO totp_recovery_codes (user_id,code_hash) VALUES (?,?)")
                .bind(setup.1)
                .bind(token_hash(recovery_code).to_vec())
                .execute(&mut *tx)
                .await?;
        }
        sqlx::query("UPDATE users SET status='active',totp_enabled=1,last_totp_counter=? WHERE id=? AND status='totp_setup_pending'")
            .bind(counter as i64).bind(setup.1).execute(&mut *tx).await?;
        sqlx::query("UPDATE totp_enrollment_tokens SET consumed_at=? WHERE id=?")
            .bind(now)
            .bind(setup.0)
            .execute(&mut *tx)
            .await?;
        sqlx::query("INSERT INTO security_events (user_id,event_type,result,ip_address,device_label,created_at) VALUES (?,'totp_enrollment','success','','',?)")
            .bind(setup.1).bind(now).execute(&mut *tx).await?;
        insert_audit(
            &mut tx,
            Some(setup.1),
            "core",
            "account.activation.complete",
            "user",
            Some(setup.1.to_string()),
            "success",
        )
        .await?;
        tx.commit().await?;
        Ok(BootstrapComplete {
            recovery_codes,
            status: "active",
        })
    }

    async fn enqueue_account_email(
        &self,
        sender_user_id: u64,
        user_id: u64,
        token_type: &str,
        target_email: Option<&str>,
        recipient: &str,
        subject: &str,
        body_intro: &str,
    ) -> Result<(), CoreError> {
        let raw_token = random_token();
        let now = Utc::now().naive_utc();
        let mut tx = self.database.pool.begin().await?;
        sqlx::query("UPDATE account_tokens SET consumed_at=? WHERE user_id=? AND token_type=? AND consumed_at IS NULL")
            .bind(now).bind(user_id).bind(token_type).execute(&mut *tx).await?;
        sqlx::query("UPDATE mail_outbox SET delivery_status='cancelled',body_ciphertext='' WHERE account_token_id IN (SELECT id FROM account_tokens WHERE user_id=? AND token_type=? AND consumed_at=?) AND delivery_status IN ('pending','failed','delivering')")
            .bind(user_id).bind(token_type).bind(now).execute(&mut *tx).await?;
        let token_id=sqlx::query("INSERT INTO account_tokens (user_id,token_hash,token_type,target_email,created_by,created_at) VALUES (?,?,?,?,?,?)")
            .bind(user_id).bind(token_hash(&raw_token).to_vec()).bind(token_type).bind(target_email).bind(sender_user_id).bind(now).execute(&mut *tx).await?.last_insert_id();
        let route = if token_type == "invitation" {
            "activate"
        } else {
            "reset-password"
        };
        let link = format!("{}/{route}?token={raw_token}", self.public_app_origin);
        let body = format!("{body_intro}\n\n{link}");
        let encrypted_body = self.encrypt(&body)?;
        sqlx::query("INSERT INTO mail_outbox (kind,sender_user_id,recipient_email,subject,body_ciphertext,account_token_id,delivery_status,available_at,created_at) VALUES (?,?,?,?,?,?,'pending',?,?)")
            .bind(token_type).bind(sender_user_id).bind(recipient).bind(subject).bind(encrypted_body).bind(token_id).bind(now).bind(now).execute(&mut *tx).await?;
        insert_audit(
            &mut tx,
            Some(sender_user_id),
            "core",
            "account.email.queued",
            "user",
            Some(user_id.to_string()),
            "success",
        )
        .await?;
        tx.commit().await?;
        Ok(())
    }

    async fn require_system_administrator(&self, user_id: u64) -> Result<(), CoreError> {
        let admin:Option<u64>=sqlx::query_scalar("SELECT u.id FROM users u JOIN roles r ON r.id=u.role_id WHERE u.id=? AND u.status='active' AND r.system_key='system_administrator'").bind(user_id).fetch_optional(&self.database.pool).await?;
        if admin.is_some() {
            Ok(())
        } else {
            Err(CoreError::Forbidden)
        }
    }

    pub async fn deliver_one_mail(&self) -> Result<bool, CoreError> {
        let now = Utc::now().naive_utc();
        let mut tx = self.database.pool.begin().await?;
        let mail = sqlx::query_as::<_,PendingMail>("SELECT id,sender_user_id,recipient_email,subject,body_ciphertext,account_token_id FROM mail_outbox WHERE delivery_status IN ('pending','delivering') AND available_at<=? ORDER BY id LIMIT 1 FOR UPDATE")
            .bind(now).fetch_optional(&mut *tx).await?;
        let Some(mail) = mail else {
            tx.commit().await?;
            return Ok(false);
        };
        sqlx::query("UPDATE mail_outbox SET delivery_status='delivering',attempts=attempts+1,available_at=? WHERE id=?")
            .bind(now+chrono::Duration::minutes(5)).bind(mail.id).execute(&mut *tx).await?;
        tx.commit().await?;

        let credentials = sqlx::query_as::<_,(String,Vec<u8>)>("SELECT c.smtp_email,c.password_ciphertext FROM user_smtp_credentials c JOIN users u ON u.id=c.user_id WHERE c.user_id=? AND u.status='active'")
            .bind(mail.sender_user_id).fetch_optional(&self.database.pool).await?;
        let Some((smtp_email, encrypted_password)) = credentials else {
            self.fail_mail(mail.id, "sender_credentials_missing")
                .await?;
            return Ok(true);
        };
        let settings = sqlx::query_as::<_,(String,serde_json::Value)>("SELECT setting_key,value_json FROM settings WHERE module_id='core' AND scope_key='global' AND setting_key IN ('smtp.host','smtp.port','smtp.tls_mode')")
            .fetch_all(&self.database.pool).await?;
        let mut host = None;
        let mut port = None;
        let mut tls_mode = None;
        for (key, value) in settings {
            match key.as_str() {
                "smtp.host" => host = value.as_str().map(str::to_owned),
                "smtp.port" => port = value.as_u64().and_then(|n| u16::try_from(n).ok()),
                "smtp.tls_mode" => tls_mode = value.as_str().map(str::to_owned),
                _ => {}
            }
        }
        let (Some(host), Some(port), Some(tls_mode)) = (host, port, tls_mode) else {
            self.fail_mail(mail.id, "smtp_configuration_missing")
                .await?;
            return Ok(true);
        };
        if !matches!(port, 465 | 587)
            || !matches!(tls_mode.as_str(), "implicit_tls" | "starttls_required")
            || (port == 465) != (tls_mode == "implicit_tls")
        {
            self.fail_mail(mail.id, "smtp_configuration_invalid")
                .await?;
            return Ok(true);
        }
        if !smtp_host_resolves_publicly(&host, port).await {
            self.fail_mail(mail.id, "smtp_host_not_public").await?;
            return Ok(true);
        }
        let password = match self.decrypt(&encrypted_password) {
            Ok(password) => password,
            Err(_) => {
                self.fail_mail(mail.id, "credential_decryption_failed")
                    .await?;
                return Ok(true);
            }
        };
        let body = match self.decrypt(&mail.body_ciphertext) {
            Ok(body) => body,
            Err(_) => {
                self.fail_mail(mail.id, "message_decryption_failed").await?;
                return Ok(true);
            }
        };
        let message = match Message::builder()
            .from(match smtp_email.parse::<Mailbox>() {
                Ok(m) => m,
                Err(_) => {
                    self.fail_mail(mail.id, "sender_address_invalid").await?;
                    return Ok(true);
                }
            })
            .to(match mail.recipient_email.parse::<Mailbox>() {
                Ok(m) => m,
                Err(_) => {
                    self.fail_mail(mail.id, "recipient_address_invalid").await?;
                    return Ok(true);
                }
            })
            .subject(mail.subject)
            .body(body)
        {
            Ok(message) => message,
            Err(_) => {
                self.fail_mail(mail.id, "message_invalid").await?;
                return Ok(true);
            }
        };
        let builder = match tls_mode.as_str() {
            "implicit_tls" => AsyncSmtpTransport::<Tokio1Executor>::relay(&host),
            "starttls_required" => AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&host),
            _ => unreachable!(),
        };
        let transport = match builder {
            Ok(builder) => builder
                .port(port)
                .credentials(Credentials::new(smtp_email, password))
                .timeout(Some(std::time::Duration::from_secs(15)))
                .build(),
            Err(_) => {
                self.fail_mail(mail.id, "smtp_tls_configuration_invalid")
                    .await?;
                return Ok(true);
            }
        };
        match transport.send(message).await {
            Ok(_) => {
                let mut tx = self.database.pool.begin().await?;
                sqlx::query("UPDATE mail_outbox SET delivery_status='sent',sent_at=?,last_error_code=NULL,body_ciphertext='' WHERE id=? AND delivery_status='delivering'")
                    .bind(Utc::now().naive_utc()).bind(mail.id).execute(&mut *tx).await?;
                if let Some(token_id) = mail.account_token_id {
                    let sent_at = Utc::now().naive_utc();
                    sqlx::query("UPDATE account_tokens SET mailed_at=?,expires_at=CASE token_type WHEN 'invitation' THEN ?+INTERVAL 24 HOUR WHEN 'password_reset' THEN ?+INTERVAL 1 HOUR WHEN 'email_change' THEN ?+INTERVAL 1 HOUR ELSE expires_at END WHERE id=? AND consumed_at IS NULL")
                        .bind(sent_at).bind(sent_at).bind(sent_at).bind(sent_at).bind(token_id).execute(&mut *tx).await?;
                }
                tx.commit().await?;
            }
            Err(_) => self.fail_mail(mail.id, "smtp_delivery_failed").await?,
        }
        Ok(true)
    }

    async fn fail_mail(&self, mail_id: u64, code: &str) -> Result<(), CoreError> {
        sqlx::query("UPDATE mail_outbox SET delivery_status='failed',last_error_code=?,available_at=UTC_TIMESTAMP(6)+INTERVAL 5 MINUTE WHERE id=? AND delivery_status='delivering'")
            .bind(code).bind(mail_id).execute(&self.database.pool).await?;
        Ok(())
    }

    pub async fn verify_session_totp(
        &self,
        session_token: &str,
        code: &str,
        ip: &str,
        user_agent: &str,
    ) -> Result<(), CoreError> {
        if session_token.is_empty() {
            return Err(CoreError::Unauthorized);
        }
        let now = Utc::now().naive_utc();
        let mut tx = self.database.pool.begin().await?;
        let row = sqlx::query_as::<_, (u64, u64, Option<Vec<u8>>, Option<i64>, Option<NaiveDateTime>, Option<NaiveDateTime>, bool, NaiveDateTime)>(
            "SELECT s.id,s.user_id,u.totp_secret_enc,u.last_totp_counter,s.revoked_at,s.expires_at,s.keep_logged_in,s.last_activity_at FROM sessions s JOIN users u ON u.id=s.user_id WHERE s.token_hash=? AND u.status='active' FOR UPDATE"
        ).bind(token_hash(session_token).to_vec()).fetch_optional(&mut *tx).await?.ok_or(CoreError::Unauthorized)?;
        if row.4.is_some()
            || row.5.is_some_and(|e| e <= now)
            || (!row.6 && row.7 + chrono::Duration::hours(24) <= now)
        {
            return Err(CoreError::Unauthorized);
        }
        if self.is_totp_limited(row.1, ip).await? {
            return Err(CoreError::Locked);
        }
        let secret_enc = row.2.ok_or(CoreError::Unauthorized)?;
        let secret = self.decrypt(&secret_enc)?;
        let Some(counter) = verify_totp(&secret, code, row.3.unwrap_or(-1)) else {
            sqlx::query("INSERT INTO security_events (user_id,event_type,result,ip_address,device_label,created_at) VALUES (?,'totp_reauth','failed',?,?,?)")
                .bind(row.1).bind(ip).bind(safe_agent(user_agent)).bind(now).execute(&mut *tx).await?;
            tx.commit().await?;
            self.record_totp_failure(row.1, ip).await?;
            return Err(CoreError::Unauthorized);
        };
        sqlx::query("UPDATE users SET last_totp_counter=? WHERE id=?")
            .bind(counter as i64)
            .bind(row.1)
            .execute(&mut *tx)
            .await?;
        sqlx::query("UPDATE sessions SET last_totp_at=?,last_activity_at=? WHERE id=?")
            .bind(now)
            .bind(now)
            .bind(row.0)
            .execute(&mut *tx)
            .await?;
        sqlx::query("UPDATE trusted_devices SET last_totp_at=?,expires_at=? WHERE user_id=? AND revoked_at IS NULL AND expires_at>? ORDER BY last_used_at DESC LIMIT 1")
            .bind(now).bind(now + chrono::Duration::days(15)).bind(row.1).bind(now).execute(&mut *tx).await?;
        sqlx::query("INSERT INTO security_events (user_id,event_type,result,ip_address,device_label,created_at) VALUES (?,'totp_reauth','success',?,?,?)")
            .bind(row.1).bind(ip).bind(safe_agent(user_agent)).bind(now).execute(&mut *tx).await?;
        insert_audit(
            &mut tx,
            Some(row.1),
            "core",
            "auth.totp_reauth",
            "session",
            None,
            "success",
        )
        .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn logout(&self, session_token: &str) -> Result<(), CoreError> {
        if session_token.is_empty() {
            return Ok(());
        }
        let now = Utc::now().naive_utc();
        sqlx::query("UPDATE sessions SET revoked_at=?,revocation_reason='logout' WHERE token_hash=? AND revoked_at IS NULL")
            .bind(now).bind(token_hash(session_token).to_vec()).execute(&self.database.pool).await?;
        Ok(())
    }

    pub async fn csrf_matches(
        &self,
        session_token: &str,
        csrf_token: &str,
    ) -> Result<bool, CoreError> {
        let stored: Option<Vec<u8>> = sqlx::query_scalar(
            "SELECT csrf_hash FROM sessions WHERE token_hash=? AND revoked_at IS NULL",
        )
        .bind(token_hash(session_token).to_vec())
        .fetch_optional(&self.database.pool)
        .await?;
        let expected = token_hash(csrf_token);
        Ok(stored.is_some_and(|hash| bool::from(hash.as_slice().ct_eq(expected.as_slice()))))
    }

    pub async fn revoke_session(
        &self,
        session_token: &str,
        session_id: u64,
        user_id: u64,
    ) -> Result<(), CoreError> {
        let now = Utc::now().naive_utc();
        let current: Option<u64> = sqlx::query_scalar(
            "SELECT id FROM sessions WHERE token_hash=? AND user_id=? AND revoked_at IS NULL",
        )
        .bind(token_hash(session_token).to_vec())
        .bind(user_id)
        .fetch_optional(&self.database.pool)
        .await?;
        if current.is_none() {
            return Err(CoreError::Unauthorized);
        }
        let result = sqlx::query("UPDATE sessions SET revoked_at=?,revocation_reason='user_revoked' WHERE id=? AND user_id=? AND revoked_at IS NULL")
            .bind(now).bind(session_id).bind(user_id).execute(&self.database.pool).await?;
        if result.rows_affected() == 0 {
            return Err(CoreError::Invalid);
        }
        self.audit(
            Some(user_id),
            "core",
            "session.revoke",
            "session",
            session_id.to_string(),
            "success",
        )
        .await?;
        Ok(())
    }

    pub async fn list_sessions(
        &self,
        user_id: u64,
        session_token: &str,
    ) -> Result<Vec<SessionInfo>, CoreError> {
        let current: Option<u64> = sqlx::query_scalar(
            "SELECT id FROM sessions WHERE token_hash=? AND user_id=? AND revoked_at IS NULL",
        )
        .bind(token_hash(session_token).to_vec())
        .bind(user_id)
        .fetch_optional(&self.database.pool)
        .await?;
        if current.is_none() {
            return Err(CoreError::Unauthorized);
        }
        sqlx::query_as::<_,SessionInfo>("SELECT id,user_agent AS device,ip_address,started_at,last_activity_at,(token_hash=?) AS current FROM sessions WHERE user_id=? AND revoked_at IS NULL AND (expires_at IS NULL OR expires_at>UTC_TIMESTAMP(6)) AND (keep_logged_in=1 OR last_activity_at>UTC_TIMESTAMP(6)-INTERVAL 24 HOUR) ORDER BY started_at DESC,id DESC")
            .bind(token_hash(session_token).to_vec()).bind(user_id).fetch_all(&self.database.pool).await.map_err(Into::into)
    }

    pub async fn revoke_other_sessions(
        &self,
        user_id: u64,
        session_token: &str,
    ) -> Result<u64, CoreError> {
        let now = Utc::now().naive_utc();
        let current: Option<u64> = sqlx::query_scalar(
            "SELECT id FROM sessions WHERE token_hash=? AND user_id=? AND revoked_at IS NULL",
        )
        .bind(token_hash(session_token).to_vec())
        .bind(user_id)
        .fetch_optional(&self.database.pool)
        .await?;
        if current.is_none() {
            return Err(CoreError::Unauthorized);
        }
        let result=sqlx::query("UPDATE sessions SET revoked_at=?,revocation_reason='user_revoked' WHERE user_id=? AND token_hash<>? AND revoked_at IS NULL")
            .bind(now).bind(user_id).bind(token_hash(session_token).to_vec()).execute(&self.database.pool).await?;
        self.audit(
            Some(user_id),
            "core",
            "session.revoke_others",
            "session",
            user_id.to_string(),
            "success",
        )
        .await?;
        Ok(result.rows_affected())
    }

    pub async fn list_trusted_devices(
        &self,
        user_id: u64,
    ) -> Result<Vec<TrustedDeviceInfo>, CoreError> {
        sqlx::query_as::<_,TrustedDeviceInfo>("SELECT id,device_label AS device,ip_address,created_at,last_used_at,expires_at FROM trusted_devices WHERE user_id=? AND revoked_at IS NULL AND expires_at>UTC_TIMESTAMP(6) ORDER BY last_used_at DESC,id DESC")
            .bind(user_id).fetch_all(&self.database.pool).await.map_err(Into::into)
    }

    pub async fn revoke_trusted_device(
        &self,
        user_id: u64,
        device_id: u64,
    ) -> Result<(), CoreError> {
        let now = Utc::now().naive_utc();
        let result=sqlx::query("UPDATE trusted_devices SET revoked_at=? WHERE id=? AND user_id=? AND revoked_at IS NULL")
            .bind(now).bind(device_id).bind(user_id).execute(&self.database.pool).await?;
        if result.rows_affected() == 0 {
            return Err(CoreError::Invalid);
        }
        self.audit(
            Some(user_id),
            "core",
            "device.revoke",
            "trusted_device",
            device_id.to_string(),
            "success",
        )
        .await
    }

    pub async fn security_history(
        &self,
        user_id: u64,
        page: u32,
        page_size: u32,
    ) -> Result<Vec<SecurityEventInfo>, CoreError> {
        let page = page.max(1);
        let size = page_size.clamp(1, 100);
        let offset = u64::from(page - 1)
            .checked_mul(u64::from(size))
            .ok_or(CoreError::Invalid)?;
        sqlx::query_as::<_,SecurityEventInfo>("SELECT event_type,result,ip_address,device_label AS device,created_at FROM security_events WHERE user_id=? AND created_at>=UTC_TIMESTAMP(6)-INTERVAL 30 DAY ORDER BY created_at DESC,id DESC LIMIT ? OFFSET ?")
            .bind(user_id).bind(size).bind(offset).fetch_all(&self.database.pool).await.map_err(Into::into)
    }

    async fn create_session(
        &self,
        user_id: u64,
        ip: &str,
        agent: &str,
        keep: bool,
        now: NaiveDateTime,
        last_totp_at: NaiveDateTime,
        existing_device: Option<String>,
    ) -> Result<AuthCookies, CoreError> {
        let session_token = random_token();
        let csrf_token = random_token();
        let expires_at = None;
        let mut tx = self.database.pool.begin().await?;
        let active: Option<String> =
            sqlx::query_scalar("SELECT status FROM users WHERE id=? FOR UPDATE")
                .bind(user_id)
                .fetch_optional(&mut *tx)
                .await?;
        if active.as_deref() != Some("active") {
            return Err(CoreError::Unauthorized);
        }
        sqlx::query("UPDATE sessions SET revoked_at=?,revocation_reason='replaced' WHERE user_id=? AND revoked_at IS NULL")
            .bind(now).bind(user_id).execute(&mut *tx).await?;
        sqlx::query("INSERT INTO sessions (user_id,token_hash,csrf_hash,user_agent,ip_address,keep_logged_in,started_at,last_activity_at,last_totp_at,expires_at) VALUES (?,?,?,?,?,?,?,?,?,?)")
            .bind(user_id).bind(token_hash(&session_token).to_vec()).bind(token_hash(&csrf_token).to_vec()).bind(agent).bind(ip).bind(keep).bind(now).bind(now).bind(last_totp_at).bind(expires_at)
            .execute(&mut *tx).await?;
        insert_audit(
            &mut tx,
            Some(user_id),
            "core",
            "auth.login",
            "session",
            None,
            "success",
        )
        .await?;
        tx.commit().await?;
        Ok(AuthCookies {
            session_token,
            csrf_token,
            device_token: existing_device.unwrap_or_default(),
            expires_at,
            keep_logged_in: keep,
        })
    }

    async fn trust_device(
        &self,
        user_id: u64,
        prior_token: Option<&str>,
        ip: &str,
        agent: &str,
        now: NaiveDateTime,
    ) -> Result<(String, bool), CoreError> {
        let prior_hash = prior_token
            .filter(|s| !s.is_empty())
            .map(|s| token_hash(s).to_vec());
        let existing_id = if let Some(hash) = &prior_hash {
            sqlx::query_scalar::<_, u64>("SELECT id FROM trusted_devices WHERE user_id=? AND token_hash=? AND revoked_at IS NULL")
                .bind(user_id).bind(hash).fetch_optional(&self.database.pool).await?
        } else {
            None
        };
        let token = if existing_id.is_some() {
            prior_token.unwrap_or_default().to_owned()
        } else {
            random_token()
        };
        if let Some(id) = existing_id {
            sqlx::query("UPDATE trusted_devices SET ip_address=?,device_label=?,last_used_at=?,last_totp_at=?,expires_at=? WHERE id=?")
                .bind(ip).bind(agent).bind(now).bind(now).bind(now + chrono::Duration::days(15)).bind(id).execute(&self.database.pool).await?;
        } else {
            sqlx::query("INSERT INTO trusted_devices (user_id,token_hash,device_label,ip_address,created_at,last_used_at,last_totp_at,expires_at) VALUES (?,?,?,?,?,?,?,?)")
                .bind(user_id).bind(token_hash(&token).to_vec()).bind(agent).bind(ip).bind(now).bind(now).bind(now).bind(now + chrono::Duration::days(15)).execute(&self.database.pool).await?;
        }
        Ok((token, existing_id.is_none()))
    }

    async fn notify_new_device(&self, user_id: u64) {
        let result=async {
            let email:String=sqlx::query_scalar("SELECT email FROM users WHERE id=? AND status='active'").bind(user_id).fetch_one(&self.database.pool).await?;
            self.enqueue_plain_email(user_id,&email,"Nuevo dispositivo conectado a Bastion","Se inició sesión correctamente en tu cuenta desde un dispositivo nuevo. Si no fuiste tú, contacta al Administrador del sistema y cambia tu contraseña.").await
        }.await;
        if result.is_err() {
            tracing::warn!(user_id, "could not queue new-device security notice");
        }
    }

    async fn enqueue_plain_email(
        &self,
        sender_user_id: u64,
        recipient: &str,
        subject: &str,
        body: &str,
    ) -> Result<(), CoreError> {
        let ciphertext = self.encrypt(body)?;
        let now = Utc::now().naive_utc();
        sqlx::query("INSERT INTO mail_outbox (kind,sender_user_id,recipient_email,subject,body_ciphertext,delivery_status,available_at,created_at) VALUES ('security_notice',?,?,?,?, 'pending',?,?)")
            .bind(sender_user_id).bind(recipient).bind(subject).bind(ciphertext).bind(now).bind(now).execute(&self.database.pool).await?;
        Ok(())
    }

    async fn record_password_failure(
        &self,
        tx: &mut Transaction<'_, MySql>,
        user: &AuthUser,
        ip: &str,
    ) -> Result<(), CoreError> {
        let now = Utc::now().naive_utc();
        let failures = user.failed_password_count.saturating_add(1);
        let mut level = user.lock_level;
        if failures >= 3 {
            level = level.saturating_add(1).min(7);
            let minutes = (30i64.saturating_mul(1i64 << level.saturating_sub(1))).min(1440);
            let locked_until = now + chrono::Duration::minutes(minutes);
            sqlx::query(
                "UPDATE users SET failed_password_count=0,lock_level=?,locked_until=? WHERE id=?",
            )
            .bind(level)
            .bind(locked_until)
            .bind(user.id)
            .execute(&mut **tx)
            .await?;
            let alert_due = user
                .incident_alerted_at
                .is_none_or(|last| last + chrono::Duration::hours(24) <= now);
            if alert_due {
                sqlx::query("INSERT INTO admin_notifications (recipient_user_id,kind,subject,payload_json,delivery_status,created_at) SELECT u.id,'login_lock','Bloqueo por intentos de acceso',JSON_OBJECT('affected_user_id',?,'locked_until',?), 'pending', ? FROM users u JOIN roles r ON r.id=u.role_id WHERE u.status='active' AND r.system_key='system_administrator'")
                    .bind(user.id).bind(locked_until).bind(now).execute(&mut **tx).await?;
                sqlx::query("UPDATE users SET incident_alerted_at=? WHERE id=?")
                    .bind(now)
                    .bind(user.id)
                    .execute(&mut **tx)
                    .await?;
            }
        } else {
            sqlx::query("UPDATE users SET failed_password_count=? WHERE id=?")
                .bind(failures)
                .bind(user.id)
                .execute(&mut **tx)
                .await?;
        }
        sqlx::query("INSERT INTO security_events (user_id,event_type,result,ip_address,device_label,created_at) VALUES (?,'password_login','failed',?,'',?)")
            .bind(user.id).bind(ip).bind(now).execute(&mut **tx).await?;
        Ok(())
    }

    async fn is_totp_limited(&self, user_id: u64, ip: &str) -> Result<bool, CoreError> {
        let since = Utc::now().naive_utc() - chrono::Duration::minutes(15);
        let account_block: Option<NaiveDateTime> = sqlx::query_scalar("SELECT MAX(created_at) FROM security_events WHERE user_id=? AND event_type='totp_rate_limit' AND created_at>?")
            .bind(user_id).bind(since).fetch_one(&self.database.pool).await?;
        let ip_block: Option<NaiveDateTime> = sqlx::query_scalar("SELECT MAX(created_at) FROM security_events WHERE user_id IS NULL AND event_type='totp_ip_rate_limit' AND ip_address=? AND created_at>?")
            .bind(ip).bind(since).fetch_one(&self.database.pool).await?;
        Ok(account_block.is_some() || ip_block.is_some())
    }

    async fn record_totp_failure(&self, user_id: u64, ip: &str) -> Result<(), CoreError> {
        let now = Utc::now().naive_utc();
        let since = now - chrono::Duration::minutes(15);
        let account_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM security_events WHERE user_id=? AND event_type IN ('totp_login','recovery_login','totp_reauth','totp_sensitive_action') AND result='failed' AND created_at>?")
            .bind(user_id).bind(since).fetch_one(&self.database.pool).await?;
        if account_count >= 10 {
            sqlx::query("INSERT INTO security_events (user_id,event_type,result,ip_address,device_label,created_at) VALUES (?,'totp_rate_limit','blocked',?,'',?)")
                .bind(user_id).bind(ip).bind(now).execute(&self.database.pool).await?;
        }
        let ip_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM security_events WHERE event_type IN ('totp_login','recovery_login','totp_reauth','totp_sensitive_action') AND result='failed' AND ip_address=? AND created_at>?")
            .bind(ip).bind(since).fetch_one(&self.database.pool).await?;
        if ip_count >= 30 {
            sqlx::query("INSERT INTO security_events (user_id,event_type,result,ip_address,device_label,created_at) VALUES (NULL,'totp_ip_rate_limit','blocked',?,'',?)")
                .bind(ip).bind(now).execute(&self.database.pool).await?;
        }
        Ok(())
    }

    fn encrypt(&self, value: &str) -> Result<Vec<u8>, CoreError> {
        let nonce = random_bytes::<12>();
        let ciphertext = self
            .cipher
            .encrypt(Nonce::from_slice(&nonce), value.as_bytes())
            .map_err(|_| CoreError::Invalid)?;
        let mut packed = nonce.to_vec();
        packed.extend(ciphertext);
        Ok(packed)
    }

    fn decrypt(&self, packed: &[u8]) -> Result<String, CoreError> {
        if packed.len() < 12 {
            return Err(CoreError::Invalid);
        }
        let (nonce, ciphertext) = packed.split_at(12);
        let plain = self
            .cipher
            .decrypt(Nonce::from_slice(nonce), ciphertext)
            .map_err(|_| CoreError::Invalid)?;
        String::from_utf8(plain).map_err(|_| CoreError::Invalid)
    }

    async fn log_security(
        &self,
        user_id: Option<u64>,
        kind: &str,
        result: &str,
        ip: &str,
        agent: String,
    ) -> Result<(), CoreError> {
        sqlx::query("INSERT INTO security_events (user_id,event_type,result,ip_address,device_label,created_at) VALUES (?,?,?,?,?,?)")
            .bind(user_id).bind(kind).bind(result).bind(ip).bind(agent).bind(Utc::now().naive_utc()).execute(&self.database.pool).await?;
        Ok(())
    }

    async fn audit(
        &self,
        actor: Option<u64>,
        module: &str,
        action: &str,
        target_type: &str,
        target_id: String,
        result: &str,
    ) -> Result<(), CoreError> {
        sqlx::query("INSERT INTO audit_events (actor_id,module_id,action,target_type,target_id,result,created_at) VALUES (?,?,?,?,?,?,?)")
            .bind(actor).bind(module).bind(action).bind(target_type).bind(target_id).bind(result).bind(Utc::now().naive_utc()).execute(&self.database.pool).await?;
        Ok(())
    }
}

async fn revoke_user_access(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    now: NaiveDateTime,
    reason: &str,
) -> Result<(), CoreError> {
    sqlx::query("UPDATE sessions SET revoked_at=?,revocation_reason=? WHERE user_id=? AND revoked_at IS NULL").bind(now).bind(reason).bind(user_id).execute(&mut **tx).await?;
    sqlx::query("UPDATE trusted_devices SET revoked_at=? WHERE user_id=? AND revoked_at IS NULL")
        .bind(now)
        .bind(user_id)
        .execute(&mut **tx)
        .await?;
    sqlx::query("UPDATE otp_challenges SET consumed_at=? WHERE user_id=? AND consumed_at IS NULL")
        .bind(now)
        .bind(user_id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

async fn insert_audit(
    tx: &mut Transaction<'_, MySql>,
    actor: Option<u64>,
    module: &str,
    action: &str,
    target_type: &str,
    target_id: Option<String>,
    result: &str,
) -> Result<(), CoreError> {
    sqlx::query("INSERT INTO audit_events (actor_id,module_id,action,target_type,target_id,result,created_at) VALUES (?,?,?,?,?,?,?)")
        .bind(actor).bind(module).bind(action).bind(target_type).bind(target_id).bind(result).bind(Utc::now().naive_utc()).execute(&mut **tx).await?;
    Ok(())
}

fn random_bytes<const N: usize>() -> [u8; N] {
    let mut bytes = [0u8; N];
    OsRng.fill_bytes(&mut bytes);
    bytes
}
fn random_token() -> String {
    base64::Engine::encode(
        &base64::engine::general_purpose::URL_SAFE_NO_PAD,
        random_bytes::<32>(),
    )
}
fn token_hash(value: &str) -> [u8; 32] {
    Sha256::digest(value.as_bytes()).into()
}
fn constant_time_text_eq(left: &str, right: &str) -> bool {
    left.as_bytes().ct_eq(right.as_bytes()).into()
}
fn hash_password(password: &str) -> Result<String, CoreError> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|_| CoreError::Invalid)
}
fn verify_password(password: &str, encoded: &str) -> bool {
    PasswordHash::new(encoded).ok().is_some_and(|parsed| {
        Argon2::default()
            .verify_password(password.as_bytes(), &parsed)
            .is_ok()
    })
}
fn map_unique_conflict(error: sqlx::Error) -> CoreError {
    if let sqlx::Error::Database(db) = &error {
        if db.code().as_deref() == Some("23000") {
            return CoreError::Conflict;
        }
    }
    CoreError::Internal(error)
}
fn validate_password(password: &str) -> Result<(), CoreError> {
    if password.chars().count() < 8 || password.len() > 1024 {
        return Err(CoreError::Invalid);
    }
    Ok(())
}
fn validate_password_input(password: &str) -> Result<(), CoreError> {
    if password.len() > 1024 {
        Err(CoreError::Invalid)
    } else {
        Ok(())
    }
}
fn normalize_email(email: &str) -> Result<String, CoreError> {
    let normalized = email.trim().to_lowercase();
    if normalized.is_empty()
        || normalized.len() > 254
        || !normalized.is_ascii()
        || !normalized.contains('@')
        || normalized.chars().any(char::is_control)
    {
        return Err(CoreError::Invalid);
    }
    Ok(normalized)
}
fn safe_agent(agent: &str) -> String {
    agent
        .chars()
        .filter(|c| !c.is_control())
        .take(180)
        .collect()
}
async fn smtp_host_resolves_publicly(host: &str, port: u16) -> bool {
    if host.is_empty()
        || host.len() > 253
        || host.parse::<IpAddr>().is_ok()
        || host
            .chars()
            .any(|c| c.is_control() || c.is_whitespace() || matches!(c, '/' | '\\' | '@' | ':'))
    {
        return false;
    }
    let resolved = tokio::time::timeout(
        Duration::from_secs(5),
        tokio::net::lookup_host((host, port)),
    )
    .await;
    let Ok(Ok(addresses)) = resolved else {
        return false;
    };
    let addresses = addresses.collect::<Vec<_>>();
    !addresses.is_empty() && addresses.iter().all(|address| is_public_ip(address.ip()))
}
fn is_public_ip(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(ip) => is_public_ipv4(ip),
        IpAddr::V6(ip) => {
            if let Some(mapped) = ip.to_ipv4_mapped() {
                return is_public_ipv4(mapped);
            }
            let octets = ip.octets();
            let global_unicast = octets[0] & 0xe0 == 0x20;
            let special_2001 = octets[0] == 0x20 && octets[1] == 0x01 && octets[2] < 0x02;
            let six_to_four = octets[0] == 0x20 && octets[1] == 0x02;
            let documentation = octets[0] == 0x3f && octets[1] == 0xff;
            global_unicast && !special_2001 && !six_to_four && !documentation
        }
    }
}
fn is_public_ipv4(ip: Ipv4Addr) -> bool {
    let [a, b, c, _] = ip.octets();
    !(a == 0
        || a == 10
        || a == 127
        || (a == 169 && b == 254)
        || (a == 172 && (16..=31).contains(&b))
        || (a == 192 && b == 168)
        || (a == 100 && (64..=127).contains(&b))
        || (a == 192 && b == 0)
        || (a == 192 && b == 0 && c == 2)
        || (a == 198 && (b == 18 || b == 19))
        || (a == 198 && b == 51 && c == 100)
        || (a == 203 && b == 0 && c == 113)
        || a >= 224)
}
fn recovery_code() -> String {
    hex::encode(random_bytes::<8>()).to_uppercase()
}
fn verify_totp(secret: &str, code: &str, last_counter: i64) -> Option<u64> {
    if code.len() != 6 || !code.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let secret = BASE32_NOPAD.decode(secret.as_bytes()).ok()?;
    let current = (Utc::now().timestamp().max(0) as u64) / 30;
    for counter in [
        current.saturating_sub(1),
        current,
        current.saturating_add(1),
    ] {
        if counter as i64 <= last_counter {
            continue;
        }
        let mut mac = <HmacSha1 as Mac>::new_from_slice(&secret).ok()?;
        mac.update(&counter.to_be_bytes());
        let digest = mac.finalize().into_bytes();
        let offset = (digest[19] & 0x0f) as usize;
        let binary = (u32::from(digest[offset] & 0x7f) << 24)
            | (u32::from(digest[offset + 1]) << 16)
            | (u32::from(digest[offset + 2]) << 8)
            | u32::from(digest[offset + 3]);
        let expected = format!("{:06}", binary % 1_000_000);
        if bool::from(expected.as_bytes().ct_eq(code.as_bytes())) {
            return Some(counter);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_passwords_below_eight_characters_and_accepts_passphrases() {
        assert!(validate_password("short7").is_err());
        assert!(validate_password("a long passphrase with unicode ñ").is_ok());
        assert!(validate_password(&"x".repeat(1025)).is_err());
    }

    #[test]
    fn hashes_are_one_way_and_verification_uses_argon2() {
        let hash = hash_password("correct horse battery staple").expect("hash password");
        assert!(verify_password("correct horse battery staple", &hash));
        assert!(!verify_password("wrong password", &hash));
        assert!(!hash.contains("correct horse"));
    }

    #[test]
    fn recovery_and_session_tokens_are_unpredictable_and_hashed() {
        let a = random_token();
        let b = random_token();
        assert_ne!(a, b);
        assert_eq!(a.len(), 43);
        assert_ne!(token_hash(&a), token_hash(&b));
        assert_eq!(recovery_code().len(), 16);
    }

    #[test]
    fn totp_is_single_use_by_counter() {
        let secret = BASE32_NOPAD.encode(b"12345678901234567890");
        let counter = (Utc::now().timestamp().max(0) as u64 / 30) + 1;
        let code = totp_for_counter(&secret, counter);
        assert_eq!(verify_totp(&secret, &code, -1), Some(counter));
        assert_eq!(verify_totp(&secret, &code, counter as i64), None);
    }

    #[test]
    fn smtp_ip_filter_rejects_local_and_documentation_ranges() {
        assert!(!is_public_ip("127.0.0.1".parse().unwrap()));
        assert!(!is_public_ip("10.1.2.3".parse().unwrap()));
        assert!(!is_public_ip("192.0.2.10".parse().unwrap()));
        assert!(!is_public_ip("::1".parse().unwrap()));
        assert!(is_public_ip("8.8.8.8".parse().unwrap()));
    }
}

#[cfg(test)]
fn totp_for_counter(secret: &str, counter: u64) -> String {
    let secret = BASE32_NOPAD
        .decode(secret.as_bytes())
        .expect("base32 secret");
    let mut mac = <HmacSha1 as Mac>::new_from_slice(&secret).expect("hmac key");
    mac.update(&counter.to_be_bytes());
    let digest = mac.finalize().into_bytes();
    let offset = (digest[19] & 0x0f) as usize;
    let binary = (u32::from(digest[offset] & 0x7f) << 24)
        | (u32::from(digest[offset + 1]) << 16)
        | (u32::from(digest[offset + 2]) << 8)
        | u32::from(digest[offset + 3]);
    format!("{:06}", binary % 1_000_000)
}
