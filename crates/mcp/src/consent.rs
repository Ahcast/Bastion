use std::collections::BTreeSet;

use chrono::{Duration, NaiveDateTime};
use sha2::{Digest, Sha256};
use sqlx::{MySql, MySqlPool, Transaction, types::Json};

use crate::{AuthorizationParameters, Credential, CredentialKind, ExchangePolicy};

pub struct ConsentIdentity {
    pub user_id: u64,
    pub session_id: u64,
}

/// Only the Core may verify a browser session. No default trust is provided.
#[allow(async_fn_in_trait)]
pub trait ConsentPolicy {
    async fn session(
        &self,
        tx: &mut Transaction<'_, MySql>,
        session_token: &str,
    ) -> Result<Option<ConsentIdentity>, sqlx::Error>;
}

pub struct MysqlConsent<S, P> {
    pool: MySqlPool,
    session: S,
    scopes: P,
}

pub struct PendingConsent {
    pub handle: Credential,
    pub client_name: String,
    pub scopes: BTreeSet<String>,
    pub expires_at: NaiveDateTime,
}

/// Only the browser-facing handler may use this result after explicit consent.
pub struct ConsentResult {
    pub code: Credential,
    pub redirect_uri: String,
    pub state: Option<String>,
}

/// Validated callback information for an explicit user denial. No code or
/// grant is issued; the HTTP handler must still protect the response.
pub struct DeniedConsent {
    pub redirect_uri: String,
    pub state: Option<String>,
}

#[derive(Debug)]
pub enum ConsentError {
    InvalidRequest,
    Unauthorized,
    Unavailable,
    Database(sqlx::Error),
}
impl From<sqlx::Error> for ConsentError {
    fn from(value: sqlx::Error) -> Self {
        Self::Database(value)
    }
}

impl<S: ConsentPolicy, P: ExchangePolicy> MysqlConsent<S, P> {
    pub fn new(pool: MySqlPool, session: S, scopes: P) -> Self {
        Self {
            pool,
            session,
            scopes,
        }
    }

    /// Starts a browser consent request without creating a grant. The caller
    /// passes only an already parsed request and a configured resource URI.
    pub async fn begin(
        &self,
        session_token: &str,
        request: &AuthorizationParameters,
        configured_resource: &str,
    ) -> Result<PendingConsent, ConsentError> {
        if configured_resource.is_empty()
            || request.resource != configured_resource
            || crate::validate_redirect_uri(&request.redirect_uri).is_err()
            || request.client_id.len() > 2048
            || request.redirect_uri.len() > 2048
            || request.scopes.is_empty()
            || request.scopes.len() > 64
            || request.scopes.iter().any(|s| s.is_empty() || s.len() > 191)
            || request.state.as_ref().is_some_and(|s| s.len() > 2048)
        {
            return Err(ConsentError::InvalidRequest);
        }
        let mut tx = self.pool.begin().await?;
        let identity = self
            .session
            .session(&mut tx, session_token)
            .await?
            .ok_or(ConsentError::Unauthorized)?;
        let client_hash = Sha256::digest(request.client_id.as_bytes());
        let (client_id, identity_text, client_name, metadata, approved, enabled, method): (u64,String,String,Vec<u8>,Option<Vec<u8>>,bool,String) = sqlx::query_as("SELECT id,client_id,display_name,metadata_hash,approved_metadata_hash,enabled,token_auth_method FROM oauth_clients WHERE client_id_hash=? FOR UPDATE")
            .bind(client_hash.as_slice()).fetch_optional(&mut *tx).await?.ok_or(ConsentError::InvalidRequest)?;
        if identity_text != request.client_id
            || !enabled
            || method != "none"
            || approved.as_deref() != Some(metadata.as_slice())
        {
            return Err(ConsentError::InvalidRequest);
        }
        let redirect_hash = Sha256::digest(request.redirect_uri.as_bytes());
        let stored_redirect = sqlx::query_scalar::<_, String>(
            "SELECT uri FROM oauth_client_redirects WHERE client_id=? AND uri_hash=?",
        )
        .bind(client_id)
        .bind(redirect_hash.as_slice())
        .fetch_optional(&mut *tx)
        .await?;
        if stored_redirect.as_deref() != Some(request.redirect_uri.as_str()) {
            return Err(ConsentError::InvalidRequest);
        }
        let ceiling: BTreeSet<String> = sqlx::query_scalar::<_, String>(
            "SELECT scope_name FROM oauth_client_scopes WHERE client_id=?",
        )
        .bind(client_id)
        .fetch_all(&mut *tx)
        .await?
        .into_iter()
        .collect();
        if !request.scopes.is_subset(&ceiling)
            || !self
                .scopes
                .authorize(&mut tx, identity.user_id, client_id, &request.scopes)
                .await?
        {
            return Err(ConsentError::InvalidRequest);
        }
        let handle = Credential::generate().map_err(|_| ConsentError::Unavailable)?;
        let now: NaiveDateTime = sqlx::query_scalar("SELECT UTC_TIMESTAMP(6)")
            .fetch_one(&mut *tx)
            .await?;
        let expires_at = now + Duration::minutes(10);
        let scopes: Vec<String> = request.scopes.iter().cloned().collect();
        sqlx::query("INSERT INTO oauth_consent_requests (request_hash,user_id,session_id,client_id,redirect_uri,resource_uri,scopes_json,challenge,state_value,issued_at,expires_at,approved_metadata_hash) VALUES (?,?,?,?,?,?,?,?,?,?,?,?)")
            .bind(handle.digest(CredentialKind::ConsentRequest).as_slice()).bind(identity.user_id).bind(identity.session_id).bind(client_id).bind(&request.redirect_uri).bind(&request.resource).bind(Json(scopes)).bind(request.challenge.encode()).bind(&request.state).bind(now).bind(expires_at).bind(metadata).execute(&mut *tx).await?;
        sqlx::query("INSERT INTO audit_events (actor_id,module_id,action,target_type,target_id,result,created_at) VALUES (?,'core','oauth.consent.begin','oauth_client',?,'success',?)")
            .bind(identity.user_id).bind(client_id.to_string()).bind(now).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(PendingConsent {
            handle,
            client_name,
            scopes: request.scopes.clone(),
            expires_at,
        })
    }

    /// Called only after the authenticated user explicitly approves in the
    /// browser; enforce CSRF and exact Origin at the HTTP boundary.
    pub async fn approve(
        &self,
        session_token: &str,
        handle: &Credential,
        configured_resource: &str,
    ) -> Result<ConsentResult, ConsentError> {
        if configured_resource.is_empty() {
            return Err(ConsentError::InvalidRequest);
        }
        let mut tx = self.pool.begin().await?;
        let identity = self
            .session
            .session(&mut tx, session_token)
            .await?
            .ok_or(ConsentError::Unauthorized)?;
        let hash = handle.digest(CredentialKind::ConsentRequest);
        let client_id = sqlx::query_scalar::<_,u64>("SELECT client_id FROM oauth_consent_requests WHERE request_hash=? AND user_id=? AND session_id=?")
            .bind(hash.as_slice()).bind(identity.user_id).bind(identity.session_id).fetch_optional(&mut *tx).await?.ok_or(ConsentError::InvalidRequest)?;
        let (metadata, approved, enabled, method): (Vec<u8>,Option<Vec<u8>>,bool,String) = sqlx::query_as("SELECT metadata_hash,approved_metadata_hash,enabled,token_auth_method FROM oauth_clients WHERE id=? FOR UPDATE")
            .bind(client_id).fetch_optional(&mut *tx).await?.ok_or(ConsentError::InvalidRequest)?;
        let (redirect,resource,Json(scope_vec),challenge,state,expires,consumed,snapshot): (String,String,Json<Vec<String>>,String,Option<String>,NaiveDateTime,Option<NaiveDateTime>,Option<Vec<u8>>) = sqlx::query_as("SELECT redirect_uri,resource_uri,scopes_json,challenge,state_value,expires_at,consumed_at,approved_metadata_hash FROM oauth_consent_requests WHERE request_hash=? AND user_id=? AND session_id=? FOR UPDATE")
            .bind(hash.as_slice()).bind(identity.user_id).bind(identity.session_id).fetch_optional(&mut *tx).await?.ok_or(ConsentError::InvalidRequest)?;
        let now: NaiveDateTime = sqlx::query_scalar("SELECT UTC_TIMESTAMP(6)")
            .fetch_one(&mut *tx)
            .await?;
        if !enabled
            || method != "none"
            || approved.as_deref() != Some(metadata.as_slice())
            || snapshot.as_deref() != Some(metadata.as_slice())
            || consumed.is_some()
            || expires <= now
            || resource != configured_resource
            || crate::validate_redirect_uri(&redirect).is_err()
        {
            return Err(ConsentError::InvalidRequest);
        }
        let redirect_hash = Sha256::digest(redirect.as_bytes());
        let current_redirect = sqlx::query_scalar::<_, String>(
            "SELECT uri FROM oauth_client_redirects WHERE client_id=? AND uri_hash=?",
        )
        .bind(client_id)
        .bind(redirect_hash.as_slice())
        .fetch_optional(&mut *tx)
        .await?;
        if current_redirect.as_deref() != Some(redirect.as_str()) {
            return Err(ConsentError::InvalidRequest);
        }
        let scopes: BTreeSet<String> = scope_vec.iter().cloned().collect();
        let ceiling: BTreeSet<String> = sqlx::query_scalar::<_, String>(
            "SELECT scope_name FROM oauth_client_scopes WHERE client_id=?",
        )
        .bind(client_id)
        .fetch_all(&mut *tx)
        .await?
        .into_iter()
        .collect();
        if scopes.is_empty()
            || scopes.len() != scope_vec.len()
            || !scopes.is_subset(&ceiling)
            || !self
                .scopes
                .authorize(&mut tx, identity.user_id, client_id, &scopes)
                .await?
        {
            return Err(ConsentError::InvalidRequest);
        }
        let code = Credential::generate().map_err(|_| ConsentError::Unavailable)?;
        let grant_expires = now + Duration::days(15);
        let result = sqlx::query("INSERT INTO oauth_grants (user_id,client_id,resource_uri,scopes_json,issued_at,expires_at) VALUES (?,?,?,?,?,?)")
            .bind(identity.user_id).bind(client_id).bind(&resource).bind(Json(scope_vec)).bind(now).bind(grant_expires).execute(&mut *tx).await?;
        let grant_id = result.last_insert_id();
        sqlx::query("INSERT INTO oauth_authorization_codes (grant_id,code_hash,redirect_uri,challenge,issued_at,expires_at) VALUES (?,?,?,?,?,?)")
            .bind(grant_id).bind(code.digest(CredentialKind::AuthorizationCode).as_slice()).bind(&redirect).bind(challenge).bind(now).bind(now+Duration::minutes(5)).execute(&mut *tx).await?;
        let used=sqlx::query("UPDATE oauth_consent_requests SET consumed_at=? WHERE request_hash=? AND consumed_at IS NULL")
            .bind(now).bind(hash.as_slice()).execute(&mut *tx).await?.rows_affected();
        if used != 1 {
            return Err(ConsentError::InvalidRequest);
        }
        sqlx::query("INSERT INTO audit_events (actor_id,module_id,action,target_type,target_id,result,created_at) VALUES (?,'core','oauth.consent.approve','oauth_grant',?,'success',?)")
            .bind(identity.user_id).bind(grant_id.to_string()).bind(now).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(ConsentResult {
            code,
            redirect_uri: redirect,
            state,
        })
    }

    /// Reject a pending request from the same authenticated browser session.
    /// It consumes the request without creating a grant or credentials.
    pub async fn deny(
        &self,
        session_token: &str,
        handle: &Credential,
        configured_resource: &str,
    ) -> Result<DeniedConsent, ConsentError> {
        if configured_resource.is_empty() {
            return Err(ConsentError::InvalidRequest);
        }
        let mut tx = self.pool.begin().await?;
        let identity = self
            .session
            .session(&mut tx, session_token)
            .await?
            .ok_or(ConsentError::Unauthorized)?;
        let hash = handle.digest(CredentialKind::ConsentRequest);
        let client_id=sqlx::query_scalar::<_,u64>("SELECT client_id FROM oauth_consent_requests WHERE request_hash=? AND user_id=? AND session_id=?")
            .bind(hash.as_slice()).bind(identity.user_id).bind(identity.session_id).fetch_optional(&mut *tx).await?.ok_or(ConsentError::InvalidRequest)?;
        let client=sqlx::query_as::<_,(Vec<u8>,Option<Vec<u8>>,bool,String)>("SELECT metadata_hash,approved_metadata_hash,enabled,token_auth_method FROM oauth_clients WHERE id=? FOR UPDATE")
            .bind(client_id).fetch_optional(&mut *tx).await?.ok_or(ConsentError::InvalidRequest)?;
        let row=sqlx::query_as::<_,(String,String,Option<String>,NaiveDateTime,Option<NaiveDateTime>,Option<Vec<u8>>)>("SELECT redirect_uri,resource_uri,state_value,expires_at,consumed_at,approved_metadata_hash FROM oauth_consent_requests WHERE request_hash=? AND user_id=? AND session_id=? FOR UPDATE")
            .bind(hash.as_slice()).bind(identity.user_id).bind(identity.session_id).fetch_optional(&mut *tx).await?.ok_or(ConsentError::InvalidRequest)?;
        let now: NaiveDateTime = sqlx::query_scalar("SELECT UTC_TIMESTAMP(6)")
            .fetch_one(&mut *tx)
            .await?;
        if !client.2
            || client.3 != "none"
            || client.1.as_deref() != Some(client.0.as_slice())
            || row.5.as_deref() != Some(client.0.as_slice())
            || row.1 != configured_resource
            || row.3 <= now
            || row.4.is_some()
            || crate::validate_redirect_uri(&row.0).is_err()
        {
            return Err(ConsentError::InvalidRequest);
        }
        let redirect_hash = Sha256::digest(row.0.as_bytes());
        let current_redirect = sqlx::query_scalar::<_, String>(
            "SELECT uri FROM oauth_client_redirects WHERE client_id=? AND uri_hash=?",
        )
        .bind(client_id)
        .bind(redirect_hash.as_slice())
        .fetch_optional(&mut *tx)
        .await?;
        if current_redirect.as_deref() != Some(row.0.as_str()) {
            return Err(ConsentError::InvalidRequest);
        }
        let used=sqlx::query("UPDATE oauth_consent_requests SET consumed_at=? WHERE request_hash=? AND consumed_at IS NULL")
            .bind(now).bind(hash.as_slice()).execute(&mut *tx).await?.rows_affected();
        if used != 1 {
            return Err(ConsentError::InvalidRequest);
        }
        sqlx::query("INSERT INTO audit_events (actor_id,module_id,action,target_type,target_id,result,created_at) VALUES (?,'core','oauth.consent.deny','oauth_client',?,'success',?)")
            .bind(identity.user_id).bind(client_id.to_string()).bind(now).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(DeniedConsent {
            redirect_uri: row.0,
            state: row.2,
        })
    }
}
