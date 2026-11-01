use chrono::{Duration, NaiveDateTime};
use sha2::{Digest, Sha256};
use sqlx::MySqlPool;

/// Server-owned limits, not values provided by the MCP client. The window and
/// total limits must be identical for all tools in the same server deployment.
#[derive(Clone, Copy)]
pub struct ActorQuota {
    pub window_seconds: u32,
    pub user_total: u32,
    pub client_total: u32,
}

#[derive(Clone, Copy)]
pub struct ToolQuota {
    pub user_tool: u32,
    pub client_tool: u32,
}

#[derive(Debug)]
pub enum QuotaError {
    InvalidConfiguration,
    Limited { retry_after_seconds: u64 },
    Database(sqlx::Error),
}

impl From<sqlx::Error> for QuotaError {
    fn from(value: sqlx::Error) -> Self {
        Self::Database(value)
    }
}

#[derive(Clone)]
pub struct MysqlQuota {
    pool: MySqlPool,
    limits: ActorQuota,
}

impl MysqlQuota {
    pub fn new(pool: MySqlPool, limits: ActorQuota) -> Result<Self, QuotaError> {
        if !(1..=3600).contains(&limits.window_seconds)
            || limits.user_total == 0
            || limits.client_total == 0
        {
            return Err(QuotaError::InvalidConfiguration);
        }
        Ok(Self { pool, limits })
    }

    /// Charge four independent buckets atomically. The tool identifier and
    /// policy must come from the trusted registry after token authorization.
    /// The caller records the denied or accepted invocation in audit.
    pub async fn charge(
        &self,
        user_id: u64,
        client_id: u64,
        tool: &str,
        tool_limits: ToolQuota,
    ) -> Result<(), QuotaError> {
        if user_id == 0
            || client_id == 0
            || tool.is_empty()
            || tool.len() > 128
            || !tool
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b))
            || tool_limits.user_tool == 0
            || tool_limits.client_tool == 0
        {
            return Err(QuotaError::InvalidConfiguration);
        }
        let global: [u8; 32] = Sha256::digest(b"bastion/mcp/quota/global/v1").into();
        let mut hasher = Sha256::new();
        hasher.update(b"bastion/mcp/quota/tool/v1\0");
        hasher.update(tool.as_bytes());
        let specific: [u8; 32] = hasher.finalize().into();
        // Consistent lock order avoids reversing user/client bucket locks.
        let buckets = [
            (1u8, user_id, global, self.limits.user_total),
            (1u8, user_id, specific, tool_limits.user_tool),
            (2u8, client_id, global, self.limits.client_total),
            (2u8, client_id, specific, tool_limits.client_tool),
        ];
        let mut tx = self.pool.begin().await?;
        let now: NaiveDateTime = sqlx::query_scalar("SELECT UTC_TIMESTAMP(6)")
            .fetch_one(&mut *tx)
            .await?;
        for (kind, id, hash, limit) in buckets {
            sqlx::query("INSERT INTO oauth_actor_quotas (actor_kind,actor_id,bucket_hash,window_start,requests) VALUES (?,?,?,?,0) ON DUPLICATE KEY UPDATE actor_id=actor_id")
                .bind(kind).bind(id).bind(hash.as_slice()).bind(now).execute(&mut *tx).await?;
            let (start,count):(NaiveDateTime,u32)=sqlx::query_as("SELECT window_start,requests FROM oauth_actor_quotas WHERE actor_kind=? AND actor_id=? AND bucket_hash=? FOR UPDATE")
                .bind(kind).bind(id).bind(hash.as_slice()).fetch_one(&mut *tx).await?;
            // Another transaction may have created the bucket while this one
            // waited, so sample the clock again after taking the lock.
            let now: NaiveDateTime = sqlx::query_scalar("SELECT UTC_TIMESTAMP(6)")
                .fetch_one(&mut *tx)
                .await?;
            let end = start + Duration::seconds(i64::from(self.limits.window_seconds));
            if start > now {
                return Err(QuotaError::InvalidConfiguration);
            }
            if end <= now {
                sqlx::query("UPDATE oauth_actor_quotas SET window_start=?,requests=1 WHERE actor_kind=? AND actor_id=? AND bucket_hash=?")
                    .bind(now).bind(kind).bind(id).bind(hash.as_slice()).execute(&mut *tx).await?;
            } else if count >= limit {
                let remaining = (end - now).num_milliseconds().max(1);
                return Err(QuotaError::Limited {
                    retry_after_seconds: ((remaining + 999) / 1000) as u64,
                });
            } else {
                sqlx::query("UPDATE oauth_actor_quotas SET requests=requests+1 WHERE actor_kind=? AND actor_id=? AND bucket_hash=?")
                    .bind(kind).bind(id).bind(hash.as_slice()).execute(&mut *tx).await?;
            }
        }
        tx.commit().await?;
        Ok(())
    }
}
