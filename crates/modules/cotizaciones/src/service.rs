use bastion_core::{CoreAdmin, CoreError, Database};
use sha2::{Digest, Sha256};
use sqlx::{MySql, QueryBuilder, Transaction};

#[path = "documents.rs"]
mod documents;
pub use documents::{QuoteDocument, QuoteDocumentReadiness};
#[path = "mail.rs"]
mod mail;
pub use mail::{QuoteEmailInput, QuoteEmailJob, QuoteMailService};

use crate::{
    QuoteCalculation, QuoteDraft, QuoteFilter, QuoteInput, QuotePage, QuoteStatus, QuoteSummary,
    calculate,
};

#[derive(Clone)]
pub struct QuoteService {
    database: Database,
    admin: CoreAdmin,
}

impl QuoteService {
    pub fn new(database: Database, admin: CoreAdmin) -> Self {
        Self { database, admin }
    }

    pub async fn migrate(database: &Database) -> Result<(), CoreError> {
        let mut migrator = sqlx::migrate!("./migrations");
        migrator.dangerous_set_table_name("_cotizaciones_migrations");
        migrator
            .run(database.pool())
            .await
            .map_err(CoreError::Migration)
    }

    pub async fn synchronize_release(&self, allowed: bool) -> Result<(), CoreError> {
        let exists: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM information_schema.tables WHERE table_schema=DATABASE() AND table_name='_cotizaciones_migrations'")
            .fetch_one(self.database.pool()).await?;
        let ready = if exists > 0 {
            let applied = sqlx::query_as::<_, (i64, bool, Vec<u8>)>(
                "SELECT version,success,checksum FROM _cotizaciones_migrations",
            )
            .fetch_all(self.database.pool())
            .await?;
            let expected = sqlx::migrate!("./migrations");
            applied.len() == expected.iter().count()
                && expected.iter().all(|m| {
                    applied.iter().any(|(v, ok, hash)| {
                        *v == m.version && *ok && hash.as_slice() == m.checksum.as_ref()
                    })
                })
        } else {
            false
        };
        sqlx::query("INSERT INTO module_registry (module_id,display_name,included_in_release,environment_allowed,enabled) VALUES ('cotizaciones','Cotizaciones',1,?,0) ON DUPLICATE KEY UPDATE included_in_release=1,environment_allowed=VALUES(environment_allowed)")
            .bind(allowed && ready).execute(self.database.pool()).await?;
        Ok(())
    }

    async fn begin(
        &self,
        actor: u64,
        capability: &str,
    ) -> Result<Transaction<'_, MySql>, CoreError> {
        self.admin
            .authorize(actor, "cotizaciones", "documentos", capability)
            .await?;
        let mut tx = self.database.pool().begin().await?;
        // Same lock used by Core's administrative module toggle.
        let enabled: Option<bool> = sqlx::query_scalar("SELECT (included_in_release AND environment_allowed AND enabled) FROM module_registry WHERE module_id='cotizaciones' FOR UPDATE")
            .fetch_optional(&mut *tx).await?;
        if enabled != Some(true) {
            return Err(CoreError::NotFound);
        }
        Ok(tx)
    }

    pub async fn check_access(&self, actor: u64, capability: &str) -> Result<(), CoreError> {
        self.begin(actor, capability).await?.commit().await?;
        Ok(())
    }

    pub async fn consume_request(&self, actor: u64) -> Result<(), CoreError> {
        let mut tx = self.database.pool().begin().await?;
        sqlx::query("INSERT INTO cotizaciones_actor_limits (actor_id,window_start,requests) VALUES (?,UTC_TIMESTAMP(6),1) ON DUPLICATE KEY UPDATE requests=IF(window_start<=UTC_TIMESTAMP(6)-INTERVAL 1 MINUTE,1,requests+1),window_start=IF(window_start<=UTC_TIMESTAMP(6)-INTERVAL 1 MINUTE,UTC_TIMESTAMP(6),window_start)")
            .bind(actor).execute(&mut *tx).await?;
        let count: u32 =
            sqlx::query_scalar("SELECT requests FROM cotizaciones_actor_limits WHERE actor_id=?")
                .bind(actor)
                .fetch_one(&mut *tx)
                .await?;
        tx.commit().await?;
        if count > 120 {
            return Err(CoreError::Locked);
        }
        Ok(())
    }

    pub async fn list(&self, actor: u64, filter: &QuoteFilter) -> Result<QuotePage, CoreError> {
        let (page, page_size) = filter.bounds()?;
        let mut tx = self.begin(actor, "ver").await?;
        let mut query = QueryBuilder::<MySql>::new(
            "SELECT id,version,client_name,order_number,quote_number,JSON_UNQUOTE(JSON_EXTRACT(data_json,'$.quote_date')) AS quote_date,CAST(JSON_LENGTH(data_json,'$.items') AS UNSIGNED) AS item_count,COALESCE(JSON_UNQUOTE(JSON_EXTRACT(data_json,'$.status')),'Borrador') AS status,JSON_UNQUOTE(JSON_EXTRACT(calculation_json,'$.valid_until')) AS valid_until,JSON_UNQUOTE(JSON_EXTRACT(calculation_json,'$.totals.total')) AS total FROM cotizaciones_drafts WHERE 1=1",
        );
        if let Some(q) = filter.q.as_deref().map(str::trim).filter(|q| !q.is_empty()) {
            // LOCATE treats wildcard characters literally and all values are bound.
            query
                .push(" AND (LOCATE(")
                .push_bind(q)
                .push(",client_name)>0 OR LOCATE(")
                .push_bind(q)
                .push(",order_number)>0 OR LOCATE(")
                .push_bind(q)
                .push(",quote_number)>0 OR LOCATE(")
                .push_bind(q)
                .push(",JSON_UNQUOTE(JSON_EXTRACT(data_json,'$.status')))>0 OR LOCATE(")
                .push_bind(q)
                .push(",JSON_UNQUOTE(JSON_EXTRACT(data_json,'$.items[*].description')))>0)");
        }
        query
            .push(" ORDER BY quote_date DESC,id DESC LIMIT ")
            .push_bind(page_size + 1)
            .push(" OFFSET ")
            .push_bind((page - 1) * page_size);
        let mut rows: Vec<QuoteSummary> = query.build_query_as().fetch_all(&mut *tx).await?;
        let has_more = rows.len() > page_size as usize;
        rows.truncate(page_size as usize);
        tx.commit().await?;
        Ok(QuotePage {
            items: rows,
            page,
            page_size,
            has_more,
        })
    }

    pub async fn preview(
        &self,
        actor: u64,
        input: &QuoteInput,
    ) -> Result<QuoteCalculation, CoreError> {
        let tx = self.begin(actor, "editar").await?;
        let result = calculate(input)?;
        tx.commit().await?;
        Ok(result)
    }

    pub async fn detail(&self, actor: u64, id: u64) -> Result<QuoteDraft, CoreError> {
        let mut tx = self.begin(actor, "ver").await?;
        let result = load(&mut tx, id).await?;
        tx.commit().await?;
        Ok(result)
    }

    async fn resolve_client(
        &self,
        tx: &mut Transaction<'_, MySql>,
        actor: u64,
        input: &mut QuoteInput,
        current: Option<&QuoteInput>,
    ) -> Result<(), CoreError> {
        if let Some(id) = input.id_client {
            if let Some(previous) = current.filter(|previous| previous.id_client == Some(id)) {
                // Retain the saved document identity when only other fields change.
                input.client_name.clone_from(&previous.client_name);
                input.client_rtn.clone_from(&previous.client_rtn);
                input.client_email.clone_from(&previous.client_email);
            } else {
                let client =
                    bastion_clientes::ClientService::new(self.database.clone(), self.admin.clone())
                        .document_snapshot(actor, id, tx)
                        .await?;
                input.client_name = client.cliente_name;
                input.client_rtn = client.rtn;
                input.client_email = client.email;
            }
        } else {
            input.client_rtn.clear();
            input.client_email.clear();
        }
        Ok(())
    }

    pub async fn create(
        &self,
        actor: u64,
        key: &str,
        input: QuoteInput,
    ) -> Result<QuoteDraft, CoreError> {
        input.validate()?;
        if !(16..=128).contains(&key.len())
            || !key
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
        {
            return Err(CoreError::Invalid);
        }
        let payload = serde_json::to_vec(&input).map_err(|_| CoreError::Invalid)?;
        let request_hash = Sha256::digest(&payload).to_vec();
        let key_hash = Sha256::digest(key.as_bytes()).to_vec();
        let (name, email) = self.admin.user_contact(actor).await?;
        let mut tx = self.begin(actor, "editar").await?;
        // The module lock serializes creators, including retries with the same key.
        let previous: Option<(Vec<u8>, serde_json::Value)> = sqlx::query_as("SELECT request_hash,response_json FROM cotizaciones_create_receipts WHERE actor_id=? AND key_hash=?")
            .bind(actor).bind(&key_hash).fetch_optional(&mut *tx).await?;
        if let Some((hash, response)) = previous {
            if hash != request_hash {
                return Err(CoreError::Conflict);
            }
            let result = serde_json::from_value(response).map_err(|_| CoreError::Invalid)?;
            tx.commit().await?;
            return Ok(result);
        }
        let mut input = input;
        self.resolve_client(&mut tx, actor, &mut input, None)
            .await?;
        input.prepared_by_name = name;
        input.contact_email = email;
        let calculation = calculate(&input)?;
        let input = calculation.data.clone();
        let number = next_number(&mut tx).await?;
        let json = serde_json::to_value(&input).map_err(|_| CoreError::Invalid)?;
        let calculated_json = serde_json::to_value(&calculation).map_err(|_| CoreError::Invalid)?;
        let id = sqlx::query("INSERT INTO cotizaciones_drafts (client_name,order_number,data_json,calculation_json,quote_number,created_by,updated_by,created_at,updated_at) VALUES (?,?,?,?,?,?,?,UTC_TIMESTAMP(6),UTC_TIMESTAMP(6))")
            .bind(&input.client_name).bind(&input.order_number).bind(json).bind(calculated_json).bind(&number).bind(actor).bind(actor)
            .execute(&mut *tx).await?.last_insert_id();
        let result = QuoteDraft {
            id,
            version: 1,
            status: input.status.as_str().into(),
            currency: "HNL".into(),
            data: input,
            quote_number: Some(number),
            calculation,
        };
        let response = serde_json::to_value(&result).map_err(|_| CoreError::Invalid)?;
        sqlx::query("INSERT INTO cotizaciones_create_receipts (actor_id,key_hash,request_hash,response_json) VALUES (?,?,?,?)")
            .bind(actor).bind(key_hash).bind(request_hash).bind(response).execute(&mut *tx).await?;
        CoreAdmin::record_domain_event(
            &mut tx,
            actor,
            "cotizaciones",
            "quote.create",
            &id.to_string(),
            serde_json::json!({"version":1,"quote_number":result.quote_number,"status":result.status}),
        )
        .await?;
        tx.commit().await?;
        Ok(result)
    }

    pub async fn replace(
        &self,
        actor: u64,
        id: u64,
        expected: u64,
        input: QuoteInput,
    ) -> Result<QuoteDraft, CoreError> {
        input.validate()?;
        let mut tx = self.begin(actor, "editar").await?;
        let current = load(&mut tx, id).await?;
        if current.version != expected {
            return Err(CoreError::PreconditionFailed);
        }
        let version = expected.checked_add(1).ok_or(CoreError::Invalid)?;
        let mut input = input;
        self.resolve_client(&mut tx, actor, &mut input, Some(&current.data))
            .await?;
        input.prepared_by_name = current.data.prepared_by_name;
        input.contact_email = current.data.contact_email;
        let calculation = calculate(&input)?;
        let input = calculation.data.clone();
        let number = match current.quote_number {
            Some(value) => value,
            None => next_number(&mut tx).await?,
        };
        let calculated_json = serde_json::to_value(&calculation).map_err(|_| CoreError::Invalid)?;
        let json = serde_json::to_value(&input).map_err(|_| CoreError::Invalid)?;
        sqlx::query("UPDATE cotizaciones_drafts SET client_name=?,order_number=?,data_json=?,calculation_json=?,quote_number=?,updated_by=?,updated_at=UTC_TIMESTAMP(6),version=? WHERE id=? AND version=?")
            .bind(&input.client_name).bind(&input.order_number).bind(json).bind(calculated_json).bind(&number).bind(actor).bind(version).bind(id).bind(expected)
            .execute(&mut *tx).await?;
        CoreAdmin::record_domain_event(
            &mut tx,
            actor,
            "cotizaciones",
            "quote.update",
            &id.to_string(),
            serde_json::json!({"version":version,"status":input.status.as_str(),"previous_status":current.status}),
        )
        .await?;
        tx.commit().await?;
        Ok(QuoteDraft {
            id,
            version,
            status: input.status.as_str().into(),
            currency: "HNL".into(),
            data: input,
            quote_number: Some(number),
            calculation,
        })
    }

    pub async fn change_status(
        &self,
        actor: u64,
        id: u64,
        expected: u64,
        status: QuoteStatus,
    ) -> Result<QuoteDraft, CoreError> {
        let mut tx = self.begin(actor, "editar").await?;
        let current = load(&mut tx, id).await?;
        if current.version != expected {
            return Err(CoreError::PreconditionFailed);
        }
        let version = expected.checked_add(1).ok_or(CoreError::Invalid)?;
        let mut result = current;
        let previous = result.status.clone();
        result.version = version;
        result.status = status.as_str().into();
        result.data.status = status;
        result.calculation.data.status = status;
        sqlx::query("UPDATE cotizaciones_drafts SET data_json=?,calculation_json=?,updated_by=?,updated_at=UTC_TIMESTAMP(6),version=? WHERE id=? AND version=?")
            .bind(serde_json::to_value(&result.data).map_err(|_| CoreError::Invalid)?)
            .bind(serde_json::to_value(&result.calculation).map_err(|_| CoreError::Invalid)?)
            .bind(actor).bind(version).bind(id).bind(expected).execute(&mut *tx).await?;
        CoreAdmin::record_domain_event(&mut tx,actor,"cotizaciones","quote.status",&id.to_string(),
            serde_json::json!({"version":version,"previous_status":previous,"status":result.status})).await?;
        tx.commit().await?;
        Ok(result)
    }

    pub async fn clone_quote(
        &self,
        actor: u64,
        id: u64,
        expected: u64,
        key: &str,
    ) -> Result<QuoteDraft, CoreError> {
        if !(16..=128).contains(&key.len())
            || !key
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
        {
            return Err(CoreError::Invalid);
        }
        let key_hash = Sha256::digest(key.as_bytes()).to_vec();
        let request_hash =
            Sha256::digest(format!("quote.clone:{id}:{expected}").as_bytes()).to_vec();
        let (name, email) = self.admin.user_contact(actor).await?;
        let mut tx = self.begin(actor, "editar").await?;
        let previous: Option<(Vec<u8>, serde_json::Value)> = sqlx::query_as("SELECT request_hash,response_json FROM cotizaciones_create_receipts WHERE actor_id=? AND key_hash=?")
            .bind(actor).bind(&key_hash).fetch_optional(&mut *tx).await?;
        if let Some((hash, response)) = previous {
            if hash != request_hash {
                return Err(CoreError::Conflict);
            }
            let result = serde_json::from_value(response).map_err(|_| CoreError::Invalid)?;
            tx.commit().await?;
            return Ok(result);
        }
        let original = load(&mut tx, id).await?;
        if original.version != expected {
            return Err(CoreError::PreconditionFailed);
        }
        let mut input = original.data;
        input.id_client = None;
        input.client_name.clear();
        input.client_rtn.clear();
        input.client_email.clear();
        input.prepared_by_name = name;
        input.contact_email = email;
        input.status = QuoteStatus::Borrador;
        input.quote_date = chrono::Utc::now()
            .with_timezone(&chrono::FixedOffset::west_opt(6 * 3600).ok_or(CoreError::Invalid)?)
            .date_naive();
        let calculation = calculate(&input)?;
        let input = calculation.data.clone();
        let number = next_number(&mut tx).await?;
        let id_new = sqlx::query("INSERT INTO cotizaciones_drafts (client_name,order_number,data_json,calculation_json,quote_number,created_by,updated_by,created_at,updated_at) VALUES (?,?,?,?,?,?,?,UTC_TIMESTAMP(6),UTC_TIMESTAMP(6))")
            .bind(&input.client_name).bind(&input.order_number)
            .bind(serde_json::to_value(&input).map_err(|_| CoreError::Invalid)?)
            .bind(serde_json::to_value(&calculation).map_err(|_| CoreError::Invalid)?)
            .bind(&number).bind(actor).bind(actor).execute(&mut *tx).await?.last_insert_id();
        let result = QuoteDraft {
            id: id_new,
            version: 1,
            status: input.status.as_str().into(),
            currency: "HNL".into(),
            data: input,
            quote_number: Some(number),
            calculation,
        };
        sqlx::query("INSERT INTO cotizaciones_create_receipts (actor_id,key_hash,request_hash,response_json) VALUES (?,?,?,?)")
            .bind(actor).bind(key_hash).bind(request_hash).bind(serde_json::to_value(&result).map_err(|_| CoreError::Invalid)?).execute(&mut *tx).await?;
        CoreAdmin::record_domain_event(&mut tx,actor,"cotizaciones","quote.clone",&id_new.to_string(),
            serde_json::json!({"source_id":id,"source_version":expected,"quote_number":result.quote_number})).await?;
        tx.commit().await?;
        Ok(result)
    }

    pub async fn delete(&self, actor: u64, id: u64, expected: u64) -> Result<(), CoreError> {
        let mut tx = self.begin(actor, "eliminar").await?;
        let current = load(&mut tx, id).await?;
        if current.version != expected {
            return Err(CoreError::PreconditionFailed);
        }
        sqlx::query("INSERT INTO cotizaciones_document_deletions (object_key,available_at) SELECT object_key,UTC_TIMESTAMP(6) FROM cotizaciones_documents WHERE quote_id=? ON DUPLICATE KEY UPDATE available_at=VALUES(available_at)").bind(id).execute(&mut *tx).await?;
        sqlx::query("UPDATE cotizaciones_email_jobs SET delivery_status='cancelled',payload_ciphertext='',completed_at=UTC_TIMESTAMP(6) WHERE quote_id=? AND delivery_status IN ('pending','failed')").bind(id).execute(&mut *tx).await?;
        let delivering:i64=sqlx::query_scalar("SELECT COUNT(*) FROM cotizaciones_email_jobs WHERE quote_id=? AND delivery_status='delivering'").bind(id).fetch_one(&mut *tx).await?;
        if delivering > 0 {
            return Err(CoreError::Conflict);
        }
        sqlx::query("DELETE FROM cotizaciones_drafts WHERE id=? AND version=?")
            .bind(id)
            .bind(expected)
            .execute(&mut *tx)
            .await?;
        CoreAdmin::record_domain_event(
            &mut tx,
            actor,
            "cotizaciones",
            "quote.delete",
            &id.to_string(),
            serde_json::json!({"version":expected,"quote_number":current.quote_number}),
        )
        .await?;
        tx.commit().await?;
        Ok(())
    }
}

async fn load(tx: &mut Transaction<'_, MySql>, id: u64) -> Result<QuoteDraft, CoreError> {
    let (id, version, data, number, stored): (u64, u64, serde_json::Value, Option<String>, Option<serde_json::Value>) =
        sqlx::query_as(
            "SELECT id,version,data_json,quote_number,calculation_json FROM cotizaciones_drafts WHERE id=?",
        )
        .bind(id)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(CoreError::NotFound)?;
    let data: QuoteInput = serde_json::from_value(data).map_err(|_| CoreError::Invalid)?;
    let calculation = match stored {
        Some(value) => serde_json::from_value(value).map_err(|_| CoreError::Invalid)?,
        None => calculate(&data)?,
    };
    Ok(QuoteDraft {
        id,
        version,
        status: data.status.as_str().into(),
        currency: "HNL".into(),
        data,
        quote_number: number,
        calculation,
    })
}

async fn next_number(tx: &mut Transaction<'_, MySql>) -> Result<String, CoreError> {
    let value: u64 =
        sqlx::query_scalar("SELECT last_number FROM cotizaciones_counter WHERE id=1 FOR UPDATE")
            .fetch_one(&mut **tx)
            .await?;
    let next = value.checked_add(1).ok_or(CoreError::Invalid)?;
    sqlx::query("UPDATE cotizaciones_counter SET last_number=? WHERE id=1")
        .bind(next)
        .execute(&mut **tx)
        .await?;
    Ok(format!("COT-{next:06}"))
}
