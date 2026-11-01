use std::collections::BTreeSet;

use sha2::{Digest, Sha256};
use sqlx::{MySql, MySqlPool, QueryBuilder, Transaction, types::Json};

use crate::{ClientMetadata, ConsentPolicy};

#[derive(Debug)]
pub enum ClientRequestError {
    Forbidden,
    Invalid,
    Conflict,
    AlreadyApproved,
    NotFound,
    Database(sqlx::Error),
}
impl From<sqlx::Error> for ClientRequestError {
    fn from(error: sqlx::Error) -> Self {
        Self::Database(error)
    }
}

pub enum ReviewDecision<'a> {
    Approve {
        metadata: &'a ClientMetadata,
        scopes: &'a BTreeSet<String>,
    },
    Reject,
}

pub struct ClientRequestStore {
    pool: MySqlPool,
}

#[derive(serde::Serialize)]
pub struct ClientRequestSummary {
    pub id: u64,
    pub client_name: String,
    pub client_id: String,
    pub scopes: Vec<String>,
    pub status: String,
    pub created_at: chrono::NaiveDateTime,
}

#[derive(serde::Serialize)]
pub struct ClientRequestPage {
    pub items: Vec<ClientRequestSummary>,
    pub next_before: Option<u64>,
}

fn valid_scopes(scopes: &BTreeSet<String>, registered: &BTreeSet<String>) -> bool {
    !scopes.is_empty()
        && scopes.len() <= 64
        && scopes.is_subset(registered)
        && scopes.iter().all(|scope| {
            !scope.is_empty()
                && scope.len() <= 191
                && scope
                    .bytes()
                    .all(|b| b == 0x21 || (0x23..=0x5b).contains(&b) || (0x5d..=0x7e).contains(&b))
        })
}

impl ClientRequestStore {
    pub fn new(pool: MySqlPool) -> Self {
        Self { pool }
    }

    /// Check a browser session before expensive work such as metadata fetch.
    /// A later mutation must independently revalidate it in its transaction.
    pub async fn authorize_session<S: ConsentPolicy>(
        &self,
        policy: &S,
        session: &str,
        admin: bool,
    ) -> Result<(), ClientRequestError> {
        let mut tx = self.pool.begin().await?;
        let identity = policy
            .session(&mut tx, session)
            .await?
            .ok_or(ClientRequestError::Forbidden)?;
        if admin && sqlx::query_scalar::<_,u64>("SELECT u.id FROM users u JOIN roles r ON r.id=u.role_id WHERE u.id=? AND u.status='active' AND r.system_key='system_administrator' FOR UPDATE").bind(identity.user_id).fetch_optional(&mut *tx).await?.is_none(){return Err(ClientRequestError::Forbidden);}
        tx.commit().await?;
        Ok(())
    }

    /// List only the authenticated user's requests, with a bounded cursor.
    pub async fn list_own(
        &self,
        user: u64,
        before: Option<u64>,
    ) -> Result<ClientRequestPage, ClientRequestError> {
        self.list(user, before, false).await
    }

    /// Review queue is restricted to active system administrators.
    pub async fn list_pending(
        &self,
        actor: u64,
        before: Option<u64>,
    ) -> Result<ClientRequestPage, ClientRequestError> {
        self.list(actor, before, true).await
    }

    async fn list(
        &self,
        actor: u64,
        before: Option<u64>,
        admin: bool,
    ) -> Result<ClientRequestPage, ClientRequestError> {
        let mut tx = self.pool.begin().await?;
        let page = Self::list_in_transaction(&mut tx, actor, before, admin).await?;
        tx.commit().await?;
        Ok(page)
    }

    /// List with session validity locked for the duration of the query.
    pub async fn list_for_session<S: ConsentPolicy>(
        &self,
        policy: &S,
        session: &str,
        before: Option<u64>,
        admin: bool,
    ) -> Result<ClientRequestPage, ClientRequestError> {
        let mut tx = self.pool.begin().await?;
        let identity = policy
            .session(&mut tx, session)
            .await?
            .ok_or(ClientRequestError::Forbidden)?;
        let page = Self::list_in_transaction(&mut tx, identity.user_id, before, admin).await?;
        tx.commit().await?;
        Ok(page)
    }

    async fn list_in_transaction(
        tx: &mut Transaction<'_, MySql>,
        actor: u64,
        before: Option<u64>,
        admin: bool,
    ) -> Result<ClientRequestPage, ClientRequestError> {
        if before == Some(0) {
            return Err(ClientRequestError::Invalid);
        }
        let authorized = if admin {
            sqlx::query_scalar::<_,u64>("SELECT u.id FROM users u JOIN roles r ON r.id=u.role_id WHERE u.id=? AND u.status='active' AND r.system_key='system_administrator' FOR UPDATE").bind(actor).fetch_optional(&mut **tx).await?
        } else {
            sqlx::query_scalar::<_, u64>(
                "SELECT id FROM users WHERE id=? AND status='active' FOR UPDATE",
            )
            .bind(actor)
            .fetch_optional(&mut **tx)
            .await?
        };
        if authorized.is_none() {
            return Err(ClientRequestError::Forbidden);
        }
        let mut query = QueryBuilder::<MySql>::new(
            "SELECT r.id,c.display_name,c.client_id,r.requested_scopes,r.status,r.created_at FROM oauth_client_requests r JOIN oauth_clients c ON c.id=r.client_id WHERE ",
        );
        if admin {
            query.push("r.status='pending'");
        } else {
            query.push("r.user_id=").push_bind(actor);
        }
        if let Some(before) = before {
            query.push(" AND r.id<").push_bind(before);
        }
        query.push(" ORDER BY r.id DESC LIMIT 51");
        let rows: Vec<(
            u64,
            String,
            String,
            Json<Vec<String>>,
            String,
            chrono::NaiveDateTime,
        )> = query.build_query_as().fetch_all(&mut **tx).await?;
        let has_more = rows.len() > 50;
        let items: Vec<_> = rows
            .into_iter()
            .take(50)
            .map(
                |(id, client_name, client_id, Json(scopes), status, created_at)| {
                    ClientRequestSummary {
                        id,
                        client_name,
                        client_id,
                        scopes,
                        status,
                        created_at,
                    }
                },
            )
            .collect();
        let next_before = if has_more {
            items.last().map(|item| item.id)
        } else {
            None
        };
        Ok(ClientRequestPage { items, next_before })
    }

    /// An authenticated user requests review of a validated document; no
    /// grant, token or approval is created. The caller must enforce CSRF,
    /// session identity, fetch protections, limits and trusted scope registry.
    pub async fn submit(
        &self,
        user: u64,
        metadata: &ClientMetadata,
        scopes: &BTreeSet<String>,
        registered: &BTreeSet<String>,
    ) -> Result<u64, ClientRequestError> {
        let mut tx = self.pool.begin().await?;
        let request =
            Self::submit_in_transaction(&mut tx, user, metadata, scopes, registered).await?;
        tx.commit().await?;
        Ok(request)
    }

    /// Binds request creation to a currently valid browser session in the same
    /// transaction. The HTTP boundary must still enforce Origin and CSRF.
    pub async fn submit_for_session<S: ConsentPolicy>(
        &self,
        policy: &S,
        session: &str,
        metadata: &ClientMetadata,
        scopes: &BTreeSet<String>,
        registered: &BTreeSet<String>,
    ) -> Result<u64, ClientRequestError> {
        let mut tx = self.pool.begin().await?;
        let identity = policy
            .session(&mut tx, session)
            .await?
            .ok_or(ClientRequestError::Forbidden)?;
        let request =
            Self::submit_in_transaction(&mut tx, identity.user_id, metadata, scopes, registered)
                .await?;
        tx.commit().await?;
        Ok(request)
    }

    async fn submit_in_transaction(
        tx: &mut Transaction<'_, MySql>,
        user: u64,
        metadata: &ClientMetadata,
        scopes: &BTreeSet<String>,
        registered: &BTreeSet<String>,
    ) -> Result<u64, ClientRequestError> {
        if !valid_scopes(scopes, registered) {
            return Err(ClientRequestError::Invalid);
        }
        if sqlx::query_scalar::<_, u64>(
            "SELECT id FROM users WHERE id=? AND status='active' FOR UPDATE",
        )
        .bind(user)
        .fetch_optional(&mut **tx)
        .await?
        .is_none()
        {
            return Err(ClientRequestError::Forbidden);
        }
        let hash: [u8; 32] = Sha256::digest(metadata.client_id().as_bytes()).into();
        let existing=sqlx::query_as::<_,(u64,String,Vec<u8>,Option<Vec<u8>>,bool)>("SELECT id,client_id,metadata_hash,approved_metadata_hash,enabled FROM oauth_clients WHERE client_id_hash=? FOR UPDATE").bind(hash.as_slice()).fetch_optional(&mut **tx).await?;
        let client = if let Some((id, identity, current, approved, enabled)) = existing {
            if identity != metadata.client_id() || current.as_slice() != metadata.document_hash() {
                return Err(ClientRequestError::Conflict);
            }
            if enabled && approved.as_deref() == Some(current.as_slice()) {
                let ceiling: BTreeSet<String> = sqlx::query_scalar::<_, String>(
                    "SELECT scope_name FROM oauth_client_scopes WHERE client_id=?",
                )
                .bind(id)
                .fetch_all(&mut **tx)
                .await?
                .into_iter()
                .collect();
                if scopes.is_subset(&ceiling) {
                    return Err(ClientRequestError::AlreadyApproved);
                }
            }
            id
        } else {
            let id=sqlx::query("INSERT INTO oauth_clients (client_id,client_id_hash,display_name,metadata_hash,approved_metadata_hash,enabled,created_at,updated_at,token_auth_method) VALUES (?,?,?,?,NULL,0,UTC_TIMESTAMP(6),UTC_TIMESTAMP(6),'none')")
                .bind(metadata.client_id()).bind(hash.as_slice()).bind(metadata.name().unwrap_or("Cliente sin nombre")).bind(metadata.document_hash().as_slice()).execute(&mut **tx).await?.last_insert_id();
            for uri in metadata.redirect_uris() {
                let uri_hash: [u8; 32] = Sha256::digest(uri.as_bytes()).into();
                sqlx::query(
                    "INSERT INTO oauth_client_redirects (client_id,uri,uri_hash) VALUES (?,?,?)",
                )
                .bind(id)
                .bind(uri)
                .bind(uri_hash.as_slice())
                .execute(&mut **tx)
                .await?;
            }
            id
        };
        let prior=sqlx::query_as::<_,(u64,Json<Vec<String>>,Option<Vec<u8>>)>("SELECT id,requested_scopes,metadata_hash FROM oauth_client_requests WHERE user_id=? AND client_id=? AND status='pending' FOR UPDATE")
            .bind(user).bind(client).fetch_optional(&mut **tx).await?;
        if let Some((id, Json(previous), snapshot)) = prior {
            let count = previous.len();
            let previous: BTreeSet<_> = previous.into_iter().collect();
            if count != previous.len()
                || previous != *scopes
                || snapshot.as_deref() != Some(metadata.document_hash().as_slice())
            {
                return Err(ClientRequestError::Conflict);
            }
            return Ok(id);
        }
        let values: Vec<String> = scopes.iter().cloned().collect();
        let request=sqlx::query("INSERT INTO oauth_client_requests (user_id,client_id,requested_scopes,status,created_at,updated_at,metadata_hash) VALUES (?,?,?,'pending',UTC_TIMESTAMP(6),UTC_TIMESTAMP(6),?)")
            .bind(user).bind(client).bind(Json(values)).bind(metadata.document_hash().as_slice()).execute(&mut **tx).await?.last_insert_id();
        sqlx::query("INSERT INTO admin_notifications (recipient_user_id,kind,subject,payload_json,delivery_status,created_at) SELECT u.id,'oauth_client_request','Solicitud de cliente MCP',JSON_OBJECT('request_id',?),'pending',UTC_TIMESTAMP(6) FROM users u JOIN roles r ON r.id=u.role_id WHERE u.status='active' AND r.system_key='system_administrator'").bind(request).execute(&mut **tx).await?;
        sqlx::query("INSERT INTO audit_events (actor_id,module_id,action,target_type,target_id,result,created_at) VALUES (?,'core','oauth.client.request','oauth_client',?,'success',UTC_TIMESTAMP(6))").bind(user).bind(client.to_string()).execute(&mut **tx).await?;
        Ok(request)
    }

    /// Only an active system administrator may review a pending request.
    /// Approval requires freshly fetched matching metadata and never grants
    /// access to user data. Email delivery and UI are separate integrations.
    pub async fn review(
        &self,
        actor: u64,
        request: u64,
        decision: ReviewDecision<'_>,
        registered: &BTreeSet<String>,
    ) -> Result<(), ClientRequestError> {
        let mut tx = self.pool.begin().await?;
        Self::review_in_transaction(&mut tx, actor, request, decision, registered).await?;
        tx.commit().await?;
        Ok(())
    }

    /// Review using the administrator's locked browser session, preventing
    /// a revoked session from authorizing the decision in a later transaction.
    pub async fn review_for_session<S: ConsentPolicy>(
        &self,
        policy: &S,
        session: &str,
        request: u64,
        decision: ReviewDecision<'_>,
        registered: &BTreeSet<String>,
    ) -> Result<(), ClientRequestError> {
        let mut tx = self.pool.begin().await?;
        let identity = policy
            .session(&mut tx, session)
            .await?
            .ok_or(ClientRequestError::Forbidden)?;
        Self::review_in_transaction(&mut tx, identity.user_id, request, decision, registered)
            .await?;
        tx.commit().await?;
        Ok(())
    }

    async fn review_in_transaction(
        tx: &mut Transaction<'_, MySql>,
        actor: u64,
        request: u64,
        decision: ReviewDecision<'_>,
        registered: &BTreeSet<String>,
    ) -> Result<(), ClientRequestError> {
        if sqlx::query_scalar::<_,u64>("SELECT u.id FROM users u JOIN roles r ON r.id=u.role_id WHERE u.id=? AND u.status='active' AND r.system_key='system_administrator' FOR UPDATE").bind(actor).fetch_optional(&mut **tx).await?.is_none(){return Err(ClientRequestError::Forbidden);}
        let client =
            sqlx::query_scalar::<_, u64>("SELECT client_id FROM oauth_client_requests WHERE id=?")
                .bind(request)
                .fetch_optional(&mut **tx)
                .await?
                .ok_or(ClientRequestError::NotFound)?;
        let (identity,current,approved,enabled,method):(String,Vec<u8>,Option<Vec<u8>>,bool,String)=sqlx::query_as("SELECT client_id,metadata_hash,approved_metadata_hash,enabled,token_auth_method FROM oauth_clients WHERE id=? FOR UPDATE").bind(client).fetch_optional(&mut **tx).await?.ok_or(ClientRequestError::NotFound)?;
        let (requester,Json(requested),status,snapshot):(u64,Json<Vec<String>>,String,Option<Vec<u8>>)=sqlx::query_as("SELECT user_id,requested_scopes,status,metadata_hash FROM oauth_client_requests WHERE id=? AND client_id=? FOR UPDATE").bind(request).bind(client).fetch_optional(&mut **tx).await?.ok_or(ClientRequestError::NotFound)?;
        if status != "pending" {
            return Err(ClientRequestError::Conflict);
        }
        let requested: BTreeSet<String> = requested.into_iter().collect();
        let (status, action) = match decision {
            ReviewDecision::Approve { metadata, scopes } => {
                if method != "none"
                    || snapshot.as_deref() != Some(current.as_slice())
                    || metadata.client_id() != identity
                    || metadata.document_hash().as_slice() != current.as_slice()
                    || !valid_scopes(scopes, registered)
                    || !scopes.is_subset(&requested)
                {
                    return Err(ClientRequestError::Invalid);
                }
                let redirects: Vec<String> = sqlx::query_scalar(
                    "SELECT uri FROM oauth_client_redirects WHERE client_id=? FOR UPDATE",
                )
                .bind(client)
                .fetch_all(&mut **tx)
                .await?;
                let count = redirects.len();
                let redirects: BTreeSet<_> = redirects.into_iter().collect();
                if count != redirects.len() || &redirects != metadata.redirect_uris() {
                    return Err(ClientRequestError::Conflict);
                }
                for scope in scopes {
                    sqlx::query("INSERT INTO oauth_client_scopes (client_id,scope_name) VALUES (?,?) ON DUPLICATE KEY UPDATE scope_name=VALUES(scope_name)").bind(client).bind(scope).execute(&mut **tx).await?;
                }
                if !enabled || approved.as_deref() != Some(current.as_slice()) {
                    sqlx::query("UPDATE oauth_grants SET revoked_at=UTC_TIMESTAMP(6),revocation_reason='client_reapproval' WHERE client_id=? AND revoked_at IS NULL").bind(client).execute(&mut **tx).await?;
                }
                sqlx::query("UPDATE oauth_clients SET enabled=1,approved_metadata_hash=metadata_hash,approved_by=?,approved_at=UTC_TIMESTAMP(6),updated_at=UTC_TIMESTAMP(6) WHERE id=?").bind(actor).bind(client).execute(&mut **tx).await?;
                ("approved", "oauth.client.request.approve")
            }
            ReviewDecision::Reject => ("rejected", "oauth.client.request.reject"),
        };
        sqlx::query("UPDATE oauth_client_requests SET status=?,reviewed_by=?,reviewed_at=UTC_TIMESTAMP(6),updated_at=UTC_TIMESTAMP(6) WHERE id=? AND status='pending'").bind(status).bind(actor).bind(request).execute(&mut **tx).await?;
        sqlx::query("INSERT INTO admin_notifications (recipient_user_id,kind,subject,payload_json,delivery_status,created_at) VALUES (?,'oauth_client_review','Resultado de solicitud MCP',JSON_OBJECT('request_id',?),'pending',UTC_TIMESTAMP(6))").bind(requester).bind(request).execute(&mut **tx).await?;
        sqlx::query("INSERT INTO audit_events (actor_id,module_id,action,target_type,target_id,result,created_at) VALUES (?,'core',?,'oauth_client',?,'success',UTC_TIMESTAMP(6))").bind(actor).bind(action).bind(client.to_string()).execute(&mut **tx).await?;
        Ok(())
    }
}
