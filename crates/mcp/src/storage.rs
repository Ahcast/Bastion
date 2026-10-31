use chrono::NaiveDateTime;
use sqlx::{MySql, MySqlPool, Transaction};

#[derive(Clone)]
pub struct MysqlOauthStore {
    pool: MySqlPool,
}

#[derive(Debug)]
pub enum StoreError {
    NotFound,
    Database(sqlx::Error),
}

impl From<sqlx::Error> for StoreError {
    fn from(error: sqlx::Error) -> Self {
        Self::Database(error)
    }
}

impl MysqlOauthStore {
    pub fn new(pool: MySqlPool) -> Self {
        Self { pool }
    }

    /// The actor must be obtained from a currently authenticated Core session,
    /// not from the request body. The Core HTTP layer must check CSRF and origin.
    /// The lock, revocation, and audit insert commit as one transaction.
    pub async fn revoke_own_grant(&self, actor: u64, grant_id: u64) -> Result<(), StoreError> {
        let mut tx = self.pool.begin().await?;
        let row = sqlx::query_as::<_, (Option<NaiveDateTime>,)>(
            "SELECT g.revoked_at FROM oauth_grants g JOIN users u ON u.id=g.user_id WHERE g.id=? AND g.user_id=? AND u.status='active' FOR UPDATE",
        )
        .bind(grant_id)
        .bind(actor)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(StoreError::NotFound)?;
        if row.0.is_none() {
            sqlx::query("UPDATE oauth_grants SET revoked_at=UTC_TIMESTAMP(6),revocation_reason='user_revoked' WHERE id=? AND revoked_at IS NULL")
                .bind(grant_id).execute(&mut *tx).await?;
            sqlx::query("INSERT INTO audit_events (actor_id,module_id,action,target_type,target_id,result,created_at) VALUES (?,'core','oauth.grant.revoke','oauth_grant',?,'success',UTC_TIMESTAMP(6))")
                .bind(actor).bind(grant_id.to_string()).execute(&mut *tx).await?;
        }
        tx.commit().await?;
        Ok(())
    }
}

/// Intended for the existing password-change, recovery and user-deactivation
/// transactions after the OAuth migration is registered. The caller must lock
/// the user and perform this update before committing the security change.
/// Returning an error must abort the surrounding transaction.
pub async fn revoke_user_grants_in_transaction(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    reason: &'static str,
) -> Result<u64, sqlx::Error> {
    // Do not allow arbitrary values to become persistent audit/state text.
    if !matches!(
        reason,
        "password_changed" | "password_reset" | "user_disabled"
    ) {
        return Err(sqlx::Error::Protocol(
            "invalid grant revocation reason".into(),
        ));
    }
    let result = sqlx::query("UPDATE oauth_grants SET revoked_at=UTC_TIMESTAMP(6),revocation_reason=? WHERE user_id=? AND revoked_at IS NULL")
        .bind(reason).bind(user_id).execute(&mut **tx).await?;
    Ok(result.rows_affected())
}
