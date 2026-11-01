use sha2::{Digest, Sha256};
use sqlx::{MySql, Transaction};

pub struct OAuthSessionIdentity {
    pub user_id: u64,
    pub session_id: u64,
}

/// Resolve and lock a browser session in the transaction that consumes OAuth
/// consent. This enforces the same activity, expiry, status and TOTP limits as
/// Core authentication. CSRF, exact Origin and explicit consent are separate.
pub async fn oauth_session_in_transaction(
    tx: &mut Transaction<'_, MySql>,
    session_token: &str,
) -> Result<Option<OAuthSessionIdentity>, sqlx::Error> {
    if session_token.is_empty() || session_token.len() > 2048 {
        return Ok(None);
    }
    let hash = Sha256::digest(session_token.as_bytes());
    let user_id = sqlx::query_scalar::<_, u64>("SELECT user_id FROM sessions WHERE token_hash=?")
        .bind(hash.as_slice())
        .fetch_optional(&mut **tx)
        .await?;
    let Some(user_id) = user_id else {
        return Ok(None);
    };
    let active = sqlx::query_scalar::<_, u64>(
        "SELECT id FROM users WHERE id=? AND status='active' FOR UPDATE",
    )
    .bind(user_id)
    .fetch_optional(&mut **tx)
    .await?;
    if active.is_none() {
        return Ok(None);
    }
    let session_id = sqlx::query_scalar::<_,u64>("SELECT id FROM sessions WHERE token_hash=? AND user_id=? AND revoked_at IS NULL AND (expires_at IS NULL OR expires_at>UTC_TIMESTAMP(6)) AND (keep_logged_in=1 OR last_activity_at>UTC_TIMESTAMP(6)-INTERVAL 24 HOUR) AND last_totp_at>UTC_TIMESTAMP(6)-INTERVAL 15 DAY FOR UPDATE")
        .bind(hash.as_slice()).bind(user_id).fetch_optional(&mut **tx).await?;
    Ok(session_id.map(|session_id| OAuthSessionIdentity {
        user_id,
        session_id,
    }))
}
