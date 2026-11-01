use std::{future::Future, pin::Pin, sync::Arc};

use bastion_mcp::ClientRequestStore;
use sha2::{Digest, Sha256};
use sqlx::MySqlPool;
use tokio::sync::Semaphore;

use crate::{ClientMetadataSource, MetadataFetchError};

#[derive(Debug)]
pub enum MetadataIntegrityError {
    Invalid,
    Unavailable,
}

/// Trusted server-side guard; implementations must not accept metadata or
/// a client identity supplied by the MCP tool caller as authoritative.
pub trait MetadataIntegrity: Send + Sync {
    fn check_client_id<'a>(
        &'a self,
        client_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<(), MetadataIntegrityError>> + Send + 'a>>;
    fn check_client_pk(
        &self,
        client_pk: u64,
    ) -> Pin<Box<dyn Future<Output = Result<(), MetadataIntegrityError>> + Send + '_>>;
}

pub struct DraftMetadataIntegrity<F> {
    pool: MySqlPool,
    fetcher: F,
    slots: Arc<Semaphore>,
}
impl<F: ClientMetadataSource> DraftMetadataIntegrity<F> {
    pub fn new(
        pool: MySqlPool,
        fetcher: F,
        max_concurrent: usize,
    ) -> Result<Self, MetadataIntegrityError> {
        if !(1..=16).contains(&max_concurrent) {
            return Err(MetadataIntegrityError::Invalid);
        }
        Ok(Self {
            pool,
            fetcher,
            slots: Arc::new(Semaphore::new(max_concurrent)),
        })
    }
    async fn check(&self, client_id: &str) -> Result<(), MetadataIntegrityError> {
        if client_id.len() > 2048 || client_id.is_empty() {
            return Err(MetadataIntegrityError::Invalid);
        }
        let digest: [u8; 32] = Sha256::digest(client_id.as_bytes()).into();
        let row=sqlx::query_as::<_,(String,Vec<u8>,Option<Vec<u8>>,bool,String)>("SELECT client_id,metadata_hash,approved_metadata_hash,enabled,token_auth_method FROM oauth_clients WHERE client_id_hash=?").bind(digest.as_slice()).fetch_optional(&self.pool).await.map_err(|_|MetadataIntegrityError::Unavailable)?;
        let Some((identity, current, approved, enabled, method)) = row else {
            return Err(MetadataIntegrityError::Invalid);
        };
        if identity != client_id
            || !enabled
            || method != "none"
            || approved.as_deref() != Some(current.as_slice())
        {
            return Err(MetadataIntegrityError::Invalid);
        }
        let _slot = self
            .slots
            .clone()
            .try_acquire_owned()
            .map_err(|_| MetadataIntegrityError::Unavailable)?;
        let metadata = self
            .fetcher
            .fetch(client_id)
            .await
            .map_err(|error| match error {
                MetadataFetchError::Unavailable => MetadataIntegrityError::Unavailable,
                _ => MetadataIntegrityError::Invalid,
            })?;
        if metadata.client_id() != client_id {
            return Err(MetadataIntegrityError::Invalid);
        }
        match ClientRequestStore::new(self.pool.clone())
            .observe_approved_metadata(&metadata)
            .await
        {
            Ok(true) => Ok(()),
            Ok(false) => Err(MetadataIntegrityError::Invalid),
            Err(_) => Err(MetadataIntegrityError::Unavailable),
        }
    }
}
impl<F: ClientMetadataSource> MetadataIntegrity for DraftMetadataIntegrity<F> {
    fn check_client_id<'a>(
        &'a self,
        client_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<(), MetadataIntegrityError>> + Send + 'a>> {
        Box::pin(self.check(client_id))
    }
    fn check_client_pk(
        &self,
        client_pk: u64,
    ) -> Pin<Box<dyn Future<Output = Result<(), MetadataIntegrityError>> + Send + '_>> {
        Box::pin(async move {
            let id =
                sqlx::query_scalar::<_, String>("SELECT client_id FROM oauth_clients WHERE id=?")
                    .bind(client_pk)
                    .fetch_optional(&self.pool)
                    .await
                    .map_err(|_| MetadataIntegrityError::Unavailable)?
                    .ok_or(MetadataIntegrityError::Invalid)?;
            self.check(&id).await
        })
    }
}
