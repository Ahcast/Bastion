use super::*;

#[derive(FromRow)]
struct RotationSession {
    session_id: u64,
    user_id: u64,
    email: String,
    password_hash: Option<String>,
    totp_secret_enc: Vec<u8>,
    last_totp_counter: Option<i64>,
}

async fn lock_session(
    tx: &mut Transaction<'_, MySql>,
    token: &str,
) -> Result<RotationSession, CoreError> {
    let hash = token_hash(token).to_vec();
    // Match the login lock order: user before sessions and challenges.
    sqlx::query_scalar::<_, u64>("SELECT id FROM users WHERE id=(SELECT user_id FROM sessions WHERE token_hash=?) FOR UPDATE")
        .bind(&hash).fetch_optional(&mut **tx).await?.ok_or(CoreError::Unauthorized)?;
    sqlx::query_as("SELECT s.id AS session_id,u.id AS user_id,u.email,u.password_hash,u.totp_secret_enc,u.last_totp_counter FROM sessions s JOIN users u ON u.id=s.user_id WHERE s.token_hash=? AND s.revoked_at IS NULL AND u.status='active' AND u.totp_enabled=1 AND u.totp_secret_enc IS NOT NULL AND (s.expires_at IS NULL OR s.expires_at>UTC_TIMESTAMP(6)) AND (s.keep_logged_in=1 OR s.last_activity_at>UTC_TIMESTAMP(6)-INTERVAL 24 HOUR) FOR UPDATE")
        .bind(hash)
        .fetch_optional(&mut **tx).await?.ok_or(CoreError::Unauthorized)
}

fn rotation_hash(session_id: u64, token: &str) -> Vec<u8> {
    token_hash(&format!("totp-change:{session_id}:{token}")).to_vec()
}

impl CoreService {
    pub async fn begin_totp_change(
        &self,
        session_token: &str,
        password: &str,
        code: &str,
        ip: &str,
        agent: &str,
    ) -> Result<BootstrapBegin, CoreError> {
        validate_password_input(password)?;
        let _permit = self
            .password_kdf_semaphore
            .acquire()
            .await
            .map_err(|_| CoreError::Overloaded)?;
        let mut tx = self.database.pool.begin().await?;
        let row = lock_session(&mut tx, session_token).await?;
        if self.is_totp_limited(row.user_id, ip).await? {
            return Err(CoreError::Locked);
        }
        let secret = self.decrypt(&row.totp_secret_enc)?;
        let counter = verify_totp(&secret, code, row.last_totp_counter.unwrap_or(-1));
        let correct_password = row
            .password_hash
            .as_deref()
            .is_some_and(|hash| verify_password(password, hash));
        let now = Utc::now().naive_utc();
        let Some(counter) = counter.filter(|_| correct_password) else {
            sqlx::query("INSERT INTO security_events (user_id,event_type,result,ip_address,device_label,created_at) VALUES (?,'totp_sensitive_action','failed',?,?,?)")
                .bind(row.user_id).bind(ip).bind(safe_agent(agent)).bind(now).execute(&mut *tx).await?;
            tx.commit().await?;
            self.record_totp_failure(row.user_id, ip).await?;
            return Err(CoreError::Invalid);
        };
        let mut bytes = [0; 20];
        OsRng.fill_bytes(&mut bytes);
        let seed = BASE32_NOPAD.encode(&bytes);
        let ciphertext = self.encrypt(&seed)?;
        let setup_token = random_token();
        let expires_at = now + chrono::Duration::minutes(10);
        sqlx::query("UPDATE users SET last_totp_counter=? WHERE id=?")
            .bind(counter as i64)
            .bind(row.user_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM totp_enrollment_tokens WHERE user_id=? AND token_type='rotation'")
            .bind(row.user_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("INSERT INTO totp_enrollment_tokens (user_id,token_hash,secret_ciphertext,token_type,expires_at) VALUES (?,?,?,'rotation',?)")
            .bind(row.user_id).bind(rotation_hash(row.session_id, &setup_token)).bind(ciphertext).bind(expires_at).execute(&mut *tx).await?;
        tx.commit().await?;
        let label = url::form_urlencoded::byte_serialize(row.email.as_bytes()).collect::<String>();
        Ok(BootstrapBegin {
            setup_token,
            totp_secret: seed.clone(),
            otpauth_uri: format!(
                "otpauth://totp/Bastion:{label}?secret={seed}&issuer=Bastion&algorithm=SHA1&digits=6&period=30"
            ),
            expires_at,
        })
    }

    pub async fn complete_totp_change(
        &self,
        session_token: &str,
        setup_token: &str,
        code: &str,
        ip: &str,
        agent: &str,
    ) -> Result<BootstrapComplete, CoreError> {
        let mut tx = self.database.pool.begin().await?;
        let row = lock_session(&mut tx, session_token).await?;
        if self.is_totp_limited(row.user_id, ip).await? {
            return Err(CoreError::Locked);
        }
        let now = Utc::now().naive_utc();
        let (id, seed, attempts) = sqlx::query_as::<_, (u64, Vec<u8>, u8)>("SELECT id,secret_ciphertext,failed_attempts FROM totp_enrollment_tokens WHERE user_id=? AND token_hash=? AND token_type='rotation' AND consumed_at IS NULL AND expires_at>? FOR UPDATE")
            .bind(row.user_id).bind(rotation_hash(row.session_id, setup_token)).bind(now).fetch_optional(&mut *tx).await?.ok_or(CoreError::Conflict)?;
        let Some(counter) = verify_totp(&self.decrypt(&seed)?, code, -1) else {
            let attempts = attempts.saturating_add(1);
            sqlx::query("UPDATE totp_enrollment_tokens SET failed_attempts=?,consumed_at=IF(?,?,NULL) WHERE id=?")
                .bind(attempts).bind(attempts >= 3).bind(now).bind(id).execute(&mut *tx).await?;
            sqlx::query("INSERT INTO security_events (user_id,event_type,result,ip_address,device_label,created_at) VALUES (?,'totp_sensitive_action','failed',?,?,?)")
                .bind(row.user_id).bind(ip).bind(safe_agent(agent)).bind(now).execute(&mut *tx).await?;
            tx.commit().await?;
            self.record_totp_failure(row.user_id, ip).await?;
            return Err(CoreError::Invalid);
        };
        sqlx::query("UPDATE users SET totp_secret_enc=?,last_totp_counter=? WHERE id=?")
            .bind(seed)
            .bind(counter as i64)
            .bind(row.user_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query(
            "UPDATE totp_recovery_codes SET consumed_at=? WHERE user_id=? AND consumed_at IS NULL",
        )
        .bind(now)
        .bind(row.user_id)
        .execute(&mut *tx)
        .await?;
        let recovery_codes = (0..10).map(|_| recovery_code()).collect::<Vec<_>>();
        for code in &recovery_codes {
            sqlx::query("INSERT INTO totp_recovery_codes (user_id,code_hash) VALUES (?,?)")
                .bind(row.user_id)
                .bind(token_hash(code).to_vec())
                .execute(&mut *tx)
                .await?;
        }
        sqlx::query("DELETE FROM totp_enrollment_tokens WHERE id=?")
            .bind(id)
            .execute(&mut *tx)
            .await?;
        revoke_user_access(&mut tx, row.user_id, now, "totp_changed").await?;
        sqlx::query("INSERT INTO security_events (user_id,event_type,result,ip_address,device_label,created_at) VALUES (?,'totp_change','success',?,?,?)")
            .bind(row.user_id).bind(ip).bind(safe_agent(agent)).bind(now).execute(&mut *tx).await?;
        insert_audit(
            &mut tx,
            Some(row.user_id),
            "core",
            "account.totp.change",
            "user",
            Some(row.user_id.to_string()),
            "success",
        )
        .await?;
        tx.commit().await?;
        Ok(BootstrapComplete {
            recovery_codes,
            status: "active",
        })
    }
}
