use std::collections::BTreeSet;

use chrono::{Duration, NaiveDateTime};
use sqlx::{MySql, MySqlPool, Transaction, types::Json};

use crate::{
    CodeRecord, Credential, CredentialKind, ExchangeError, GrantState, RefreshRecord,
    S256Challenge, TokenParameters, TokenWindow, validate_code_exchange, validate_refresh_exchange,
    validate_token_window,
};

/// Trusted Core adapter. It must recheck every requested scope against current
/// user, role, module and object-independent policy using this transaction.
/// There is intentionally no permissive default implementation.
pub trait ExchangePolicy {
    fn authorize(
        &self,
        tx: &mut Transaction<'_, MySql>,
        user_id: u64,
        client_id: u64,
        scopes: &BTreeSet<String>,
    ) -> impl std::future::Future<Output = Result<bool, sqlx::Error>> + Send;
}

#[derive(Clone)]
pub struct MysqlTokenExchange<P> {
    pool: MySqlPool,
    policy: P,
}

/// Secrets have no Debug or Clone implementation and must not be logged.
pub struct IssuedTokens {
    pub access_token: Credential,
    pub refresh_token: Credential,
    pub expires_in: u64,
    pub scopes: BTreeSet<String>,
}

/// Server-resolved identity, never taken from a request body or token claims.
pub struct AuthenticatedAccess {
    pub user_id: u64,
    pub client_id: u64,
    pub grant_id: u64,
    pub scopes: BTreeSet<String>,
}

#[derive(Debug)]
pub enum TokenExchangeError {
    InvalidGrant,
    Unavailable,
    Database(sqlx::Error),
}

impl From<sqlx::Error> for TokenExchangeError {
    fn from(value: sqlx::Error) -> Self {
        Self::Database(value)
    }
}

impl<P: ExchangePolicy> MysqlTokenExchange<P> {
    pub fn new(pool: MySqlPool, policy: P) -> Self {
        Self { pool, policy }
    }

    /// Revalidate a bearer token and its current grant on every invocation.
    /// Per-object authorization, quota and audit must still run at the tool.
    pub async fn authenticate_access(
        &self,
        token: &Credential,
        expected_resource: &str,
    ) -> Result<AuthenticatedAccess, TokenExchangeError> {
        if expected_resource.is_empty() {
            return Err(TokenExchangeError::InvalidGrant);
        }
        let digest = token.digest(CredentialKind::AccessToken);
        let mut tx = self.pool.begin().await?;
        let grant_id = sqlx::query_scalar::<_, u64>(
            "SELECT grant_id FROM oauth_access_tokens WHERE token_hash=?",
        )
        .bind(digest.as_slice())
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(TokenExchangeError::InvalidGrant)?;
        let (user_id, client_id) = sqlx::query_as::<_, (u64, u64)>(
            "SELECT user_id,client_id FROM oauth_grants WHERE id=?",
        )
        .bind(grant_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(TokenExchangeError::InvalidGrant)?;
        let status =
            sqlx::query_scalar::<_, String>("SELECT status FROM users WHERE id=? FOR UPDATE")
                .bind(user_id)
                .fetch_optional(&mut *tx)
                .await?
                .ok_or(TokenExchangeError::InvalidGrant)?;
        let (enabled,metadata,approved,method):(bool,Vec<u8>,Option<Vec<u8>>,String)=sqlx::query_as("SELECT enabled,metadata_hash,approved_metadata_hash,token_auth_method FROM oauth_clients WHERE id=? FOR UPDATE")
            .bind(client_id).fetch_optional(&mut *tx).await?.ok_or(TokenExchangeError::InvalidGrant)?;
        let (resource,Json(grant_scopes),issued,expires,revoked):(String,Json<Vec<String>>,NaiveDateTime,NaiveDateTime,Option<NaiveDateTime>)=sqlx::query_as("SELECT resource_uri,scopes_json,issued_at,expires_at,revoked_at FROM oauth_grants WHERE id=? AND user_id=? AND client_id=? FOR UPDATE")
            .bind(grant_id).bind(user_id).bind(client_id).fetch_optional(&mut *tx).await?.ok_or(TokenExchangeError::InvalidGrant)?;
        let (Json(token_scopes),token_issued,token_expires):(Json<Vec<String>>,NaiveDateTime,NaiveDateTime)=sqlx::query_as("SELECT scopes_json,issued_at,expires_at FROM oauth_access_tokens WHERE token_hash=? AND grant_id=? FOR UPDATE")
            .bind(digest.as_slice()).bind(grant_id).fetch_optional(&mut *tx).await?.ok_or(TokenExchangeError::InvalidGrant)?;
        let now: NaiveDateTime = sqlx::query_scalar("SELECT UTC_TIMESTAMP(6)")
            .fetch_one(&mut *tx)
            .await?;
        let ceiling: BTreeSet<String> = sqlx::query_scalar::<_, String>(
            "SELECT scope_name FROM oauth_client_scopes WHERE client_id=?",
        )
        .bind(client_id)
        .fetch_all(&mut *tx)
        .await?
        .into_iter()
        .collect();
        let granted: BTreeSet<String> = grant_scopes.iter().cloned().collect();
        let effective: BTreeSet<String> = token_scopes.iter().cloned().collect();
        if resource != expected_resource
            || method != "none"
            || granted.len() != grant_scopes.len()
            || effective.len() != token_scopes.len()
            || effective.is_empty()
            || !effective.is_subset(&granted)
        {
            return Err(TokenExchangeError::InvalidGrant);
        }
        let state = GrantState {
            issued_at: issued.and_utc().timestamp(),
            expires_at: expires.and_utc().timestamp(),
            revoked: revoked.is_some(),
            user_active: status == "active",
            client_approved: enabled,
            client_metadata_approved: approved.as_deref() == Some(metadata.as_slice()),
            credentials_current: true,
            granted_scopes: &granted,
            client_max_scopes: &ceiling,
            current_user_scopes: &granted,
        };
        let window = TokenWindow {
            issued_at: token_issued.and_utc().timestamp(),
            expires_at: token_expires.and_utc().timestamp(),
        };
        validate_token_window(&state, &window, now.and_utc().timestamp())
            .map_err(|_| TokenExchangeError::InvalidGrant)?;
        if !self
            .policy
            .authorize(&mut tx, user_id, client_id, &effective)
            .await?
        {
            return Err(TokenExchangeError::InvalidGrant);
        }
        tx.commit().await?;
        Ok(AuthenticatedAccess {
            user_id,
            client_id,
            grant_id,
            scopes: effective,
        })
    }

    /// Only public clients with an explicitly approved `none` method are
    /// accepted here. This must be called only by a bounded token endpoint.
    /// User, client, grant and code remain locked until tokens and audit commit.
    pub async fn redeem_code(
        &self,
        request: &TokenParameters,
    ) -> Result<IssuedTokens, TokenExchangeError> {
        let TokenParameters::AuthorizationCode { code, .. } = request else {
            return Err(TokenExchangeError::InvalidGrant);
        };
        let digest = code.digest(CredentialKind::AuthorizationCode);
        let mut tx = self.pool.begin().await?;
        let grant_id = sqlx::query_scalar::<_, u64>(
            "SELECT grant_id FROM oauth_authorization_codes WHERE code_hash=?",
        )
        .bind(digest.as_slice())
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(TokenExchangeError::InvalidGrant)?;
        let (user_id, client_pk) = sqlx::query_as::<_, (u64, u64)>(
            "SELECT user_id,client_id FROM oauth_grants WHERE id=?",
        )
        .bind(grant_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(TokenExchangeError::InvalidGrant)?;
        // Match Core security mutations: lock user before its grants. Client
        // administrative mutations lock the client before its grants.
        let user_status =
            sqlx::query_scalar::<_, String>("SELECT status FROM users WHERE id=? FOR UPDATE")
                .bind(user_id)
                .fetch_optional(&mut *tx)
                .await?
                .ok_or(TokenExchangeError::InvalidGrant)?;
        let (client_id, enabled, metadata, approved, auth_method): (String,bool,Vec<u8>,Option<Vec<u8>>,String) = sqlx::query_as("SELECT client_id,enabled,metadata_hash,approved_metadata_hash,token_auth_method FROM oauth_clients WHERE id=? FOR UPDATE")
            .bind(client_pk).fetch_optional(&mut *tx).await?.ok_or(TokenExchangeError::InvalidGrant)?;
        let (resource, Json(scopes_vec), issued, expires, revoked): (String,Json<Vec<String>>,NaiveDateTime,NaiveDateTime,Option<NaiveDateTime>) = sqlx::query_as("SELECT resource_uri,scopes_json,issued_at,expires_at,revoked_at FROM oauth_grants WHERE id=? AND user_id=? AND client_id=? FOR UPDATE")
            .bind(grant_id).bind(user_id).bind(client_pk).fetch_optional(&mut *tx).await?.ok_or(TokenExchangeError::InvalidGrant)?;
        let (redirect, challenge, code_issued, code_expires, consumed): (String,String,NaiveDateTime,NaiveDateTime,Option<NaiveDateTime>) = sqlx::query_as("SELECT redirect_uri,challenge,issued_at,expires_at,consumed_at FROM oauth_authorization_codes WHERE code_hash=? AND grant_id=? FOR UPDATE")
            .bind(digest.as_slice()).bind(grant_id).fetch_optional(&mut *tx).await?.ok_or(TokenExchangeError::InvalidGrant)?;
        let now: NaiveDateTime = sqlx::query_scalar("SELECT UTC_TIMESTAMP(6)")
            .fetch_one(&mut *tx)
            .await?;
        let ceiling: BTreeSet<String> = sqlx::query_scalar::<_, String>(
            "SELECT scope_name FROM oauth_client_scopes WHERE client_id=?",
        )
        .bind(client_pk)
        .fetch_all(&mut *tx)
        .await?
        .into_iter()
        .collect();
        let scopes: BTreeSet<String> = scopes_vec.iter().cloned().collect();
        if scopes.len() != scopes_vec.len() || auth_method != "none" {
            return Err(TokenExchangeError::InvalidGrant);
        }
        let state = GrantState {
            issued_at: issued.and_utc().timestamp(),
            expires_at: expires.and_utc().timestamp(),
            revoked: revoked.is_some(),
            user_active: user_status == "active",
            client_approved: enabled,
            client_metadata_approved: approved.as_deref() == Some(metadata.as_slice()),
            credentials_current: true,
            granted_scopes: &scopes,
            client_max_scopes: &ceiling,
            current_user_scopes: &scopes,
        };
        let challenge =
            S256Challenge::parse(&challenge).map_err(|_| TokenExchangeError::InvalidGrant)?;
        let record = CodeRecord {
            digest,
            client_id: &client_id,
            redirect_uri: &redirect,
            resource: &resource,
            challenge: &challenge,
            issued_at: code_issued.and_utc().timestamp(),
            expires_at: code_expires.and_utc().timestamp(),
            consumed: consumed.is_some(),
        };
        match validate_code_exchange(request, &record, &state, now.and_utc().timestamp()) {
            Err(ExchangeError::Replay) => {
                let changed = sqlx::query("UPDATE oauth_grants SET revoked_at=?,revocation_reason='code_replay' WHERE id=? AND revoked_at IS NULL")
                    .bind(now).bind(grant_id).execute(&mut *tx).await?.rows_affected();
                if changed != 0 {
                    sqlx::query("INSERT INTO audit_events (actor_id,module_id,action,target_type,target_id,result,created_at) VALUES (?,'core','oauth.code.replay','oauth_grant',?,'denied',?)")
                        .bind(user_id).bind(grant_id.to_string()).bind(now).execute(&mut *tx).await?;
                }
                tx.commit().await?;
                return Err(TokenExchangeError::InvalidGrant);
            }
            Err(_) => return Err(TokenExchangeError::InvalidGrant),
            Ok(()) => {}
        }
        if !self
            .policy
            .authorize(&mut tx, user_id, client_pk, &scopes)
            .await?
        {
            return Err(TokenExchangeError::InvalidGrant);
        }
        let access = Credential::generate().map_err(|_| TokenExchangeError::Unavailable)?;
        let refresh = Credential::generate().map_err(|_| TokenExchangeError::Unavailable)?;
        let access_expiry = std::cmp::min(now + Duration::minutes(15), expires);
        let expires_in = (access_expiry - now).num_seconds();
        if expires_in <= 0 {
            return Err(TokenExchangeError::InvalidGrant);
        }
        let used = sqlx::query("UPDATE oauth_authorization_codes SET consumed_at=? WHERE code_hash=? AND consumed_at IS NULL")
            .bind(now).bind(digest.as_slice()).execute(&mut *tx).await?.rows_affected();
        if used != 1 {
            return Err(TokenExchangeError::InvalidGrant);
        }
        sqlx::query("INSERT INTO oauth_access_tokens (grant_id,token_hash,scopes_json,issued_at,expires_at) VALUES (?,?,?,?,?)")
            .bind(grant_id).bind(access.digest(CredentialKind::AccessToken).as_slice()).bind(Json(scopes_vec)).bind(now).bind(access_expiry).execute(&mut *tx).await?;
        sqlx::query("INSERT INTO oauth_refresh_tokens (grant_id,token_hash,issued_at,expires_at) VALUES (?,?,?,?)")
            .bind(grant_id).bind(refresh.digest(CredentialKind::RefreshToken).as_slice()).bind(now).bind(expires).execute(&mut *tx).await?;
        sqlx::query("INSERT INTO audit_events (actor_id,module_id,action,target_type,target_id,result,created_at) VALUES (?,'core','oauth.code.exchange','oauth_grant',?,'success',?)")
            .bind(user_id).bind(grant_id.to_string()).bind(now).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(IssuedTokens {
            access_token: access,
            refresh_token: refresh,
            expires_in: expires_in as u64,
            scopes,
        })
    }

    /// Atomically consume one refresh token and issue its successor. A
    /// confirmed replay revokes the entire grant and all associated tokens.
    pub async fn rotate_refresh(
        &self,
        request: &TokenParameters,
    ) -> Result<IssuedTokens, TokenExchangeError> {
        let TokenParameters::RefreshToken { refresh_token, .. } = request else {
            return Err(TokenExchangeError::InvalidGrant);
        };
        let digest = refresh_token.digest(CredentialKind::RefreshToken);
        let mut tx = self.pool.begin().await?;
        let grant_id = sqlx::query_scalar::<_, u64>(
            "SELECT grant_id FROM oauth_refresh_tokens WHERE token_hash=?",
        )
        .bind(digest.as_slice())
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(TokenExchangeError::InvalidGrant)?;
        let (user_id, client_pk) = sqlx::query_as::<_, (u64, u64)>(
            "SELECT user_id,client_id FROM oauth_grants WHERE id=?",
        )
        .bind(grant_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(TokenExchangeError::InvalidGrant)?;
        let status =
            sqlx::query_scalar::<_, String>("SELECT status FROM users WHERE id=? FOR UPDATE")
                .bind(user_id)
                .fetch_optional(&mut *tx)
                .await?
                .ok_or(TokenExchangeError::InvalidGrant)?;
        let (client_id, enabled, metadata, approved, method): (String,bool,Vec<u8>,Option<Vec<u8>>,String) = sqlx::query_as("SELECT client_id,enabled,metadata_hash,approved_metadata_hash,token_auth_method FROM oauth_clients WHERE id=? FOR UPDATE")
            .bind(client_pk).fetch_optional(&mut *tx).await?.ok_or(TokenExchangeError::InvalidGrant)?;
        let (resource, Json(scopes_vec), issued, expires, revoked): (String,Json<Vec<String>>,NaiveDateTime,NaiveDateTime,Option<NaiveDateTime>) = sqlx::query_as("SELECT resource_uri,scopes_json,issued_at,expires_at,revoked_at FROM oauth_grants WHERE id=? AND user_id=? AND client_id=? FOR UPDATE")
            .bind(grant_id).bind(user_id).bind(client_pk).fetch_optional(&mut *tx).await?.ok_or(TokenExchangeError::InvalidGrant)?;
        let (token_issued, token_expires, consumed): (NaiveDateTime,NaiveDateTime,Option<NaiveDateTime>) = sqlx::query_as("SELECT issued_at,expires_at,consumed_at FROM oauth_refresh_tokens WHERE token_hash=? AND grant_id=? FOR UPDATE")
            .bind(digest.as_slice()).bind(grant_id).fetch_optional(&mut *tx).await?.ok_or(TokenExchangeError::InvalidGrant)?;
        let now: NaiveDateTime = sqlx::query_scalar("SELECT UTC_TIMESTAMP(6)")
            .fetch_one(&mut *tx)
            .await?;
        let ceiling: BTreeSet<String> = sqlx::query_scalar::<_, String>(
            "SELECT scope_name FROM oauth_client_scopes WHERE client_id=?",
        )
        .bind(client_pk)
        .fetch_all(&mut *tx)
        .await?
        .into_iter()
        .collect();
        let scopes: BTreeSet<String> = scopes_vec.iter().cloned().collect();
        if scopes.len() != scopes_vec.len() || method != "none" {
            return Err(TokenExchangeError::InvalidGrant);
        }
        let state = GrantState {
            issued_at: issued.and_utc().timestamp(),
            expires_at: expires.and_utc().timestamp(),
            revoked: revoked.is_some(),
            user_active: status == "active",
            client_approved: enabled,
            client_metadata_approved: approved.as_deref() == Some(metadata.as_slice()),
            credentials_current: true,
            granted_scopes: &scopes,
            client_max_scopes: &ceiling,
            current_user_scopes: &scopes,
        };
        let record = RefreshRecord {
            digest,
            client_id: &client_id,
            resource: &resource,
            issued_at: token_issued.and_utc().timestamp(),
            expires_at: token_expires.and_utc().timestamp(),
            consumed: consumed.is_some(),
        };
        let effective = match validate_refresh_exchange(
            request,
            &record,
            &state,
            now.and_utc().timestamp(),
        ) {
            Err(ExchangeError::Replay) => {
                let changed = sqlx::query("UPDATE oauth_grants SET revoked_at=?,revocation_reason='refresh_replay' WHERE id=? AND revoked_at IS NULL")
                    .bind(now).bind(grant_id).execute(&mut *tx).await?.rows_affected();
                if changed != 0 {
                    sqlx::query("INSERT INTO audit_events (actor_id,module_id,action,target_type,target_id,result,created_at) VALUES (?,'core','oauth.refresh.replay','oauth_grant',?,'denied',?)")
                        .bind(user_id).bind(grant_id.to_string()).bind(now).execute(&mut *tx).await?;
                }
                tx.commit().await?;
                return Err(TokenExchangeError::InvalidGrant);
            }
            Err(_) => return Err(TokenExchangeError::InvalidGrant),
            Ok(value) => value.clone(),
        };
        if !self
            .policy
            .authorize(&mut tx, user_id, client_pk, &effective)
            .await?
        {
            return Err(TokenExchangeError::InvalidGrant);
        }
        let access = Credential::generate().map_err(|_| TokenExchangeError::Unavailable)?;
        let refresh = Credential::generate().map_err(|_| TokenExchangeError::Unavailable)?;
        let access_expiry = std::cmp::min(now + Duration::minutes(15), expires);
        let expires_in = (access_expiry - now).num_seconds();
        if expires_in <= 0 {
            return Err(TokenExchangeError::InvalidGrant);
        }
        let used = sqlx::query("UPDATE oauth_refresh_tokens SET consumed_at=? WHERE token_hash=? AND consumed_at IS NULL")
            .bind(now).bind(digest.as_slice()).execute(&mut *tx).await?.rows_affected();
        if used != 1 {
            return Err(TokenExchangeError::InvalidGrant);
        }
        let effective_vec: Vec<String> = effective.iter().cloned().collect();
        sqlx::query("INSERT INTO oauth_access_tokens (grant_id,token_hash,scopes_json,issued_at,expires_at) VALUES (?,?,?,?,?)")
            .bind(grant_id).bind(access.digest(CredentialKind::AccessToken).as_slice()).bind(Json(effective_vec)).bind(now).bind(access_expiry).execute(&mut *tx).await?;
        sqlx::query("INSERT INTO oauth_refresh_tokens (grant_id,token_hash,issued_at,expires_at) VALUES (?,?,?,?)")
            .bind(grant_id).bind(refresh.digest(CredentialKind::RefreshToken).as_slice()).bind(now).bind(expires).execute(&mut *tx).await?;
        sqlx::query("INSERT INTO audit_events (actor_id,module_id,action,target_type,target_id,result,created_at) VALUES (?,'core','oauth.refresh.rotate','oauth_grant',?,'success',?)")
            .bind(user_id).bind(grant_id.to_string()).bind(now).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(IssuedTokens {
            access_token: access,
            refresh_token: refresh,
            expires_in: expires_in as u64,
            scopes: effective,
        })
    }
}
