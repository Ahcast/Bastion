use bastion_procesos::{DEFAULT_SCOUT_CSV, ProcessService, ReadyDelivery};
use std::{path::PathBuf, time::Duration};

pub struct ScoutImportConfig {
    pub enabled: bool,
    pub csv_path: PathBuf,
}

impl ScoutImportConfig {
    pub fn from_env() -> std::io::Result<Self> {
        let enabled = match std::env::var("BASTION_SCOUT_IMPORT_ENABLED") {
            Ok(value) if value == "true" => true,
            Ok(value) if value == "false" || value.is_empty() => false,
            Err(std::env::VarError::NotPresent) => false,
            _ => {
                return Err(std::io::Error::other(
                    "BASTION_SCOUT_IMPORT_ENABLED must be true or false",
                ));
            }
        };
        let csv_path = std::env::var_os("BASTION_SCOUT_CSV_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(DEFAULT_SCOUT_CSV));
        if enabled && !csv_path.is_absolute() {
            return Err(std::io::Error::other("Scout CSV path must be absolute"));
        }
        Ok(Self { enabled, csv_path })
    }
}

pub async fn run(service: ProcessService, config: ScoutImportConfig) {
    let mut timer = tokio::time::interval(Duration::from_secs(60));
    timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut last_hash = String::new();
    let mut last_state = None;
    loop {
        timer.tick().await;
        let outcome = tokio::time::timeout(Duration::from_secs(60), async {
            service
                .check_enabled()
                .await
                .map_err(|_| "module_unavailable")?;
            let path = config.csv_path.clone();
            let delivery = tokio::task::spawn_blocking(move || ReadyDelivery::read(&path))
                .await
                .map_err(|_| "reader_failed")?
                .map_err(|e| e.code())?;
            if delivery.sha256() == last_hash {
                return Ok(None);
            }
            let result = service
                .import_local_scout(&delivery)
                .await
                .map_err(|_| "import_failed")?;
            last_hash = delivery.sha256().to_owned();
            Ok(Some(result))
        })
        .await
        .unwrap_or(Err("timeout"));
        match outcome {
            Ok(Some(receipt)) => {
                tracing::info!(
                    import_id = receipt.import_id,
                    inserted = receipt.inserted,
                    updated = receipt.updated,
                    unchanged = receipt.unchanged,
                    stale = receipt.stale,
                    replayed = receipt.replayed,
                    deferred = receipt.deferred,
                    "local Scout delivery confirmed"
                );
                last_state = Some("confirmed");
            }
            Ok(None) => {}
            Err(code) if last_state != Some(code) => {
                tracing::warn!(code, "local Scout delivery pending; will retry");
                last_state = Some(code);
            }
            Err(_) => {}
        }
    }
}
