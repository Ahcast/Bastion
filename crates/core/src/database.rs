use std::time::Duration;

use sqlx::{
    MySqlPool,
    mysql::{MySqlConnectOptions, MySqlPoolOptions, MySqlSslMode},
};

use crate::DatabaseConfig;

#[derive(Clone)]
pub struct Database {
    pub(crate) pool: MySqlPool,
}

impl Database {
    #[cfg(feature = "test-support")]
    pub fn from_test_pool(pool: MySqlPool) -> Self {
        Self { pool }
    }

    /// Shared pool for explicitly composed domain repositories.
    pub fn pool(&self) -> &MySqlPool {
        &self.pool
    }

    pub fn connect_lazy(config: &DatabaseConfig) -> Self {
        let options = MySqlConnectOptions::new()
            .host(&config.host)
            .port(config.port)
            .username(&config.username)
            .password(&config.password)
            .database(&config.database)
            .ssl_mode(MySqlSslMode::VerifyIdentity)
            .ssl_ca(&config.tls_ca);
        let pool = MySqlPoolOptions::new()
            .max_connections(10)
            .min_connections(0)
            .acquire_timeout(Duration::from_secs(5))
            .idle_timeout(Duration::from_secs(300))
            .max_lifetime(Duration::from_secs(1800))
            .connect_lazy_with(options);
        Self { pool }
    }

    pub async fn migrate(&self) -> Result<(), sqlx::migrate::MigrateError> {
        sqlx::migrate!("./migrations").run(&self.pool).await
    }

    pub async fn check_ready(&self) -> Result<(), sqlx::Error> {
        sqlx::query_scalar::<_, i32>("SELECT 1")
            .fetch_one(&self.pool)
            .await
            .map(|_| ())
    }
}
