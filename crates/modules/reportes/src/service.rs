use bastion_core::{CoreAdmin, CoreError, Database};
use sqlx::{MySql, Transaction};

#[derive(Clone)]
pub struct ReportService {
    database: Database,
    admin: CoreAdmin,
}

impl ReportService {
    pub fn new(database: Database, admin: CoreAdmin) -> Self {
        Self { database, admin }
    }

    pub async fn migrate(database: &Database) -> Result<(), CoreError> {
        let mut migrator = sqlx::migrate!("./migrations");
        migrator.dangerous_set_table_name("_reportes_migrations");
        migrator
            .run(database.pool())
            .await
            .map_err(CoreError::Migration)
    }

    pub async fn synchronize_release(&self, allowed: bool) -> Result<(), CoreError> {
        let exists: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM information_schema.tables WHERE table_schema=DATABASE() AND table_name='_reportes_migrations'")
            .fetch_one(self.database.pool())
            .await?;
        let ready = if exists > 0 {
            let applied = sqlx::query_as::<_, (i64, bool, Vec<u8>)>(
                "SELECT version,success,checksum FROM _reportes_migrations",
            )
            .fetch_all(self.database.pool())
            .await?;
            let expected = sqlx::migrate!("./migrations");
            applied.len() == expected.iter().count()
                && expected.iter().all(|migration| {
                    applied.iter().any(|(version, success, checksum)| {
                        *version == migration.version
                            && *success
                            && checksum.as_slice() == migration.checksum.as_ref()
                    })
                })
        } else {
            false
        };
        sqlx::query("INSERT INTO module_registry (module_id,display_name,included_in_release,environment_allowed,enabled) VALUES ('reportes','Reportes',1,?,0) ON DUPLICATE KEY UPDATE included_in_release=1,environment_allowed=VALUES(environment_allowed)")
            .bind(allowed && ready)
            .execute(self.database.pool())
            .await?;
        Ok(())
    }

    async fn begin(&self, actor: u64, scope: &str) -> Result<Transaction<'_, MySql>, CoreError> {
        if !["ordenes", "facturas", "gastos"].contains(&scope) {
            return Err(CoreError::Invalid);
        }
        self.admin
            .authorize(actor, "reportes", scope, "ver")
            .await?;
        let mut tx = self.database.pool().begin().await?;
        let enabled: Option<bool> = sqlx::query_scalar("SELECT (included_in_release AND environment_allowed AND enabled) FROM module_registry WHERE module_id='reportes' FOR UPDATE")
            .fetch_optional(&mut *tx)
            .await?;
        if enabled != Some(true) {
            return Err(CoreError::NotFound);
        }
        Ok(tx)
    }

    pub async fn check_access(&self, actor: u64, scope: &str) -> Result<(), CoreError> {
        self.begin(actor, scope).await?.commit().await?;
        Ok(())
    }

    pub async fn consume_request(&self, actor: u64) -> Result<(), CoreError> {
        let mut tx = self.database.pool().begin().await?;
        sqlx::query("INSERT INTO reportes_actor_limits (actor_id,window_start,requests) VALUES (?,UTC_TIMESTAMP(6),1) ON DUPLICATE KEY UPDATE requests=IF(window_start<=UTC_TIMESTAMP(6)-INTERVAL 1 MINUTE,1,requests+1),window_start=IF(window_start<=UTC_TIMESTAMP(6)-INTERVAL 1 MINUTE,UTC_TIMESTAMP(6),window_start)")
            .bind(actor)
            .execute(&mut *tx)
            .await?;
        let count: u32 =
            sqlx::query_scalar("SELECT requests FROM reportes_actor_limits WHERE actor_id=?")
                .bind(actor)
                .fetch_one(&mut *tx)
                .await?;
        tx.commit().await?;
        if count > 30 {
            return Err(CoreError::Locked);
        }
        Ok(())
    }

    pub async fn record_report_event(
        &self,
        actor: u64,
        kind: &str,
        action: &str,
        date_from: &str,
        date_to: &str,
        records: u64,
    ) -> Result<(), CoreError> {
        if !["ordenes", "facturas", "gastos"].contains(&kind)
            || !["report.view", "report.export"].contains(&action)
        {
            return Err(CoreError::Invalid);
        }
        let mut tx = self.begin(actor, kind).await?;
        CoreAdmin::record_domain_event(
            &mut tx,
            actor,
            "reportes",
            action,
            kind,
            serde_json::json!({"date_from": date_from, "date_to": date_to, "records": records}),
        )
        .await?;
        tx.commit().await?;
        Ok(())
    }
}
