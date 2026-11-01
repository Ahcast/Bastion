use std::collections::BTreeSet;

use sha2::{Digest, Sha256};
use sqlx::MySqlPool;

use crate::ClientMetadata;

pub struct ApprovedClientStore {
    pool: MySqlPool,
}

#[derive(Debug)]
pub enum ClientApprovalError {
    Forbidden,
    InvalidScopes,
    AlreadyExists,
    Database(sqlx::Error),
}

impl From<sqlx::Error> for ClientApprovalError {
    fn from(value: sqlx::Error) -> Self {
        Self::Database(value)
    }
}

impl ApprovedClientStore {
    pub fn new(pool: MySqlPool) -> Self {
        Self { pool }
    }

    /// Approve a previously unregistered public client and its exact redirect
    /// URIs. The caller must pass a document validated from an SSRF-protected
    /// fetch and a server-owned scope registry, never client-provided claims.
    /// The HTTP adapter must authenticate the actor and enforce CSRF/origin.
    pub async fn approve_new_client(
        &self,
        actor: u64,
        metadata: &ClientMetadata,
        max_scopes: &BTreeSet<String>,
        registered_scopes: &BTreeSet<String>,
    ) -> Result<u64, ClientApprovalError> {
        if max_scopes.is_empty()
            || max_scopes.len() > 64
            || !max_scopes.is_subset(registered_scopes)
            || max_scopes.iter().any(|scope| {
                scope.is_empty()
                    || scope.len() > 191
                    || !scope.bytes().all(|b| {
                        b == 0x21 || (0x23..=0x5b).contains(&b) || (0x5d..=0x7e).contains(&b)
                    })
            })
        {
            return Err(ClientApprovalError::InvalidScopes);
        }
        let mut tx = self.pool.begin().await?;
        let admin = sqlx::query_scalar::<_,u64>("SELECT u.id FROM users u JOIN roles r ON r.id=u.role_id WHERE u.id=? AND u.status='active' AND r.system_key='system_administrator' FOR UPDATE")
            .bind(actor).fetch_optional(&mut *tx).await?;
        if admin.is_none() {
            return Err(ClientApprovalError::Forbidden);
        }
        let id_hash: [u8; 32] = Sha256::digest(metadata.client_id().as_bytes()).into();
        let existing = sqlx::query_scalar::<_, u64>(
            "SELECT id FROM oauth_clients WHERE client_id_hash=? FOR UPDATE",
        )
        .bind(id_hash.as_slice())
        .fetch_optional(&mut *tx)
        .await?;
        if existing.is_some() {
            return Err(ClientApprovalError::AlreadyExists);
        }
        let name = metadata.name().unwrap_or("Cliente sin nombre");
        let result = sqlx::query("INSERT INTO oauth_clients (client_id,client_id_hash,display_name,metadata_hash,approved_metadata_hash,enabled,approved_by,approved_at,created_at,updated_at,token_auth_method) VALUES (?,?,?,?,?,1,?,UTC_TIMESTAMP(6),UTC_TIMESTAMP(6),UTC_TIMESTAMP(6),'none')")
            .bind(metadata.client_id()).bind(id_hash.as_slice()).bind(name).bind(metadata.document_hash().as_slice()).bind(metadata.document_hash().as_slice()).bind(actor).execute(&mut *tx).await?;
        let id = result.last_insert_id();
        for uri in metadata.redirect_uris() {
            let hash: [u8; 32] = Sha256::digest(uri.as_bytes()).into();
            sqlx::query(
                "INSERT INTO oauth_client_redirects (client_id,uri,uri_hash) VALUES (?,?,?)",
            )
            .bind(id)
            .bind(uri)
            .bind(hash.as_slice())
            .execute(&mut *tx)
            .await?;
        }
        for scope in max_scopes {
            sqlx::query("INSERT INTO oauth_client_scopes (client_id,scope_name) VALUES (?,?)")
                .bind(id)
                .bind(scope)
                .execute(&mut *tx)
                .await?;
        }
        sqlx::query("INSERT INTO audit_events (actor_id,module_id,action,target_type,target_id,result,created_at) VALUES (?,'core','oauth.client.approve','oauth_client',?,'success',UTC_TIMESTAMP(6))")
            .bind(actor).bind(id.to_string()).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(id)
    }
}
