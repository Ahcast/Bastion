use bastion_core::Database;
use bastion_mcp::{ActorQuota, MysqlQuota, QuotaError, ToolQuota};
use sqlx::mysql::MySqlPoolOptions;

#[tokio::test]
async fn mysql57_limits_actors_tools_and_concurrent_calls_atomically() {
    let Ok(url) = std::env::var("BASTION_MCP_TEST_URL") else {
        return;
    };
    let pool = MySqlPoolOptions::new()
        .max_connections(12)
        .connect(&url)
        .await
        .unwrap();
    Database::from_test_pool(pool.clone())
        .migrate()
        .await
        .unwrap();
    let limits = ActorQuota {
        window_seconds: 60,
        user_total: 3,
        client_total: 10,
    };
    let quota = MysqlQuota::new(pool.clone(), limits).unwrap();
    let limits = ToolQuota {
        user_tool: 2,
        client_tool: 10,
    };
    quota.charge(99001, 99011, "buscar", limits).await.unwrap();
    quota.charge(99001, 99011, "buscar", limits).await.unwrap();
    assert!(matches!(
        quota.charge(99001, 99011, "buscar", limits).await,
        Err(QuotaError::Limited { .. })
    ));
    // A rejected tool bucket must roll back the earlier global charge.
    quota.charge(99001, 99011, "detalle", limits).await.unwrap();
    assert!(matches!(
        quota.charge(99001, 99012, "detalle", limits).await,
        Err(QuotaError::Limited { .. })
    ));
    let parallel_global = ActorQuota {
        window_seconds: 60,
        user_total: 5,
        client_total: 5,
    };
    let parallel_quota = MysqlQuota::new(pool.clone(), parallel_global).unwrap();
    let parallel = ToolQuota {
        user_tool: 5,
        client_tool: 5,
    };
    let mut tasks = tokio::task::JoinSet::new();
    for _ in 0..10 {
        let q = parallel_quota.clone();
        tasks.spawn(async move { q.charge(99002, 99022, "buscar", parallel).await });
    }
    let (mut accepted, mut limited) = (0, 0);
    while let Some(result) = tasks.join_next().await {
        match result.unwrap() {
            Ok(()) => accepted += 1,
            Err(QuotaError::Limited {
                retry_after_seconds,
            }) => {
                assert!((1..=60).contains(&retry_after_seconds));
                limited += 1;
            }
            Err(error) => panic!("unexpected quota error: {error:?}"),
        }
    }
    assert_eq!((accepted, limited), (5, 5));
    sqlx::query("UPDATE oauth_actor_quotas SET window_start=UTC_TIMESTAMP(6)-INTERVAL 2 MINUTE WHERE actor_id IN (99001,99011,99012)").execute(&pool).await.unwrap();
    quota.charge(99001, 99011, "buscar", limits).await.unwrap();
    assert!(matches!(
        quota.charge(0, 99011, "buscar", limits).await,
        Err(QuotaError::InvalidConfiguration)
    ));
}
