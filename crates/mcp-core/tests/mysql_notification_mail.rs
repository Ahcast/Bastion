use bastion_core::{CoreService, Database};
use bastion_mcp::Credential;
use sqlx::mysql::MySqlPoolOptions;

#[tokio::test]
async fn mysql57_mcp_notifications_are_queued_once_and_track_delivery() {
    let Ok(url) = std::env::var("BASTION_MCP_TEST_URL") else {
        return;
    };
    let pool = MySqlPoolOptions::new()
        .max_connections(5)
        .connect(&url)
        .await
        .unwrap();
    let db = Database::from_test_pool(pool.clone());
    db.migrate().await.unwrap();
    let core = CoreService::new(db, Credential::generate().unwrap().expose(), [23; 32]).unwrap();
    let admin_role: u64 =
        sqlx::query_scalar("SELECT id FROM roles WHERE system_key='system_administrator'")
            .fetch_one(&pool)
            .await
            .unwrap();
    let person = sqlx::query("INSERT INTO persons (full_name) VALUES ('MCP mail fixture')")
        .execute(&pool)
        .await
        .unwrap()
        .last_insert_id();
    let admin = sqlx::query("INSERT INTO users (person_id,role_id,email,status) VALUES (?,?,'mcp-mail-fixture@example.invalid','active')").bind(person).bind(admin_role).execute(&pool).await.unwrap().last_insert_id();
    assert!(core.queue_one_mcp_notification(admin).await.is_err());
    assert!(!core.mcp_notification_sender_ready(admin).await.unwrap());
    assert!(core.database().core_schema_matches().await.unwrap());
    // This opaque fixture is never decrypted or sent; it is not a password.
    sqlx::query("INSERT INTO user_smtp_credentials (user_id,smtp_email,password_ciphertext) VALUES (?,'mcp-mail-fixture@example.invalid',?)").bind(admin).bind(Credential::generate().unwrap().expose().as_bytes()).execute(&pool).await.unwrap();
    assert!(core.mcp_notification_sender_ready(admin).await.unwrap());
    // Drain other isolated test notices before checking concurrency on our row.
    for _ in 0..200 {
        if !core.queue_one_mcp_notification(admin).await.unwrap() {
            break;
        }
    }
    let id=sqlx::query("INSERT INTO admin_notifications (recipient_user_id,kind,subject,payload_json,delivery_status,created_at) VALUES (?,'oauth_client_request','fixture',JSON_OBJECT(),'pending',UTC_TIMESTAMP(6))").bind(admin).execute(&pool).await.unwrap().last_insert_id();
    let (a, b) = tokio::join!(
        core.queue_one_mcp_notification(admin),
        core.queue_one_mcp_notification(admin)
    );
    assert_ne!(a.unwrap(), b.unwrap());
    let (status, mail_id): (String, u64) =
        sqlx::query_as("SELECT delivery_status,mail_outbox_id FROM admin_notifications WHERE id=?")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(status, "queued");
    let (recipient, body): (String, Vec<u8>) =
        sqlx::query_as("SELECT recipient_email,body_ciphertext FROM mail_outbox WHERE id=?")
            .bind(mail_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(recipient, "mcp-mail-fixture@example.invalid");
    assert!(!String::from_utf8_lossy(&body).contains("solicitud"));
    sqlx::query(
        "UPDATE mail_outbox SET delivery_status='failed',last_error_code='fixture' WHERE id=?",
    )
    .bind(mail_id)
    .execute(&pool)
    .await
    .unwrap();
    core.reconcile_mcp_notification_mail().await.unwrap();
    let status: String =
        sqlx::query_scalar("SELECT delivery_status FROM admin_notifications WHERE id=?")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(status, "failed");
    sqlx::query("UPDATE mail_outbox SET delivery_status='pending',last_error_code=NULL WHERE id=?")
        .bind(mail_id)
        .execute(&pool)
        .await
        .unwrap();
    core.reconcile_mcp_notification_mail().await.unwrap();
    let status: String =
        sqlx::query_scalar("SELECT delivery_status FROM admin_notifications WHERE id=?")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(status, "queued");
    sqlx::query(
        "UPDATE mail_outbox SET delivery_status='sent',sent_at=UTC_TIMESTAMP(6) WHERE id=?",
    )
    .bind(mail_id)
    .execute(&pool)
    .await
    .unwrap();
    core.reconcile_mcp_notification_mail().await.unwrap();
    let status: String =
        sqlx::query_scalar("SELECT delivery_status FROM admin_notifications WHERE id=?")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(status, "sent");
}
