use super::*;
use bastion_core::{BusinessMail, BusinessMailAttachment, CoreService, DocumentService};
use chrono::{DateTime, Datelike, NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct QuoteEmailInput {
    pub to: String,
    pub cc: String,
    pub bcc: String,
    pub subject: String,
    pub message: String,
    pub scheduled_at: Option<String>,
}
#[derive(Serialize, sqlx::FromRow)]
pub struct QuoteEmailJob {
    pub id: u64,
    pub delivery_status: String,
    pub scheduled_at: NaiveDateTime,
    pub last_error_code: Option<String>,
}
#[derive(Serialize, Deserialize)]
struct Envelope {
    input: QuoteEmailInput,
    object_key: String,
    filename: String,
    sha256: Vec<u8>,
}
#[derive(Clone)]
pub struct QuoteMailService {
    core: CoreService,
    quotes: QuoteService,
}
impl QuoteMailService {
    pub fn new(core: CoreService) -> Self {
        Self {
            quotes: QuoteService::new(core.database(), core.admin()),
            core,
        }
    }
    pub async fn queue(
        &self,
        actor: u64,
        id: u64,
        expected: u64,
        key: &str,
        input: QuoteEmailInput,
    ) -> Result<QuoteEmailJob, CoreError> {
        self.quotes.check_access(actor, "editar").await?;
        validate(&input)?;
        if !(16..=128).contains(&key.len())
            || !key
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
        {
            return Err(CoreError::Invalid);
        }
        let hash = Sha256::digest(
            serde_json::to_vec(&(id, expected, &input)).map_err(|_| CoreError::Invalid)?,
        )
        .to_vec();
        let key_hash = Sha256::digest(key.as_bytes()).to_vec();
        if let Some((job, previous)) = self.receipt(actor, &key_hash).await? {
            if previous != hash {
                return Err(CoreError::Conflict);
            }
            return self.job(actor, id, job).await;
        }
        if !self.core.business_mail_configured(actor).await? {
            return Err(CoreError::Invalid);
        }
        let current = self.quotes.detail(actor, id).await?;
        if current.version != expected {
            return Err(CoreError::PreconditionFailed);
        }
        let mut input = input;
        let reference = if current.data.order_number.trim().is_empty() {
            current.quote_number.as_deref().unwrap_or("")
        } else {
            current.data.order_number.trim()
        };
        if input.subject.trim().is_empty() {
            input.subject = format!("Cotización {reference}");
        }
        if input.message.trim().is_empty() {
            input.message = format!(
                "Estimados {}\n\nAdjunto la cotización correspondiente al proceso {reference}",
                if current.data.client_name.trim().is_empty() {
                    "Cliente"
                } else {
                    current.data.client_name.trim()
                }
            );
        }
        let document = self.quotes.pdf(actor, id, expected).await?;
        let raw = base64::Engine::decode(
            &base64::engine::general_purpose::STANDARD,
            &document.content_base64,
        )
        .map_err(|_| CoreError::Invalid)?;
        let envelope = Envelope {
            input: input.clone(),
            object_key: document.object_key,
            filename: document.filename,
            sha256: Sha256::digest(raw).to_vec(),
        };
        let encrypted = self
            .core
            .seal_domain_json(&serde_json::to_value(envelope).map_err(|_| CoreError::Invalid)?)?;
        let scheduled_at = schedule(input.scheduled_at.as_deref())?;
        let mut tx = self.quotes.begin(actor, "editar").await?;
        let current = load(&mut tx, id).await?;
        if current.version != expected {
            return Err(CoreError::PreconditionFailed);
        }
        let previous: Option<(u64, Vec<u8>)> = sqlx::query_as(
            "SELECT id,request_hash FROM cotizaciones_email_jobs WHERE actor_id=? AND key_hash=?",
        )
        .bind(actor)
        .bind(&key_hash)
        .fetch_optional(&mut *tx)
        .await?;
        if let Some((job, previous)) = previous {
            if previous != hash {
                return Err(CoreError::Conflict);
            }
            tx.commit().await?;
            return self.job(actor, id, job).await;
        }
        let job=sqlx::query("INSERT INTO cotizaciones_email_jobs (quote_id,quote_version,actor_id,key_hash,request_hash,payload_ciphertext,scheduled_at,created_at) VALUES (?,?,?,?,?,?,?,UTC_TIMESTAMP(6))")
            .bind(id).bind(expected).bind(actor).bind(key_hash).bind(hash).bind(encrypted).bind(scheduled_at).execute(&mut *tx).await?.last_insert_id();
        CoreAdmin::record_domain_event(
            &mut tx,
            actor,
            "cotizaciones",
            "quote.email.queued",
            &id.to_string(),
            serde_json::json!({"job_id":job,"version":expected,"scheduled_at":scheduled_at}),
        )
        .await?;
        tx.commit().await?;
        self.job(actor, id, job).await
    }
    async fn receipt(&self, actor: u64, key: &[u8]) -> Result<Option<(u64, Vec<u8>)>, CoreError> {
        Ok(sqlx::query_as(
            "SELECT id,request_hash FROM cotizaciones_email_jobs WHERE actor_id=? AND key_hash=?",
        )
        .bind(actor)
        .bind(key)
        .fetch_optional(self.core.database().pool())
        .await?)
    }
    pub async fn job(&self, actor: u64, quote: u64, job: u64) -> Result<QuoteEmailJob, CoreError> {
        self.quotes.check_access(actor, "ver").await?;
        sqlx::query_as("SELECT id,delivery_status,scheduled_at,last_error_code FROM cotizaciones_email_jobs WHERE id=? AND quote_id=?").bind(job).bind(quote).fetch_optional(self.core.database().pool()).await?.ok_or(CoreError::NotFound)
    }
    /// Claims one scheduled snapshot; uncertain SMTP results are never retried automatically.
    pub async fn deliver_one(&self) -> Result<bool, CoreError> {
        let database = self.core.database();
        sqlx::query("UPDATE cotizaciones_email_jobs SET delivery_status='failed',last_error_code='delivery_unconfirmed',completed_at=UTC_TIMESTAMP(6) WHERE delivery_status='delivering' AND claimed_at<UTC_TIMESTAMP(6)-INTERVAL 2 MINUTE").execute(database.pool()).await?;
        let mut tx = database.pool().begin().await?;
        let row:Option<(u64,Option<u64>,u64,u64,Vec<u8>)>=sqlx::query_as("SELECT id,quote_id,quote_version,actor_id,payload_ciphertext FROM cotizaciones_email_jobs WHERE delivery_status='pending' AND scheduled_at<=UTC_TIMESTAMP(6) ORDER BY scheduled_at,id LIMIT 1 FOR UPDATE").fetch_optional(&mut *tx).await?;
        let Some((job, quote, version, actor, payload)) = row else {
            tx.commit().await?;
            return Ok(false);
        };
        sqlx::query("UPDATE cotizaciones_email_jobs SET delivery_status='delivering',claimed_at=UTC_TIMESTAMP(6) WHERE id=?").bind(job).execute(&mut *tx).await?;
        tx.commit().await?;
        let sent = async {
            let id = quote.ok_or(CoreError::NotFound)?;
            self.quotes.check_access(actor, "editar").await?;
            let current = self.quotes.detail(actor, id).await?;
            if current.version != version {
                return Err(CoreError::PreconditionFailed);
            }
            let envelope: Envelope =
                serde_json::from_value(self.core.unseal_domain_json(&payload)?)
                    .map_err(|_| CoreError::Invalid)?;
            validate(&envelope.input)?;
            let bytes = DocumentService::from_environment()
                .get(envelope.object_key)
                .await?;
            if Sha256::digest(&bytes).as_slice() != envelope.sha256.as_slice() {
                return Err(CoreError::Invalid);
            }
            let (to, cc, bcc) = recipients(&envelope.input)?;
            self.quotes.check_access(actor, "editar").await?;
            self.core
                .send_business_mail(
                    actor,
                    BusinessMail {
                        to,
                        cc,
                        bcc,
                        subject: envelope.input.subject,
                        message: envelope.input.message,
                        attachments: vec![BusinessMailAttachment {
                            filename: envelope.filename,
                            mime: "application/pdf".into(),
                            bytes,
                        }],
                    },
                )
                .await?;
            Ok::<(), CoreError>(())
        }
        .await;
        match sent {
            Ok(()) => {
                let mut tx = database.pool().begin().await?;
                // Only this exact revision was sent; preserve edits made during SMTP IO.
                if let Some(id) = quote {
                    let current = load(&mut tx, id).await?;
                    if current.version == version {
                        sqlx::query("UPDATE cotizaciones_drafts SET data_json=JSON_SET(data_json,'$.status','Enviada'),calculation_json=JSON_SET(calculation_json,'$.data.status','Enviada'),version=version+1,updated_by=?,updated_at=UTC_TIMESTAMP(6) WHERE id=? AND version=?").bind(actor).bind(id).bind(version).execute(&mut *tx).await?;
                    }
                    CoreAdmin::record_domain_event(
                        &mut tx,
                        actor,
                        "cotizaciones",
                        "quote.email.sent",
                        &id.to_string(),
                        serde_json::json!({"job_id":job,"version":version}),
                    )
                    .await?;
                }
                sqlx::query("UPDATE cotizaciones_email_jobs SET delivery_status='sent',payload_ciphertext='',completed_at=UTC_TIMESTAMP(6),last_error_code=NULL WHERE id=? AND delivery_status='delivering'").bind(job).execute(&mut *tx).await?;
                tx.commit().await?;
            }
            Err(error) => {
                let code = match error {
                    CoreError::PreconditionFailed => "quote_changed",
                    CoreError::NotFound | CoreError::Forbidden | CoreError::Unauthorized => {
                        "authorization_changed"
                    }
                    CoreError::Invalid => "configuration_invalid",
                    _ => "delivery_unconfirmed",
                };
                let mut tx = database.pool().begin().await?;
                sqlx::query("UPDATE cotizaciones_email_jobs SET delivery_status='failed',last_error_code=?,completed_at=UTC_TIMESTAMP(6) WHERE id=? AND delivery_status='delivering'").bind(code).bind(job).execute(&mut *tx).await?;
                if let Some(id) = quote {
                    CoreAdmin::record_domain_event(
                        &mut tx,
                        actor,
                        "cotizaciones",
                        "quote.email.failed",
                        &id.to_string(),
                        serde_json::json!({"job_id":job,"error_code":code}),
                    )
                    .await?;
                }
                tx.commit().await?;
            }
        }
        Ok(true)
    }
}
fn schedule(value: Option<&str>) -> Result<NaiveDateTime, CoreError> {
    let now = Utc::now().naive_utc();
    let Some(value) = value.filter(|value| !value.is_empty()) else {
        return Ok(now);
    };
    let time = DateTime::parse_from_rfc3339(value)
        .map_err(|_| CoreError::Invalid)?
        .with_timezone(&Utc);
    if !(1000..=9999).contains(&time.year()) {
        return Err(CoreError::Invalid);
    }
    Ok(time.naive_utc().max(now))
}
fn validate(input: &QuoteEmailInput) -> Result<(), CoreError> {
    if input.to.len() > 255
        || input.cc.len() > 1000
        || input.bcc.len() > 1000
        || input.subject.chars().count() > 255
        || input.subject.contains(['\r', '\n'])
        || input.message.chars().count() > 5000
    {
        return Err(CoreError::Invalid);
    }
    recipients(input)?;
    schedule(input.scheduled_at.as_deref())?;
    Ok(())
}
fn recipients(
    input: &QuoteEmailInput,
) -> Result<(Vec<String>, Vec<String>, Vec<String>), CoreError> {
    let mut seen = std::collections::BTreeSet::new();
    let mut parse = |value: &str| -> Result<Vec<String>, CoreError> {
        let mut result = Vec::new();
        for address in value
            .split(|c: char| c.is_whitespace() || c == ',' || c == ';')
            .filter(|value| !value.is_empty())
        {
            if address.len() > 254 || address.parse::<lettre::Address>().is_err() {
                return Err(CoreError::Invalid);
            }
            if seen.insert(address.to_lowercase()) {
                result.push(address.to_owned());
            }
        }
        Ok(result)
    };
    let (to, cc, bcc) = (parse(&input.to)?, parse(&input.cc)?, parse(&input.bcc)?);
    if seen.is_empty() || seen.len() > 50 {
        return Err(CoreError::Invalid);
    }
    Ok((to, cc, bcc))
}
#[cfg(test)]
mod tests {
    use super::*;
    fn input() -> QuoteEmailInput {
        QuoteEmailInput {
            to: "to@example.test".into(),
            cc: "copy@example.test; TO@example.test".into(),
            bcc: "private@example.test".into(),
            subject: "Cotización COT-1".into(),
            message: "Texto".into(),
            scheduled_at: None,
        }
    }
    #[test]
    fn recipients_are_validated_and_deduplicated_across_all_fields() {
        let (to, cc, bcc) = recipients(&input()).unwrap();
        assert_eq!(to.len(), 1);
        assert_eq!(cc, ["copy@example.test"]);
        assert_eq!(bcc, ["private@example.test"]);
        let mut value = input();
        value.cc = "bad-address".into();
        assert!(validate(&value).is_err());
        let mut value = input();
        value.subject = "Cotización\r\nBcc: other@example.test".into();
        assert!(validate(&value).is_err());
    }
    #[test]
    fn schedule_requires_an_explicit_timezone_and_uses_utc() {
        assert_eq!(
            schedule(Some("2099-10-22T13:00:00-06:00"))
                .unwrap()
                .to_string(),
            "2099-10-22 19:00:00"
        );
        assert!(schedule(Some("2099-10-22T13:00:00")).is_err());
    }
}
