use std::collections::BTreeSet;

use bastion_core::{CoreAdmin, CoreError, CoreService, DomainActor};
use chrono::{DateTime, FixedOffset, Utc};
use sha2::{Digest, Sha256};
use sqlx::{MySql, QueryBuilder};

use crate::{ShortlistItem, TenderShortlist};

use super::ProcessService;

impl ProcessService {
    pub async fn queue_tender_alerts(
        &self,
        core: &CoreService,
        sender_id: u64,
        recipient_email: &str,
        since: DateTime<Utc>,
        snapshot: &TenderShortlist,
    ) -> Result<usize, CoreError> {
        let now = Utc::now();
        let mut seen = BTreeSet::new();
        let candidates: Vec<_> = snapshot
            .items
            .iter()
            .filter(|item| {
                item.has_interest()
                    && item.first_seen_at >= since
                    && item.first_seen_at <= now
                    && item.fecha_recepcion > now
            })
            .map(|item| (alert_identity(item), item))
            .filter(|(id, _)| seen.insert(id.clone()))
            .collect();
        if candidates.is_empty() {
            return Ok(0);
        }
        // The module lock serializes with other workers and administrative disable.
        let recipient = CoreService::tender_alert_recipient(recipient_email)?;
        let recipient_key = hex::encode(Sha256::digest(recipient.as_bytes()));
        // Sending scheduled alerts retains the existing read permission, but
        // writes deduplication receipts and the outbox: it needs an exclusive lock.
        let mut tx = self
            .begin_with_lock(DomainActor::User(sender_id), "consulta", "ver", true)
            .await?;
        let mut query = QueryBuilder::<MySql>::new(
            "SELECT tender_id FROM procesos_tender_alerts WHERE recipient_key=",
        );
        query.push_bind(&recipient_key).push(" AND tender_id IN (");
        let mut identifiers = query.separated(",");
        for (id, _) in &candidates {
            identifiers.push_bind(id);
        }
        identifiers.push_unseparated(")");
        let queued: BTreeSet<_> = query
            .build_query_scalar::<String>()
            .fetch_all(&mut *tx)
            .await?
            .into_iter()
            .collect();
        let mut count = 0;
        for (id, item) in candidates
            .into_iter()
            .filter(|(id, _)| !queued.contains(id))
            .take(20)
        {
            let (subject, body) = alert_message(item);
            let mail = core
                .enqueue_tender_alert(&mut tx, sender_id, &recipient, &subject, &body)
                .await?;
            sqlx::query("INSERT INTO procesos_tender_alerts (recipient_key,tender_id,mail_outbox_id) VALUES (?,?,?)")
                .bind(&recipient_key).bind(&id).bind(mail).execute(&mut *tx).await?;
            CoreAdmin::record_actor_event(
                &mut tx,
                DomainActor::LocalScout,
                "procesos",
                "tender.alert_queued",
                &item.id,
                serde_json::json!({"sender_user_id": sender_id, "mail_outbox_id": mail}),
            )
            .await?;
            count += 1;
        }
        tx.commit().await?;
        Ok(count)
    }
}

fn limited(value: &str, max: usize) -> String {
    value
        .split(|ch: char| ch.is_whitespace() || ch.is_control())
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(max)
        .collect()
}

fn alert_identity(item: &ShortlistItem) -> String {
    let normalized = |value: &str| limited(value, usize::MAX).to_lowercase();
    hex::encode(Sha256::digest(format!(
        "{}\0{}",
        normalized(&item.entidad),
        normalized(&item.expediente)
    )))
}

fn alert_message(item: &ShortlistItem) -> (String, String) {
    let subject = format!(
        "Nueva licitación {} · {}",
        item.coincidencias_interes,
        limited(&item.expediente, 100)
    );
    let honduras = FixedOffset::west_opt(6 * 3600).expect("valid Honduras offset");
    let deadline = item
        .fecha_recepcion
        .with_timezone(&honduras)
        .format("%d/%m/%Y %H:%M");
    let body = format!(
        "Nueva licitación detectada por Scout\n\nExpediente: {}\nEntidad: {}\nObjeto: {}\nUnidad de compra: {}\nLugar de recepción: {}\nRecepción de ofertas: {deadline} (Honduras)\nFuente publicada: {}\nCoincidencias: {}\n\nEvidencia:\n{}\n\nFicha de HonduCompras:\n{}",
        limited(&item.expediente, 250),
        limited(&item.entidad, 500),
        limited(&item.objeto, 1800),
        limited(&item.unidad_compra, 600),
        limited(&item.lugar_recepcion, 900),
        limited(&item.fuente, 500),
        item.coincidencias_interes,
        limited(&item.evidencia_interes, 1800),
        item.url
    );
    (subject, body)
}
