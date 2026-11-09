use bastion_core::CoreService;
use bastion_procesos::{
    DEFAULT_SCOUT_CSV, ProcessService, ReadyDelivery, ReportAvailability, TenderShortlist,
};
use chrono::{DateTime, Utc};
use std::{path::PathBuf, time::Duration};

pub struct ScoutImportConfig {
    pub enabled: bool,
    pub csv_path: PathBuf,
}

pub struct TenderAlertConfig {
    sender_id: u64,
    recipient_email: String,
    since: DateTime<Utc>,
}

impl TenderAlertConfig {
    pub fn from_env() -> std::io::Result<Option<Self>> {
        let sender = std::env::var("BASTION_TENDER_ALERT_SENDER_USER_ID").unwrap_or_default();
        let recipient_email =
            std::env::var("BASTION_TENDER_ALERT_RECIPIENT_EMAIL").unwrap_or_default();
        let since = std::env::var("BASTION_TENDER_ALERT_SINCE").unwrap_or_default();
        if sender.is_empty() && recipient_email.is_empty() && since.is_empty() {
            return Ok(None);
        }
        let sender_id = sender
            .parse::<u64>()
            .ok()
            .filter(|id| *id > 0)
            .ok_or_else(|| {
                std::io::Error::other("BASTION_TENDER_ALERT_SENDER_USER_ID must be a user ID")
            })?;
        let recipient_email =
            CoreService::tender_alert_recipient(&recipient_email).map_err(|_| {
                std::io::Error::other(
                    "BASTION_TENDER_ALERT_RECIPIENT_EMAIL must be an email address",
                )
            })?;
        let since = since
            .parse()
            .map_err(|_| std::io::Error::other("BASTION_TENDER_ALERT_SINCE must be RFC3339"))?;
        Ok(Some(Self {
            sender_id,
            recipient_email,
            since,
        }))
    }
}

pub async fn run_tender_alerts(
    service: ProcessService,
    core: CoreService,
    csv_path: PathBuf,
    config: TenderAlertConfig,
) {
    let mut timer = tokio::time::interval(Duration::from_secs(60));
    timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut last_state = None;
    loop {
        timer.tick().await;
        let path = csv_path.clone();
        let result = tokio::time::timeout(Duration::from_secs(55), async {
            service
                .check_enabled()
                .await
                .map_err(|_| "module_unavailable")?;
            let feed = tokio::task::spawn_blocking(move || TenderShortlist::read(&path))
                .await
                .map_err(|_| "reader_failed")?;
            if feed.availability != ReportAvailability::Available {
                return Err("feed_unavailable");
            }
            service
                .queue_tender_alerts(
                    &core,
                    config.sender_id,
                    &config.recipient_email,
                    config.since,
                    &feed,
                )
                .await
                .map_err(|_| "enqueue_failed")
        })
        .await
        .unwrap_or(Err("timeout"));
        match result {
            Ok(0) => last_state = None,
            Ok(count) => {
                tracing::info!(count, "tender alert messages queued");
                last_state = None;
            }
            Err(code) if last_state != Some(code) => {
                tracing::warn!(code, "tender alerts pending; will retry");
                last_state = Some(code);
            }
            Err(_) => {}
        }
    }
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
