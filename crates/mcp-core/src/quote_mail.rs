use std::collections::BTreeSet;

use bastion_core::{CoreError, CoreService, oauth_session_in_transaction};
use bastion_cotizaciones::{QuoteEmailInput, QuoteMailService, QuoteService};
use bastion_mcp::{AuthenticatedAccess, Credential, CredentialKind, ExchangePolicy};
use chrono::{DateTime, NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::{MySql, Transaction};

use crate::{CoreScopePolicy, approved_read_requirements};

#[derive(Clone)]
pub struct QuoteMailConfirmation {
    core: CoreService,
    frontend: String,
}

#[derive(Serialize, Deserialize)]
struct Snapshot {
    input: QuoteEmailInput,
    quote_number: Option<String>,
    client_name: String,
    attachments: Vec<String>,
}

#[derive(sqlx::FromRow)]
struct Pending {
    id: u64,
    user_id: u64,
    grant_id: u64,
    quote_id: u64,
    quote_version: u64,
    payload_ciphertext: Vec<u8>,
    expires_at: NaiveDateTime,
    confirmed_at: Option<NaiveDateTime>,
    denied_at: Option<NaiveDateTime>,
    job_id: Option<u64>,
}

impl QuoteMailConfirmation {
    /// Frontend origin is validated by the HTTP composition root.
    pub fn new(core: CoreService, frontend: String) -> Self {
        Self { core, frontend }
    }

    async fn authorize(
        &self,
        tx: &mut Transaction<'_, MySql>,
        user: u64,
        grant: u64,
    ) -> Result<(), CoreError> {
        let client: Option<u64> = sqlx::query_scalar(
            "SELECT g.client_id FROM oauth_grants g JOIN oauth_clients c ON c.id=g.client_id +             WHERE g.id=? AND g.user_id=? AND g.revoked_at IS NULL AND g.expires_at>UTC_TIMESTAMP(6) +             AND c.enabled=1 AND c.metadata_hash=c.approved_metadata_hash +             AND JSON_CONTAINS(g.scopes_json,JSON_QUOTE('cotizaciones:write'))=1 +             AND JSON_CONTAINS(g.scopes_json,JSON_QUOTE('cotizaciones:read'))=1 +             AND EXISTS (SELECT 1 FROM oauth_client_scopes s WHERE s.client_id=c.id AND s.scope_name='cotizaciones:write') +             AND EXISTS (SELECT 1 FROM oauth_client_scopes s WHERE s.client_id=c.id AND s.scope_name='cotizaciones:read') FOR UPDATE",
        )
        .bind(grant).bind(user).fetch_optional(&mut **tx).await?;
        let client = client.ok_or(CoreError::Forbidden)?;
        let policy = CoreScopePolicy::new(self.core.admin(), approved_read_requirements())
            .map_err(|_| CoreError::Invalid)?;
        let scopes = BTreeSet::from(["cotizaciones:read".into(), "cotizaciones:write".into()]);
        if !policy.authorize(tx, user, client, &scopes).await? {
            return Err(CoreError::Forbidden);
        }
        Ok(())
    }

    pub async fn prepare(
        &self,
        access: &AuthenticatedAccess,
        quote_id: u64,
        version: u64,
        input: QuoteEmailInput,
    ) -> Result<Value, CoreError> {
        if !access.scopes.contains("cotizaciones:write")
            || !access.scopes.contains("cotizaciones:read")
        {
            return Err(CoreError::Forbidden);
        }
        future_schedule(&input)?;
        let quotes = QuoteService::new(self.core.database(), self.core.admin());
        quotes.consume_request(access.user_id).await?;
        let input = QuoteMailService::new(self.core.clone())
            .prepare_input(access.user_id, quote_id, version, input)
            .await?;
        let quote = quotes.detail(access.user_id, quote_id).await?;
        let files = quotes.attachments(access.user_id, quote_id, 1).await?;
        if quote.version != version || files.quote_version != version {
            return Err(CoreError::PreconditionFailed);
        }
        if files.has_more || files.items.len() > 10 {
            return Err(CoreError::Invalid);
        }
        let snapshot = Snapshot {
            input,
            quote_number: quote.quote_number,
            client_name: quote.data.client_name,
            attachments: files.items.into_iter().map(|file| file.filename).collect(),
        };
        let encrypted = self
            .core
            .seal_domain_json(&serde_json::to_value(snapshot).map_err(|_| CoreError::Invalid)?)?;
        let credential = Credential::generate().map_err(|_| CoreError::Overloaded)?;
        let handle = credential.expose();
        let mut tx = self.core.database().pool().begin().await?;
        self.authorize(&mut tx, access.user_id, access.grant_id)
            .await?;
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM mcp_quote_email_confirmations WHERE user_id=? AND expires_at>UTC_TIMESTAMP(6) AND denied_at IS NULL AND job_id IS NULL")
            .bind(access.user_id).fetch_one(&mut *tx).await?;
        if count >= 10 {
            return Err(CoreError::Overloaded);
        }
        sqlx::query("INSERT INTO mcp_quote_email_confirmations (handle_hash,user_id,grant_id,quote_id,quote_version,payload_ciphertext,created_at,expires_at) VALUES (?,?,?,?,?,?,UTC_TIMESTAMP(6),UTC_TIMESTAMP(6)+INTERVAL 15 MINUTE)")
            .bind(credential.digest(CredentialKind::ConsentRequest).as_slice()).bind(access.user_id).bind(access.grant_id)
            .bind(quote_id).bind(version).bind(encrypted).execute(&mut *tx).await?;
        Self::audit(
            &mut tx,
            access.user_id,
            quote_id,
            "quote.email.confirmation.requested",
        )
        .await?;
        tx.commit().await?;
        Ok(json!({
            "status":"awaiting_user_confirmation",
            "handle":handle,
            "confirmation_url":format!("{}/mcp/quote-email#{}",self.frontend,handle),
            "expires_in_seconds":900,
            "message":"El usuario debe abrir el enlace, revisar el envío y confirmarlo en Bastion. Esta solicitud todavía no encola correo."
        }))
    }

    async fn load(
        &self,
        tx: &mut Transaction<'_, MySql>,
        handle: &str,
    ) -> Result<Pending, CoreError> {
        let credential = Credential::parse(handle).map_err(|_| CoreError::Invalid)?;
        sqlx::query_as("SELECT id,user_id,grant_id,quote_id,quote_version,payload_ciphertext,expires_at,confirmed_at,denied_at,job_id FROM mcp_quote_email_confirmations WHERE handle_hash=? FOR UPDATE")
            .bind(credential.digest(CredentialKind::ConsentRequest).as_slice())
            .fetch_optional(&mut **tx).await?.ok_or(CoreError::NotFound)
    }

    async fn recover_job(
        &self,
        tx: &mut Transaction<'_, MySql>,
        pending: &mut Pending,
    ) -> Result<(), CoreError> {
        if pending.confirmed_at.is_some() && pending.job_id.is_none() {
            let key = format!("mcp-quote-email-confirmation-{}", pending.id);
            let job: Option<u64> = sqlx::query_scalar(
                "SELECT id FROM cotizaciones_email_jobs WHERE actor_id=? AND key_hash=UNHEX(SHA2(?,256))",
            ).bind(pending.user_id).bind(key).fetch_optional(&mut **tx).await?;
            if let Some(job) = job {
                sqlx::query("UPDATE mcp_quote_email_confirmations SET job_id=? WHERE id=?")
                    .bind(job)
                    .bind(pending.id)
                    .execute(&mut **tx)
                    .await?;
                pending.job_id = Some(job);
            }
        }
        Ok(())
    }

    pub async fn status(
        &self,
        access: &AuthenticatedAccess,
        handle: &str,
    ) -> Result<Value, CoreError> {
        if !access.scopes.contains("cotizaciones:write")
            || !access.scopes.contains("cotizaciones:read")
        {
            return Err(CoreError::Forbidden);
        }
        let mut tx = self.core.database().pool().begin().await?;
        let mut pending = self.load(&mut tx, handle).await?;
        if pending.user_id != access.user_id || pending.grant_id != access.grant_id {
            return Err(CoreError::Forbidden);
        }
        self.authorize(&mut tx, pending.user_id, pending.grant_id)
            .await?;
        self.recover_job(&mut tx, &mut pending).await?;
        tx.commit().await?;
        if let Some(job) = pending.job_id {
            let job = QuoteMailService::new(self.core.clone())
                .job(access.user_id, pending.quote_id, job)
                .await?;
            return Ok(json!({"status":"queued","job":job}));
        }
        Ok(json!({"status":state(&pending)}))
    }

    /// Only a CSRF-protected authenticated browser session reaches this method.
    pub async fn review(&self, session: &str, handle: &str) -> Result<Value, CoreError> {
        let mut tx = self.core.database().pool().begin().await?;
        let actor = oauth_session_in_transaction(&mut tx, session)
            .await?
            .ok_or(CoreError::Unauthorized)?;
        let mut pending = self.load(&mut tx, handle).await?;
        if pending.user_id != actor.user_id {
            return Err(CoreError::Forbidden);
        }
        self.authorize(&mut tx, actor.user_id, pending.grant_id)
            .await?;
        self.recover_job(&mut tx, &mut pending).await?;
        if pending.expires_at <= Utc::now().naive_utc() && pending.job_id.is_none() {
            return Err(CoreError::Conflict);
        }
        let snapshot: Snapshot =
            serde_json::from_value(self.core.unseal_domain_json(&pending.payload_ciphertext)?)
                .map_err(|_| CoreError::Invalid)?;
        tx.commit().await?;
        Ok(
            json!({"status":state(&pending),"quote_id":pending.quote_id,"version":pending.quote_version,
            "quote_number":snapshot.quote_number,"client_name":snapshot.client_name,
            "input":snapshot.input,"attachments":snapshot.attachments,
            "expires_at":pending.expires_at.and_utc().to_rfc3339(),"job_id":pending.job_id}),
        )
    }

    pub async fn decide(
        &self,
        session: &str,
        handle: &str,
        approve: bool,
    ) -> Result<Value, CoreError> {
        let mut tx = self.core.database().pool().begin().await?;
        let actor = oauth_session_in_transaction(&mut tx, session)
            .await?
            .ok_or(CoreError::Unauthorized)?;
        let mut pending = self.load(&mut tx, handle).await?;
        if pending.user_id != actor.user_id {
            return Err(CoreError::Forbidden);
        }
        self.authorize(&mut tx, actor.user_id, pending.grant_id)
            .await?;
        self.recover_job(&mut tx, &mut pending).await?;
        if let Some(job) = pending.job_id {
            tx.commit().await?;
            return Ok(json!({"status":"queued","job_id":job}));
        }
        if pending.denied_at.is_some() {
            return Err(CoreError::Conflict);
        }
        if pending.expires_at <= Utc::now().naive_utc() {
            return Err(CoreError::Conflict);
        }
        if !approve {
            if pending.confirmed_at.is_some() {
                return Err(CoreError::Conflict);
            }
            sqlx::query(
                "UPDATE mcp_quote_email_confirmations SET denied_at=UTC_TIMESTAMP(6) WHERE id=?",
            )
            .bind(pending.id)
            .execute(&mut *tx)
            .await?;
            Self::audit(
                &mut tx,
                actor.user_id,
                pending.quote_id,
                "quote.email.confirmation.denied",
            )
            .await?;
            tx.commit().await?;
            return Ok(json!({"status":"denied"}));
        }
        let snapshot: Snapshot =
            serde_json::from_value(self.core.unseal_domain_json(&pending.payload_ciphertext)?)
                .map_err(|_| CoreError::Invalid)?;
        future_schedule(&snapshot.input)?;
        sqlx::query("UPDATE mcp_quote_email_confirmations SET confirmed_at=COALESCE(confirmed_at,UTC_TIMESTAMP(6)) WHERE id=?")
            .bind(pending.id).execute(&mut *tx).await?;
        if pending.confirmed_at.is_none() {
            Self::audit(
                &mut tx,
                actor.user_id,
                pending.quote_id,
                "quote.email.confirmation.approved",
            )
            .await?;
        }
        tx.commit().await?;
        // Durable approval precedes side effects. A stable idempotency key makes
        // retries and concurrent browser submissions refer to the same mail job.
        let job = QuoteMailService::new(self.core.clone())
            .queue(
                actor.user_id,
                pending.quote_id,
                pending.quote_version,
                &format!("mcp-quote-email-confirmation-{}", pending.id),
                snapshot.input,
            )
            .await?;
        sqlx::query("UPDATE mcp_quote_email_confirmations SET job_id=? WHERE id=? AND confirmed_at IS NOT NULL AND denied_at IS NULL")
            .bind(job.id).bind(pending.id).execute(self.core.database().pool()).await?;
        Ok(json!({"status":"queued","job_id":job.id}))
    }

    async fn audit(
        tx: &mut Transaction<'_, MySql>,
        actor: u64,
        quote: u64,
        action: &str,
    ) -> Result<(), CoreError> {
        bastion_core::CoreAdmin::record_domain_event(
            tx,
            actor,
            "cotizaciones",
            action,
            &quote.to_string(),
            json!({}),
        )
        .await
    }
}

fn future_schedule(input: &QuoteEmailInput) -> Result<(), CoreError> {
    let date = input.scheduled_at.as_deref().ok_or(CoreError::Invalid)?;
    let date = DateTime::parse_from_rfc3339(date).map_err(|_| CoreError::Invalid)?;
    if date <= Utc::now() {
        return Err(CoreError::Invalid);
    }
    Ok(())
}

fn state(pending: &Pending) -> &'static str {
    if pending.job_id.is_some() {
        "queued"
    } else if pending.denied_at.is_some() {
        "denied"
    } else if pending.expires_at <= Utc::now().naive_utc() {
        "expired"
    } else if pending.confirmed_at.is_some() {
        "confirmed"
    } else {
        "awaiting_user_confirmation"
    }
}
