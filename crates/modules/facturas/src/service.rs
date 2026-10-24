use bastion_core::{CoreAdmin, CoreError, Database};
use sha2::{Digest, Sha256};
use sqlx::{MySql, QueryBuilder, Row, Transaction};

use crate::{
    InvoiceCalculation, InvoiceDraft, InvoiceFilter, InvoiceInput, InvoicePage, InvoiceSummary,
    calculate,
};

#[path = "conversion.rs"]
mod conversion;

#[derive(Clone)]
pub struct InvoiceService {
    database: Database,
    admin: CoreAdmin,
}

impl InvoiceService {
    pub fn new(database: Database, admin: CoreAdmin) -> Self {
        Self { database, admin }
    }

    pub async fn migrate(database: &Database) -> Result<(), CoreError> {
        let mut migrator = sqlx::migrate!("./migrations");
        migrator.dangerous_set_table_name("_facturas_migrations");
        migrator
            .run(database.pool())
            .await
            .map_err(CoreError::Migration)
    }

    pub async fn synchronize_release(&self, allowed: bool) -> Result<(), CoreError> {
        let exists: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM information_schema.tables WHERE table_schema=DATABASE() AND table_name='_facturas_migrations'").fetch_one(self.database.pool()).await?;
        let ready = if exists > 0 {
            let applied = sqlx::query_as::<_, (i64, bool, Vec<u8>)>(
                "SELECT version,success,checksum FROM _facturas_migrations",
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
        sqlx::query("INSERT INTO module_registry (module_id,display_name,included_in_release,environment_allowed,enabled) VALUES ('facturas','Facturas',1,?,0) ON DUPLICATE KEY UPDATE included_in_release=1,environment_allowed=VALUES(environment_allowed)")
            .bind(allowed && ready).execute(self.database.pool()).await?;
        Ok(())
    }

    async fn begin(
        &self,
        actor: u64,
        capability: &str,
    ) -> Result<Transaction<'_, MySql>, CoreError> {
        self.admin
            .authorize(actor, "facturas", "documentos", capability)
            .await?;
        let mut tx = self.database.pool().begin().await?;
        let enabled: Option<bool> = sqlx::query_scalar("SELECT (included_in_release AND environment_allowed AND enabled) FROM module_registry WHERE module_id='facturas' FOR UPDATE").fetch_optional(&mut *tx).await?;
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
        sqlx::query("INSERT INTO facturas_actor_limits (actor_id,window_start,requests) VALUES (?,UTC_TIMESTAMP(6),1) ON DUPLICATE KEY UPDATE requests=IF(window_start<=UTC_TIMESTAMP(6)-INTERVAL 1 MINUTE,1,requests+1),window_start=IF(window_start<=UTC_TIMESTAMP(6)-INTERVAL 1 MINUTE,UTC_TIMESTAMP(6),window_start)")
            .bind(actor).execute(&mut *tx).await?;
        let count: u32 =
            sqlx::query_scalar("SELECT requests FROM facturas_actor_limits WHERE actor_id=?")
                .bind(actor)
                .fetch_one(&mut *tx)
                .await?;
        tx.commit().await?;
        if count > 120 {
            return Err(CoreError::Locked);
        }
        Ok(())
    }

    pub async fn list(&self, actor: u64, filter: &InvoiceFilter) -> Result<InvoicePage, CoreError> {
        let (page, page_size) = filter.bounds()?;
        let mut tx = self.begin(actor, "ver").await?;
        let mut query = QueryBuilder::<MySql>::new(
            "SELECT id,version,client_name,order_number,CAST(invoice_date AS CHAR) AS invoice_date,CAST(JSON_LENGTH(data_json,'$.items') AS UNSIGNED) AS item_count,status,JSON_UNQUOTE(JSON_EXTRACT(calculation_json,'$.totals.total')) AS total,JSON_UNQUOTE(JSON_EXTRACT(calculation_json,'$.balance')) AS balance FROM facturas_drafts WHERE 1=1",
        );
        if let Some(term) = filter
            .search
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            query
                .push(" AND (LOCATE(")
                .push_bind(term)
                .push(",client_name)>0 OR LOCATE(")
                .push_bind(term)
                .push(",order_number)>0)");
        }
        query
            .push(" ORDER BY created_at DESC,id DESC LIMIT ")
            .push_bind(page_size + 1)
            .push(" OFFSET ")
            .push_bind((page - 1) * page_size);
        let mut items: Vec<InvoiceSummary> = query.build_query_as().fetch_all(&mut *tx).await?;
        let has_more = items.len() > page_size as usize;
        items.truncate(page_size as usize);
        tx.commit().await?;
        Ok(InvoicePage {
            items,
            page,
            page_size,
            has_more,
        })
    }

    pub async fn detail(&self, actor: u64, id: u64) -> Result<InvoiceDraft, CoreError> {
        let mut tx = self.begin(actor, "ver").await?;
        let draft = load(&mut tx, id).await?;
        tx.commit().await?;
        Ok(draft)
    }

    pub async fn preview(
        &self,
        actor: u64,
        input: &InvoiceInput,
    ) -> Result<InvoiceCalculation, CoreError> {
        let tx = self.begin(actor, "editar").await?;
        let result = calculate(input)?;
        tx.commit().await?;
        Ok(result)
    }

    pub async fn create(
        &self,
        actor: u64,
        key: &str,
        mut input: InvoiceInput,
    ) -> Result<InvoiceDraft, CoreError> {
        if !(16..=128).contains(&key.len())
            || !key
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        {
            return Err(CoreError::Invalid);
        }
        let mut tx = self.begin(actor, "editar").await?;
        let mut calculation = calculate(&input)?;
        calculation.ensure_saveable()?;
        let data = serde_json::to_value(&calculation.data).map_err(|_| CoreError::Invalid)?;
        let request_hash =
            Sha256::digest(serde_json::to_vec(&data).map_err(|_| CoreError::Invalid)?).to_vec();
        let key_hash = Sha256::digest(key.as_bytes()).to_vec();
        let previous: Option<(Vec<u8>, serde_json::Value)> = sqlx::query_as("SELECT request_hash,response_json FROM facturas_create_receipts WHERE actor_id=? AND key_hash=?").bind(actor).bind(&key_hash).fetch_optional(&mut *tx).await?;
        if let Some((hash, response)) = previous {
            if hash != request_hash {
                return Err(CoreError::Conflict);
            }
            let result = serde_json::from_value(response).map_err(|_| CoreError::Invalid)?;
            tx.commit().await?;
            return Ok(result);
        }
        self.resolve_client(&mut tx, actor, &mut input, None)
            .await?;
        calculation = calculate(&input)?;
        calculation.ensure_saveable()?;
        let data = serde_json::to_value(&calculation.data).map_err(|_| CoreError::Invalid)?;
        let id = sqlx::query("INSERT INTO facturas_drafts (client_name,order_number,invoice_date,data_json,calculation_json,created_by,updated_by,created_at,updated_at) VALUES (?,?,?,?,?,?,?,UTC_TIMESTAMP(6),UTC_TIMESTAMP(6))")
            .bind(&calculation.data.client_name).bind(&calculation.data.order_number).bind(calculation.data.invoice_date).bind(data)
            .bind(serde_json::to_value(&calculation).map_err(|_| CoreError::Invalid)?).bind(actor).bind(actor).execute(&mut *tx).await?.last_insert_id();
        let draft = InvoiceDraft {
            id,
            version: 1,
            status: "Borrador".into(),
            currency: "HNL".into(),
            invoice_number: None,
            source_quote: None,
            data: calculation.data.clone(),
            calculation,
        };
        sqlx::query("INSERT INTO facturas_create_receipts (actor_id,key_hash,request_hash,response_json) VALUES (?,?,?,?)")
            .bind(actor).bind(key_hash).bind(request_hash).bind(serde_json::to_value(&draft).map_err(|_| CoreError::Invalid)?).execute(&mut *tx).await?;
        CoreAdmin::record_domain_event(
            &mut tx,
            actor,
            "facturas",
            "invoice.draft.create",
            &id.to_string(),
            serde_json::json!({"version":1}),
        )
        .await?;
        tx.commit().await?;
        Ok(draft)
    }

    pub async fn replace(
        &self,
        actor: u64,
        id: u64,
        expected: u64,
        mut input: InvoiceInput,
    ) -> Result<InvoiceDraft, CoreError> {
        let mut tx = self.begin(actor, "editar").await?;
        let current = load(&mut tx, id).await?;
        if current.version != expected {
            return Err(CoreError::PreconditionFailed);
        }
        if current.status != "Borrador" {
            return Err(CoreError::Conflict);
        }
        self.resolve_client(&mut tx, actor, &mut input, Some(&current.data))
            .await?;
        let calculation = calculate(&input)?;
        calculation.ensure_saveable()?;
        let version = expected.checked_add(1).ok_or(CoreError::Invalid)?;
        sqlx::query("UPDATE facturas_drafts SET version=?,client_name=?,order_number=?,invoice_date=?,data_json=?,calculation_json=?,updated_by=?,updated_at=UTC_TIMESTAMP(6) WHERE id=?")
            .bind(version).bind(&calculation.data.client_name).bind(&calculation.data.order_number).bind(calculation.data.invoice_date)
            .bind(serde_json::to_value(&calculation.data).map_err(|_| CoreError::Invalid)?).bind(serde_json::to_value(&calculation).map_err(|_| CoreError::Invalid)?).bind(actor).bind(id).execute(&mut *tx).await?;
        CoreAdmin::record_domain_event(
            &mut tx,
            actor,
            "facturas",
            "invoice.draft.update",
            &id.to_string(),
            serde_json::json!({"version":version}),
        )
        .await?;
        tx.commit().await?;
        Ok(InvoiceDraft {
            id,
            version,
            status: "Borrador".into(),
            currency: "HNL".into(),
            invoice_number: None,
            source_quote: current.source_quote,
            data: calculation.data.clone(),
            calculation,
        })
    }

    async fn resolve_client(
        &self,
        tx: &mut Transaction<'_, MySql>,
        actor: u64,
        input: &mut InvoiceInput,
        current: Option<&InvoiceInput>,
    ) -> Result<(), CoreError> {
        if let Some(id) = input.id_client {
            if let Some(previous) = current.filter(|previous| previous.id_client == Some(id)) {
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
}

async fn load(tx: &mut Transaction<'_, MySql>, id: u64) -> Result<InvoiceDraft, CoreError> {
    let row = sqlx::query("SELECT id,version,status,data_json,calculation_json,source_quote_json FROM facturas_drafts WHERE id=? FOR UPDATE").bind(id).fetch_optional(&mut **tx).await?.ok_or(CoreError::NotFound)?;
    let data: serde_json::Value = row.try_get("data_json")?;
    let calculation: serde_json::Value = row.try_get("calculation_json")?;
    let origin: Option<serde_json::Value> = row.try_get("source_quote_json")?;
    Ok(InvoiceDraft {
        id: row.try_get("id")?,
        version: row.try_get("version")?,
        status: row.try_get("status")?,
        currency: "HNL".into(),
        invoice_number: None,
        source_quote: origin
            .map(serde_json::from_value)
            .transpose()
            .map_err(|_| CoreError::Invalid)?,
        data: serde_json::from_value(data).map_err(|_| CoreError::Invalid)?,
        calculation: serde_json::from_value(calculation).map_err(|_| CoreError::Invalid)?,
    })
}
