use super::*;

fn otp(secret: &str) -> String {
    let key = BASE32_NOPAD.decode(secret.as_bytes()).unwrap();
    let mut mac = <HmacSha1 as Mac>::new_from_slice(&key).unwrap();
    mac.update(&(Utc::now().timestamp() as u64 / 30).to_be_bytes());
    let hash = mac.finalize().into_bytes();
    let at = (hash[19] & 15) as usize;
    let n = u32::from_be_bytes(hash[at..at + 4].try_into().unwrap()) & 0x7fff_ffff;
    format!("{:06}", n % 1_000_000)
}

#[tokio::test]
async fn cancelled_password_work_keeps_its_slot_until_the_worker_finishes() {
    let pool = sqlx::mysql::MySqlPoolOptions::new()
        .connect_lazy("mysql://test@127.0.0.1/unused")
        .unwrap();
    let mut core = CoreService::new(Database { pool }, "x".repeat(32), [7; 32]).unwrap();
    core.password_kdf_semaphore = Arc::new(Semaphore::new(1));
    assert!(
        !core
            .verify_account_password("constant-time-dummy-password", None)
            .await
            .unwrap()
    );
    let worker = core.clone();
    let (entered, started) = tokio::sync::oneshot::channel();
    let (release, wait) = std::sync::mpsc::channel();
    let task = tokio::spawn(async move {
        worker
            .password_work(move || {
                let _ = entered.send(());
                wait.recv_timeout(Duration::from_secs(5)).unwrap();
            })
            .await
    });
    // A blocked worker must not stall this single-thread async runtime.
    tokio::time::timeout(Duration::from_secs(2), started)
        .await
        .unwrap()
        .unwrap();
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    assert!(matches!(
        core.password_work(|| ()).await,
        Err(CoreError::Overloaded)
    ));
    release.send(()).unwrap();
    tokio::time::timeout(Duration::from_secs(2), async {
        while core.password_kdf_semaphore.available_permits() == 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    core.password_work(|| ()).await.unwrap();
}

async fn reset_link(core: &CoreService, user: u64, raw: &str) -> u64 {
    let token = sqlx::query("INSERT INTO account_tokens(user_id,token_type,token_hash,expires_at,mailed_at) VALUES (?,'password_reset',?,UTC_TIMESTAMP(6)+INTERVAL 1 HOUR,UTC_TIMESTAMP(6))")
        .bind(user).bind(token_hash(raw).to_vec()).execute(core.database.pool()).await.unwrap().last_insert_id();
    sqlx::query("INSERT INTO mail_outbox(kind,sender_user_id,recipient_email,subject,body_ciphertext,account_token_id,available_at) VALUES ('password_reset',?,'synthetic@example.test','Synthetic reset',?,?,UTC_TIMESTAMP(6))")
        .bind(user).bind(core.encrypt("synthetic recovery link").unwrap()).bind(token).execute(core.database.pool()).await.unwrap();
    token
}

#[tokio::test]
async fn mysql57_password_change_invalidates_old_links_and_preserves_totp_and_recovery() {
    let Ok(url) = std::env::var("BASTION_CORE_SECURITY_TEST_URL") else {
        eprintln!("Requires isolated BASTION_CORE_SECURITY_TEST_URL; CI supplies it.");
        return;
    };
    let pool = sqlx::mysql::MySqlPoolOptions::new()
        .max_connections(5)
        .connect(&url)
        .await
        .unwrap();
    let database = Database { pool: pool.clone() };
    database.migrate().await.unwrap();
    let core = CoreService::new(database, "synthetic-bootstrap".repeat(3), [9; 32]).unwrap();
    let setup = core
        .begin_bootstrap(
            &"synthetic-bootstrap".repeat(3),
            "Synthetic security owner",
            "security@example.test",
            "Original synthetic password",
        )
        .await
        .unwrap();
    core.complete_bootstrap(&setup.setup_token, &otp(&setup.totp_secret))
        .await
        .unwrap();
    let user: u64 = sqlx::query_scalar("SELECT id FROM users WHERE email='security@example.test'")
        .fetch_one(&pool)
        .await
        .unwrap();
    // Reset replay counter only in this disposable fixture between independent actions.
    sqlx::query("UPDATE users SET last_totp_counter=NULL WHERE id=?")
        .bind(user)
        .execute(&pool)
        .await
        .unwrap();
    let challenge = match core
        .start_login(
            "security@example.test",
            "Original synthetic password",
            "192.0.2.1",
            "test",
            false,
            None,
        )
        .await
        .unwrap()
    {
        LoginStart::TotpRequired { challenge_token } => challenge_token,
        _ => panic!("new device must require TOTP"),
    };
    let cookies = core
        .verify_login_totp(&challenge, &otp(&setup.totp_secret), None)
        .await
        .unwrap();
    let first = reset_link(&core, user, "old-reset-a").await;
    let second = reset_link(&core, user, "old-reset-b").await;
    assert!(
        core.change_password(
            &cookies.session_token,
            "wrong password",
            "New synthetic password",
            "invalid",
            "192.0.2.1",
            "test"
        )
        .await
        .is_err()
    );
    let untouched: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM account_tokens WHERE id IN (?,?) AND consumed_at IS NULL",
    )
    .bind(first)
    .bind(second)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(untouched, 2);
    sqlx::query("UPDATE users SET last_totp_counter=NULL WHERE id=?")
        .bind(user)
        .execute(&pool)
        .await
        .unwrap();
    core.change_password(
        &cookies.session_token,
        "Original synthetic password",
        "New synthetic password",
        &otp(&setup.totp_secret),
        "192.0.2.1",
        "test",
    )
    .await
    .unwrap();
    assert!(core.authenticate(&cookies.session_token).await.is_err());
    for raw in ["old-reset-a", "old-reset-b"] {
        assert!(matches!(
            core.complete_password_reset(raw, "Attacker replacement password", "192.0.2.2")
                .await,
            Err(CoreError::Unauthorized)
        ));
    }
    let cancelled: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM mail_outbox WHERE account_token_id IN (?,?) AND delivery_status='cancelled' AND LENGTH(body_ciphertext)=0").bind(first).bind(second).fetch_one(&pool).await.unwrap();
    assert_eq!(cancelled, 2);
    assert!(matches!(
        core.start_login(
            "security@example.test",
            "New synthetic password",
            "192.0.2.3",
            "test",
            false,
            None
        )
        .await
        .unwrap(),
        LoginStart::TotpRequired { .. }
    ));
    reset_link(&core, user, "fresh-reset").await;
    reset_link(&core, user, "other-pending-reset").await;
    core.complete_password_reset("fresh-reset", "Recovered synthetic password", "192.0.2.2")
        .await
        .unwrap();
    for raw in ["fresh-reset", "other-pending-reset"] {
        assert!(matches!(
            core.complete_password_reset(raw, "Unwanted replacement password", "192.0.2.2")
                .await,
            Err(CoreError::Unauthorized)
        ));
    }
    assert!(matches!(
        core.start_login(
            "security@example.test",
            "Recovered synthetic password",
            "192.0.2.4",
            "test",
            false,
            None
        )
        .await
        .unwrap(),
        LoginStart::TotpRequired { .. }
    ));
    // Locked and nonexistent accounts have the same public error.
    sqlx::query("UPDATE users SET locked_until=UTC_TIMESTAMP(6)+INTERVAL 30 MINUTE WHERE id=?")
        .bind(user)
        .execute(&pool)
        .await
        .unwrap();
    for email in ["security@example.test", "absent@example.test"] {
        assert!(matches!(
            core.start_login(email, "wrong password", "192.0.2.5", "test", false, None)
                .await,
            Err(CoreError::Unauthorized)
        ));
    }
    for _ in 0..30 {
        core.login_limits.admit("192.0.2.6").unwrap();
    }
    pool.close().await;
    // Admission is shared across clones and rejected before touching even a closed pool.
    assert!(matches!(
        core.clone()
            .start_login(
                "absent@example.test",
                "wrong password",
                "192.0.2.6",
                "test",
                false,
                None
            )
            .await,
        Err(CoreError::Locked)
    ));
}
