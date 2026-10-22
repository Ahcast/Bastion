use std::{error::Error, net::SocketAddr};

use bastion_api::router;
use bastion_core::{AppConfig, CoreService, Database, DatabaseConfig};
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;

mod scout_import;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    let _ = dotenvy::dotenv();
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_target(false)
        .compact()
        .init();

    let command = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "serve".to_owned());
    if command == "check-db" {
        let database = Database::connect_lazy(&DatabaseConfig::from_env()?);
        database.check_ready().await?;
        tracing::info!("database connection verified with TLS VerifyIdentity (SELECT 1)");
        return Ok(());
    }
    if command == "migrate" {
        let migrator = Database::connect_lazy(&DatabaseConfig::migrator_from_env()?);
        migrator.migrate().await?;
        bastion_procesos::ProcessService::migrate(&migrator).await?;
        bastion_cotizaciones::QuoteService::migrate(&migrator).await?;
        tracing::info!("database migrations completed");
        return Ok(());
    }
    if command != "serve" {
        return Err(std::io::Error::other("unknown command").into());
    }
    let config = AppConfig::from_env()?;
    let scout_config = scout_import::ScoutImportConfig::from_env()?;
    let scout_csv_path = scout_config.csv_path.clone();
    if scout_config.enabled && !config.procesos_allowed {
        return Err(std::io::Error::other(
            "Local Scout import requires procesos in BASTION_ALLOWED_MODULES",
        )
        .into());
    }
    let database = Database::connect_lazy(&config.database);
    let app_origin = config.cors_allowed_origin.clone();
    let core = CoreService::new(
        database,
        config.bootstrap_secret,
        config.data_encryption_key,
    )?
    .with_public_app_origin(app_origin.clone())
    .with_local_scout_import(scout_config.enabled);
    core.check_ready().await?;
    bastion_procesos::ProcessService::new(core.database(), core.admin())
        .synchronize_release(config.procesos_allowed)
        .await?;
    bastion_cotizaciones::QuoteService::new(core.database(), core.admin())
        .synchronize_release(config.cotizaciones_allowed)
        .await?;
    if scout_config.enabled {
        tokio::spawn(scout_import::run(
            bastion_procesos::ProcessService::new(core.database(), core.admin()),
            scout_config,
        ));
    }
    let cleanup = core.admin();
    cleanup.cleanup_retention().await?;
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(24 * 60 * 60)).await;
            if let Err(error) = cleanup.cleanup_retention().await {
                let _ = error;
                tracing::error!("retention cleanup failed");
            }
        }
    });
    let mailer = core.clone();
    tokio::spawn(async move {
        loop {
            match mailer.deliver_one_mail().await {
                Ok(_) => {}
                Err(_) => tracing::error!("mail outbox worker failed"),
            }
            tokio::time::sleep(std::time::Duration::from_secs(10)).await;
        }
    });
    let app = router(core, app_origin, config.trusted_proxy_cidrs, scout_csv_path)?;
    let listener = TcpListener::bind(config.listen_addr).await?;
    let local_addr: SocketAddr = listener.local_addr()?;
    tracing::info!(%local_addr, "Bastion Core server listening");
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await?;
    Ok(())
}

async fn shutdown_signal() {
    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("could not register SIGTERM handler")
            .recv()
            .await;
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! { _ = tokio::signal::ctrl_c() => {}, _ = terminate => {}, }
    tracing::info!("shutdown signal received");
}
