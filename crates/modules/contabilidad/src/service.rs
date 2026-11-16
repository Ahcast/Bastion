use crate::*;
use bastion_core::{CoreAdmin, Database};
use sha2::{Digest, Sha256};
use sqlx::{MySql, QueryBuilder, Row, Transaction};

#[derive(Clone)]
pub struct AccountingService {
    pub(crate) database: Database,
    pub(crate) admin: CoreAdmin,
}
impl AccountingService {
    pub fn new(database: Database, admin: CoreAdmin) -> Self {
        Self { database, admin }
    }
    pub async fn migrate(database: &Database) -> Result<(), CoreError> {
        let mut m = sqlx::migrate!("./migrations");
        m.dangerous_set_table_name("_contabilidad_migrations");
        m.run(database.pool()).await.map_err(CoreError::Migration)
    }
    pub async fn synchronize_release(&self, allowed: bool) -> Result<(), CoreError> {
        let exists:i64=sqlx::query_scalar("SELECT COUNT(*) FROM information_schema.tables WHERE table_schema=DATABASE() AND table_name='_contabilidad_migrations'").fetch_one(self.database.pool()).await?;
        let ready = if exists > 0 {
            let applied = sqlx::query_as::<_, (i64, bool, Vec<u8>)>(
                "SELECT version,success,checksum FROM _contabilidad_migrations",
            )
            .fetch_all(self.database.pool())
            .await?;
            let expected = sqlx::migrate!("./migrations");
            applied.len() == expected.iter().count()
                && expected.iter().all(|m| {
                    applied.iter().any(|(v, s, c)| {
                        *v == m.version && *s && c.as_slice() == m.checksum.as_ref()
                    })
                })
        } else {
            false
        };
        sqlx::query("INSERT INTO module_registry(module_id,display_name,included_in_release,environment_allowed,enabled) VALUES('contabilidad','Contabilidad',1,?,0) ON DUPLICATE KEY UPDATE included_in_release=1,environment_allowed=VALUES(environment_allowed)").bind(allowed&&ready).execute(self.database.pool()).await?;
        Ok(())
    }
    pub async fn check_access(&self, actor: u64, capability: &str) -> Result<(), CoreError> {
        self.admin
            .authorize_enabled_module(actor, "contabilidad", "libros", capability)
            .await
    }
    pub(crate) async fn begin(
        &self,
        actor: u64,
        capability: &str,
    ) -> Result<Transaction<'_, MySql>, CoreError> {
        self.check_access(actor, capability).await?;
        let mut tx = self.database.pool().begin().await?;
        CoreAdmin::lock_enabled_module(&mut tx, "contabilidad", capability != "ver").await?;
        self.admin
            .authorize_in_transaction(&mut tx, actor, "contabilidad", "libros", capability)
            .await?;
        Ok(tx)
    }
    pub async fn consume_request(&self, actor: u64) -> Result<(), CoreError> {
        let mut tx = self.database.pool().begin().await?;
        sqlx::query("INSERT INTO contabilidad_actor_limits(actor_id,window_start,requests) VALUES(?,UTC_TIMESTAMP(6),1) ON DUPLICATE KEY UPDATE requests=IF(window_start<=UTC_TIMESTAMP(6)-INTERVAL 1 MINUTE,1,requests+1),window_start=IF(window_start<=UTC_TIMESTAMP(6)-INTERVAL 1 MINUTE,UTC_TIMESTAMP(6),window_start)").bind(actor).execute(&mut *tx).await?;
        let count: u32 =
            sqlx::query_scalar("SELECT requests FROM contabilidad_actor_limits WHERE actor_id=?")
                .bind(actor)
                .fetch_one(&mut *tx)
                .await?;
        tx.commit().await?;
        if count > 120 {
            Err(CoreError::Locked)
        } else {
            Ok(())
        }
    }
    async fn audit(
        tx: &mut Transaction<'_, MySql>,
        actor: u64,
        action: &str,
        target: &str,
        details: serde_json::Value,
    ) -> Result<(), CoreError> {
        CoreAdmin::record_domain_event(tx, actor, "contabilidad", action, target, details).await
    }
    pub async fn accounts(&self, actor: u64) -> Result<Vec<Account>, CoreError> {
        let mut tx = self.begin(actor, "ver").await?;
        let rows=sqlx::query_as::<_,Account>("SELECT code,name,class,parent_code,posting,active,version FROM contabilidad_accounts ORDER BY code LIMIT 500").fetch_all(&mut *tx).await?;
        tx.commit().await?;
        Ok(rows)
    }
    pub async fn create_account(
        &self,
        actor: u64,
        input: AccountInput,
    ) -> Result<Account, CoreError> {
        valid_code(&input.code)?;
        valid_text(&input.name, 120, true)?;
        let mut tx = self.begin(actor, "editar").await?;
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM contabilidad_accounts")
            .fetch_one(&mut *tx)
            .await?;
        if count >= 500 {
            return Err(CoreError::Conflict);
        }
        let parent = input.parent_code.as_deref().ok_or(CoreError::Invalid)?;
        let parent_row = account(&mut tx, parent).await?;
        if parent_row.posting
            || !parent_row.active
            || parent_row.class != input.class
            || input.code.len() <= parent.len()
            || !input.code.starts_with(parent)
        {
            return Err(CoreError::Invalid);
        }
        if sqlx::query_scalar::<_, String>("SELECT code FROM contabilidad_accounts WHERE code=?")
            .bind(&input.code)
            .fetch_optional(&mut *tx)
            .await?
            .is_some()
        {
            return Err(CoreError::Conflict);
        }
        sqlx::query("INSERT INTO contabilidad_accounts(code,name,class,parent_code,posting) VALUES(?,?,?,?,?)").bind(&input.code).bind(input.name.trim()).bind(&input.class).bind(parent).bind(input.posting).execute(&mut *tx).await?;
        Self::audit(
            &mut tx,
            actor,
            "account.create",
            &input.code,
            serde_json::json!({}),
        )
        .await?;
        let row = account(&mut tx, &input.code).await?;
        tx.commit().await?;
        Ok(row)
    }
    pub async fn update_account(
        &self,
        actor: u64,
        code: &str,
        version: u64,
        input: AccountUpdate,
    ) -> Result<Account, CoreError> {
        valid_text(&input.name, 120, true)?;
        let mut tx = self.begin(actor, "editar").await?;
        let row = account(&mut tx, code).await?;
        if row.version != version {
            return Err(CoreError::PreconditionFailed);
        }
        if !input.active {
            let children: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM contabilidad_accounts WHERE parent_code=? AND active=1",
            )
            .bind(code)
            .fetch_one(&mut *tx)
            .await?;
            if children > 0 {
                return Err(CoreError::Conflict);
            }
        } else if let Some(parent) = row.parent_code.as_deref() {
            if !account(&mut tx, parent).await?.active {
                return Err(CoreError::Conflict);
            }
        }
        sqlx::query(
            "UPDATE contabilidad_accounts SET name=?,active=?,version=version+1 WHERE code=?",
        )
        .bind(input.name.trim())
        .bind(input.active)
        .bind(code)
        .execute(&mut *tx)
        .await?;
        Self::audit(
            &mut tx,
            actor,
            "account.update",
            code,
            serde_json::json!({"active":input.active}),
        )
        .await?;
        let row = account(&mut tx, code).await?;
        tx.commit().await?;
        Ok(row)
    }
    pub async fn periods(&self, actor: u64) -> Result<Vec<Period>, CoreError> {
        let mut tx = self.begin(actor, "ver").await?;
        let rows = sqlx::query_as::<_, Period>(
            "SELECT month,closed,version FROM contabilidad_periods ORDER BY month DESC LIMIT 1212",
        )
        .fetch_all(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(rows)
    }
    pub async fn open_period(&self, actor: u64, month: &str) -> Result<Period, CoreError> {
        valid_month(month)?;
        let mut tx = self.begin(actor, "editar").await?;
        if period(&mut tx, month).await?.is_some() {
            return Err(CoreError::Conflict);
        }
        sqlx::query("INSERT INTO contabilidad_periods(month) VALUES(?)")
            .bind(month)
            .execute(&mut *tx)
            .await?;
        Self::audit(&mut tx, actor, "period.open", month, serde_json::json!({})).await?;
        tx.commit().await?;
        Ok(Period {
            month: month.into(),
            closed: false,
            version: 1,
        })
    }
    pub async fn update_period(
        &self,
        actor: u64,
        month: &str,
        version: u64,
        input: PeriodUpdate,
    ) -> Result<Period, CoreError> {
        valid_text(&input.reason, 240, true)?;
        let mut tx = self.begin(actor, "editar").await?;
        let row = period(&mut tx, month).await?.ok_or(CoreError::NotFound)?;
        if row.version != version {
            return Err(CoreError::PreconditionFailed);
        }
        if input.closed {
            let pending: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM contabilidad_entries WHERE month=? AND status='borrador'",
            )
            .bind(month)
            .fetch_one(&mut *tx)
            .await?;
            if pending > 0 {
                return Err(CoreError::Conflict);
            }
        }
        sqlx::query("UPDATE contabilidad_periods SET closed=?,version=version+1 WHERE month=?")
            .bind(input.closed)
            .bind(month)
            .execute(&mut *tx)
            .await?;
        Self::audit(
            &mut tx,
            actor,
            if input.closed {
                "period.close"
            } else {
                "period.reopen"
            },
            month,
            serde_json::json!({"reason":input.reason}),
        )
        .await?;
        tx.commit().await?;
        Ok(Period {
            month: month.into(),
            closed: input.closed,
            version: version + 1,
        })
    }
    pub async fn list(&self, actor: u64, filter: &EntryFilter) -> Result<EntryPage, CoreError> {
        let page = filter.page.unwrap_or(1);
        if !(1..=10000).contains(&page) {
            return Err(CoreError::Invalid);
        }
        if let Some(m) = filter.month.as_deref() {
            valid_month(m)?;
        }
        let mut tx = self.begin(actor, "ver").await?;
        let mut query = QueryBuilder::<MySql>::new(
            "SELECT id,version,entry_date AS date,description,status,kind,CAST(total_cents AS CHAR) AS total,reversed_by FROM contabilidad_entries",
        );
        if let Some(m) = filter.month.as_deref() {
            query.push(" WHERE month=").push_bind(m);
        }
        query
            .push(" ORDER BY entry_date DESC,id DESC LIMIT 51 OFFSET ")
            .push_bind((page - 1) * 50);
        let mut rows = query
            .build_query_as::<EntrySummary>()
            .fetch_all(&mut *tx)
            .await?;
        let has_more = rows.len() > 50;
        rows.truncate(50);
        for row in &mut rows {
            row.total = money(row.total.parse().map_err(|_| CoreError::Invalid)?);
        }
        tx.commit().await?;
        Ok(EntryPage {
            items: rows,
            page,
            has_more,
        })
    }
    pub async fn detail(&self, actor: u64, id: u64) -> Result<Entry, CoreError> {
        let mut tx = self.begin(actor, "ver").await?;
        let mut row = load(&mut tx, id).await?;
        if let Some(reference) = &row.data.source {
            row.source_status = match self
                .source(&mut tx, actor, &reference.module, reference.id)
                .await
            {
                Ok(current) if current.reference == *reference => "vigente",
                Ok(_) => "modificado",
                Err(_) => "no_disponible",
            }
            .into();
        }
        tx.commit().await?;
        Ok(row)
    }
    async fn validate_source(
        &self,
        tx: &mut Transaction<'_, MySql>,
        actor: u64,
        input: &EntryInput,
    ) -> Result<Option<SourceSnapshot>, CoreError> {
        if let Some(reference) = &input.source {
            let snapshot = self
                .source(tx, actor, &reference.module, reference.id)
                .await?;
            if snapshot.reference != *reference {
                return Err(CoreError::PreconditionFailed);
            }
            Ok(Some(snapshot))
        } else {
            Ok(None)
        }
    }
    pub async fn create(
        &self,
        actor: u64,
        key: &str,
        input: EntryInput,
    ) -> Result<Entry, CoreError> {
        input.validate()?;
        if key.is_empty()
            || key.starts_with("reversal-")
            || key.len() > 80
            || !key
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"-_".contains(&c))
        {
            return Err(CoreError::Invalid);
        }
        let hash =
            Sha256::digest(serde_json::to_vec(&input).map_err(|_| CoreError::Invalid)?).to_vec();
        let mut tx = self.begin(actor, "editar").await?;
        if let Some((id,existing))=sqlx::query_as::<_,(u64,Vec<u8>)>("SELECT id,request_hash FROM contabilidad_entries WHERE created_by=? AND idempotency_key=?").bind(actor).bind(key).fetch_optional(&mut *tx).await?{
            if hash!=existing{return Err(CoreError::Conflict);}let row=load(&mut tx,id).await?;tx.commit().await?;return Ok(row);
        }
        open_for(&mut tx, input.date).await?;
        validate_accounts(&mut tx, &input.lines).await?;
        let snapshot = self.validate_source(&mut tx, actor, &input).await?;
        let source_key = source_key(&input.source);
        if let Some(key) = &source_key {
            let exists: i64 =
                sqlx::query_scalar("SELECT COUNT(*) FROM contabilidad_entries WHERE source_key=?")
                    .bind(key)
                    .fetch_one(&mut *tx)
                    .await?;
            if exists > 0 {
                return Err(CoreError::Conflict);
            }
        }
        let id = insert(&mut tx, actor, key, &hash, &input, snapshot.as_ref(), None).await?;
        Self::audit(
            &mut tx,
            actor,
            "entry.create",
            &id.to_string(),
            serde_json::json!({"source":input.source}),
        )
        .await?;
        let row = load(&mut tx, id).await?;
        tx.commit().await?;
        Ok(row)
    }
    pub async fn replace(
        &self,
        actor: u64,
        id: u64,
        version: u64,
        input: EntryInput,
    ) -> Result<Entry, CoreError> {
        let total = input.validate()?;
        let mut tx = self.begin(actor, "editar").await?;
        let current = load(&mut tx, id).await?;
        if current.version != version {
            return Err(CoreError::PreconditionFailed);
        }
        if current.status != "borrador" {
            return Err(CoreError::Conflict);
        }
        if source_key(&current.data.source) != source_key(&input.source) {
            return Err(CoreError::Conflict);
        }
        open_for(&mut tx, current.data.date).await?;
        open_for(&mut tx, input.date).await?;
        validate_accounts(&mut tx, &input.lines).await?;
        let snapshot = self.validate_source(&mut tx, actor, &input).await?;
        sqlx::query("UPDATE contabilidad_entries SET version=version+1,entry_date=?,month=?,description=?,reference=?,kind=?,total_cents=?,source_json=?,updated_by=?,updated_at=UTC_TIMESTAMP(6) WHERE id=?")
            .bind(input.date).bind(input.date.format("%Y-%m").to_string()).bind(input.description.trim()).bind(input.reference.trim()).bind(&input.kind).bind(total).bind(snapshot.as_ref().map(serde_json::to_value).transpose().map_err(|_|CoreError::Invalid)?).bind(actor).bind(id).execute(&mut *tx).await?;
        sqlx::query("DELETE FROM contabilidad_lines WHERE entry_id=?")
            .bind(id)
            .execute(&mut *tx)
            .await?;
        write_lines(&mut tx, id, &input.lines).await?;
        Self::audit(
            &mut tx,
            actor,
            "entry.update",
            &id.to_string(),
            serde_json::json!({"version":version+1}),
        )
        .await?;
        let row = load(&mut tx, id).await?;
        tx.commit().await?;
        Ok(row)
    }
    pub async fn act(
        &self,
        actor: u64,
        id: u64,
        version: u64,
        action: &str,
        input: EntryAction,
    ) -> Result<Entry, CoreError> {
        if !matches!(action, "contabilizar" | "cancelar" | "reversar") {
            return Err(CoreError::Invalid);
        }
        let mut tx = self
            .begin(
                actor,
                if action == "contabilizar" {
                    "editar"
                } else {
                    "eliminar"
                },
            )
            .await?;
        let current = load(&mut tx, id).await?;
        if current.version != version {
            return Err(CoreError::PreconditionFailed);
        }
        if action == "reversar" {
            if current.status != "contabilizado"
                || current.reversed_by.is_some()
                || current.reversal_of.is_some()
            {
                return Err(CoreError::Conflict);
            }
            let date = input.date.ok_or(CoreError::Invalid)?;
            let reason = input.reason.ok_or(CoreError::Invalid)?;
            valid_text(&reason, 200, true)?;
            if date < current.data.date {
                return Err(CoreError::Invalid);
            }
            open_for(&mut tx, date).await?;
            let reversal = EntryInput {
                date,
                description: format!("Reverso #{id}: {reason}"),
                reference: current.data.reference.clone(),
                kind: "manual".into(),
                source: None,
                lines: current
                    .data
                    .lines
                    .iter()
                    .map(|l| LineInput {
                        account: l.account.clone(),
                        debit: l.credit.clone(),
                        credit: l.debit.clone(),
                    })
                    .collect(),
            };
            reversal.validate()?;
            // Reversal must remain possible after an account is deactivated.
            let hash =
                Sha256::digest(serde_json::to_vec(&reversal).map_err(|_| CoreError::Invalid)?)
                    .to_vec();
            let reversal_id = insert(
                &mut tx,
                actor,
                &format!("reversal-{id}"),
                &hash,
                &reversal,
                None,
                Some(id),
            )
            .await?;
            sqlx::query(
                "UPDATE contabilidad_entries SET kind='reverso',status='contabilizado' WHERE id=?",
            )
            .bind(reversal_id)
            .execute(&mut *tx)
            .await?;
            sqlx::query("UPDATE contabilidad_entries SET reversed_by=?,version=version+1,updated_by=?,updated_at=UTC_TIMESTAMP(6) WHERE id=?").bind(reversal_id).bind(actor).bind(id).execute(&mut *tx).await?;
            Self::audit(
                &mut tx,
                actor,
                "entry.reverse",
                &id.to_string(),
                serde_json::json!({"reversal_id":reversal_id,"reason":reason}),
            )
            .await?;
            let row = load(&mut tx, reversal_id).await?;
            tx.commit().await?;
            return Ok(row);
        }
        if current.status != "borrador" {
            return Err(CoreError::Conflict);
        }
        open_for(&mut tx, current.data.date).await?;
        if action == "contabilizar" {
            current.data.validate()?;
            validate_accounts(&mut tx, &current.data.lines).await?;
            self.validate_source(&mut tx, actor, &current.data).await?;
        }
        let status = if action == "contabilizar" {
            "contabilizado"
        } else {
            "cancelado"
        };
        sqlx::query("UPDATE contabilidad_entries SET status=?,source_key=IF(?='cancelado',NULL,source_key),version=version+1,updated_by=?,updated_at=UTC_TIMESTAMP(6) WHERE id=?").bind(status).bind(status).bind(actor).bind(id).execute(&mut *tx).await?;
        Self::audit(
            &mut tx,
            actor,
            if action == "contabilizar" {
                "entry.post"
            } else {
                "entry.cancel"
            },
            &id.to_string(),
            serde_json::json!({}),
        )
        .await?;
        let row = load(&mut tx, id).await?;
        tx.commit().await?;
        Ok(row)
    }
    pub async fn balance(&self, actor: u64, filter: &BalanceFilter) -> Result<Balance, CoreError> {
        if filter.date_from > filter.date_to
            || filter
                .date_to
                .signed_duration_since(filter.date_from)
                .num_days()
                > 365
        {
            return Err(CoreError::Invalid);
        }
        valid_month(&filter.date_from.format("%Y-%m").to_string())?;
        valid_month(&filter.date_to.format("%Y-%m").to_string())?;
        let mut tx = self.begin(actor, "ver").await?;
        let rows=sqlx::query("SELECT a.code,a.name,a.class,CAST(COALESCE(SUM(IF(e.entry_date<?,l.debit_cents-l.credit_cents,0)),0) AS CHAR) AS opening,CAST(COALESCE(SUM(IF(e.entry_date>=?,l.debit_cents,0)),0) AS CHAR) AS debit,CAST(COALESCE(SUM(IF(e.entry_date>=?,l.credit_cents,0)),0) AS CHAR) AS credit FROM contabilidad_accounts a LEFT JOIN (contabilidad_lines l INNER JOIN contabilidad_entries e ON e.id=l.entry_id AND e.status='contabilizado' AND e.entry_date<=?) ON l.account_code=a.code WHERE a.posting=1 GROUP BY a.code,a.name,a.class ORDER BY a.code LIMIT 500")
            .bind(filter.date_from).bind(filter.date_from).bind(filter.date_from).bind(filter.date_to).fetch_all(&mut *tx).await?;
        let (mut items, mut td, mut tc, mut net) = (Vec::new(), 0_i64, 0_i64, 0_i64);
        for row in rows {
            let parse = |key: &str| -> Result<i64, CoreError> {
                row.try_get::<String, _>(key)?
                    .parse()
                    .map_err(|_| CoreError::Invalid)
            };
            let (o, d, c) = (parse("opening")?, parse("debit")?, parse("credit")?);
            let b = o
                .checked_add(d)
                .and_then(|v| v.checked_sub(c))
                .ok_or(CoreError::Invalid)?;
            td = td.checked_add(d).ok_or(CoreError::Invalid)?;
            tc = tc.checked_add(c).ok_or(CoreError::Invalid)?;
            net = net.checked_add(b).ok_or(CoreError::Invalid)?;
            items.push(BalanceRow {
                code: row.try_get("code")?,
                name: row.try_get("name")?,
                class: row.try_get("class")?,
                opening: money(o),
                debit: money(d),
                credit: money(c),
                balance: money(b),
            });
        }
        tx.commit().await?;
        Ok(Balance {
            date_from: filter.date_from,
            date_to: filter.date_to,
            currency: "HNL".into(),
            items,
            total_debit: money(td),
            total_credit: money(tc),
            balanced: td == tc && net == 0,
        })
    }
}
fn source_key(source: &Option<SourceRef>) -> Option<String> {
    source.as_ref().map(|s| format!("{}:{}", s.module, s.id))
}
async fn account(tx: &mut Transaction<'_, MySql>, code: &str) -> Result<Account, CoreError> {
    sqlx::query_as("SELECT code,name,class,parent_code,posting,active,version FROM contabilidad_accounts WHERE code=? FOR UPDATE").bind(code).fetch_optional(&mut **tx).await?.ok_or(CoreError::NotFound)
}
async fn period(tx: &mut Transaction<'_, MySql>, month: &str) -> Result<Option<Period>, CoreError> {
    Ok(sqlx::query_as(
        "SELECT month,closed,version FROM contabilidad_periods WHERE month=? FOR UPDATE",
    )
    .bind(month)
    .fetch_optional(&mut **tx)
    .await?)
}
async fn open_for(tx: &mut Transaction<'_, MySql>, date: NaiveDate) -> Result<(), CoreError> {
    let month = date.format("%Y-%m").to_string();
    valid_month(&month)?;
    if period(tx, &month).await?.is_none_or(|p| p.closed) {
        return Err(CoreError::Conflict);
    }
    Ok(())
}
async fn validate_accounts(
    tx: &mut Transaction<'_, MySql>,
    lines: &[LineInput],
) -> Result<(), CoreError> {
    let codes: std::collections::BTreeSet<_> = lines.iter().map(|l| l.account.as_str()).collect();
    let mut query = QueryBuilder::<MySql>::new(
        "SELECT code FROM contabilidad_accounts WHERE active=1 AND posting=1 AND code IN (",
    );
    let mut values = query.separated(",");
    for code in &codes {
        values.push_bind(*code);
    }
    values.push_unseparated(") FOR UPDATE");
    let found = query
        .build_query_scalar::<String>()
        .fetch_all(&mut **tx)
        .await?;
    if found.len() != codes.len() {
        return Err(CoreError::Invalid);
    }
    Ok(())
}
async fn write_lines(
    tx: &mut Transaction<'_, MySql>,
    id: u64,
    lines: &[LineInput],
) -> Result<(), CoreError> {
    let amounts = lines
        .iter()
        .map(|l| Ok((cents(&l.debit)?, cents(&l.credit)?)))
        .collect::<Result<Vec<_>, CoreError>>()?;
    let mut query = QueryBuilder::<MySql>::new(
        "INSERT INTO contabilidad_lines(entry_id,line_number,account_code,debit_cents,credit_cents) ",
    );
    query.push_values(
        lines.iter().zip(amounts).enumerate(),
        |mut row, (i, (line, (debit, credit)))| {
            row.push_bind(id)
                .push_bind(i as u32 + 1)
                .push_bind(&line.account)
                .push_bind(debit)
                .push_bind(credit);
        },
    );
    query.build().execute(&mut **tx).await?;
    Ok(())
}
async fn insert(
    tx: &mut Transaction<'_, MySql>,
    actor: u64,
    key: &str,
    hash: &[u8],
    input: &EntryInput,
    snapshot: Option<&SourceSnapshot>,
    reversal: Option<u64>,
) -> Result<u64, CoreError> {
    let total = input.validate()?;
    let id=sqlx::query("INSERT INTO contabilidad_entries(entry_date,month,description,reference,kind,total_cents,source_key,source_json,reversal_of,created_by,updated_by,idempotency_key,request_hash) VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?)")
        .bind(input.date).bind(input.date.format("%Y-%m").to_string()).bind(input.description.trim()).bind(input.reference.trim()).bind(&input.kind).bind(total).bind(source_key(&input.source)).bind(snapshot.map(serde_json::to_value).transpose().map_err(|_|CoreError::Invalid)?).bind(reversal).bind(actor).bind(actor).bind(key).bind(hash).execute(&mut **tx).await?.last_insert_id();
    write_lines(tx, id, &input.lines).await?;
    Ok(id)
}
async fn load(tx: &mut Transaction<'_, MySql>, id: u64) -> Result<Entry, CoreError> {
    let row = sqlx::query("SELECT * FROM contabilidad_entries WHERE id=? FOR UPDATE")
        .bind(id)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(CoreError::NotFound)?;
    let lines=sqlx::query_as::<_,(String,i64,i64)>("SELECT account_code,debit_cents,credit_cents FROM contabilidad_lines WHERE entry_id=? ORDER BY line_number").bind(id).fetch_all(&mut **tx).await?.into_iter().map(|(a,d,c)|LineInput{account:a,debit:money(d),credit:money(c)}).collect();
    let snapshot: Option<SourceSnapshot> = row
        .try_get::<Option<serde_json::Value>, _>("source_json")?
        .map(serde_json::from_value)
        .transpose()
        .map_err(|_| CoreError::Invalid)?;
    Ok(Entry {
        id,
        version: row.try_get("version")?,
        status: row.try_get("status")?,
        total: money(row.try_get("total_cents")?),
        reversal_of: row.try_get("reversal_of")?,
        reversed_by: row.try_get("reversed_by")?,
        data: EntryInput {
            date: row.try_get("entry_date")?,
            description: row.try_get("description")?,
            reference: row.try_get("reference")?,
            kind: row.try_get("kind")?,
            source: snapshot.as_ref().map(|s| s.reference.clone()),
            lines,
        },
        source_status: if snapshot.is_some() {
            "pendiente_revision"
        } else {
            "sin_origen"
        }
        .into(),
        source_snapshot: snapshot,
    })
}
