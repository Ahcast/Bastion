use bastion_core::{CoreAdmin, CoreError, Database};
use sha2::{Digest, Sha256};
use sqlx::{MySql, QueryBuilder, Row, Transaction, mysql::MySqlRow};

use crate::{
    CatalogClient, Client, ClientFilter, ClientInput, ClientPage, ClientWrite, canonical_name,
    name_hash, rtn_digits,
};

const FIELDS: &str = "c.id,c.version,c.cliente_name,c.user_contact,c.email,c.rtn,c.phone_number,c.mobile_number,c.direction";

#[derive(Clone)]
pub struct ClientService {
    database: Database,
    admin: CoreAdmin,
}

impl ClientService {
    pub fn new(database: Database, admin: CoreAdmin) -> Self {
        Self { database, admin }
    }

    pub async fn migrate(database: &Database) -> Result<(), CoreError> {
        let mut migrator = sqlx::migrate!("./migrations");
        migrator.dangerous_set_table_name("_clientes_migrations");
        migrator
            .run(database.pool())
            .await
            .map_err(CoreError::Migration)
    }

    pub async fn synchronize_release(&self, allowed: bool) -> Result<(), CoreError> {
        let exists: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM information_schema.tables WHERE table_schema=DATABASE() AND table_name='_clientes_migrations'").fetch_one(self.database.pool()).await?;
        let ready = if exists > 0 {
            let applied = sqlx::query_as::<_, (i64, bool, Vec<u8>)>(
                "SELECT version,success,checksum FROM _clientes_migrations",
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
        sqlx::query("INSERT INTO module_registry (module_id,display_name,included_in_release,environment_allowed,enabled) VALUES ('clientes','Clientes',1,?,0) ON DUPLICATE KEY UPDATE included_in_release=1,environment_allowed=VALUES(environment_allowed)")
            .bind(allowed && ready).execute(self.database.pool()).await?;
        Ok(())
    }

    async fn begin(
        &self,
        actor: u64,
        capability: &str,
    ) -> Result<Transaction<'_, MySql>, CoreError> {
        self.admin
            .authorize(actor, "clientes", "catalogo", capability)
            .await?;
        let mut tx = self.database.pool().begin().await?;
        // Serialize writes/merges and Core activation changes using the same row.
        let enabled: Option<bool> = sqlx::query_scalar("SELECT (included_in_release AND environment_allowed AND enabled) FROM module_registry WHERE module_id='clientes' FOR UPDATE").fetch_optional(&mut *tx).await?;
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
        sqlx::query("INSERT INTO clientes_actor_limits (actor_id,window_start,requests) VALUES (?,UTC_TIMESTAMP(6),1) ON DUPLICATE KEY UPDATE requests=IF(window_start<=UTC_TIMESTAMP(6)-INTERVAL 1 MINUTE,1,requests+1),window_start=IF(window_start<=UTC_TIMESTAMP(6)-INTERVAL 1 MINUTE,UTC_TIMESTAMP(6),window_start)")
            .bind(actor).execute(&mut *tx).await?;
        let count: u32 =
            sqlx::query_scalar("SELECT requests FROM clientes_actor_limits WHERE actor_id=?")
                .bind(actor)
                .fetch_one(&mut *tx)
                .await?;
        tx.commit().await?;
        if count > 120 {
            return Err(CoreError::Locked);
        }
        Ok(())
    }

    pub async fn list(&self, actor: u64, filter: &ClientFilter) -> Result<ClientPage, CoreError> {
        let (page, page_size) = filter.bounds()?;
        let mut tx = self.begin(actor, "ver").await?;
        let mut query = list_query(FIELDS, filter)?;
        let rows = query.build().fetch_all(&mut *tx).await?;
        let has_more = rows.len() > page_size as usize;
        let items = rows
            .iter()
            .take(page_size as usize)
            .map(client_from_row)
            .collect::<Result<_, _>>()?;
        tx.commit().await?;
        Ok(ClientPage {
            items,
            page,
            page_size,
            has_more,
        })
    }

    pub async fn catalog(
        &self,
        actor: u64,
        filter: &ClientFilter,
    ) -> Result<ClientPage<CatalogClient>, CoreError> {
        let (page, page_size) = filter.bounds()?;
        let mut tx = self.begin(actor, "ver").await?;
        let mut query = list_query("c.id,c.cliente_name,c.email,c.rtn", filter)?;
        let mut items: Vec<CatalogClient> = query.build_query_as().fetch_all(&mut *tx).await?;
        let has_more = items.len() > page_size as usize;
        items.truncate(page_size as usize);
        tx.commit().await?;
        Ok(ClientPage {
            items,
            page,
            page_size,
            has_more,
        })
    }

    pub async fn detail(&self, actor: u64, id: u64) -> Result<Client, CoreError> {
        let mut tx = self.begin(actor, "ver").await?;
        let client = load(&mut tx, id).await?;
        tx.commit().await?;
        Ok(client)
    }

    pub async fn create(
        &self,
        actor: u64,
        key: &str,
        input: ClientInput,
    ) -> Result<ClientWrite, CoreError> {
        let input = input.normalize()?;
        if !(16..=128).contains(&key.len())
            || !key
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
        {
            return Err(CoreError::Invalid);
        }
        let hash =
            Sha256::digest(serde_json::to_vec(&input).map_err(|_| CoreError::Invalid)?).to_vec();
        let key_hash = Sha256::digest(key.as_bytes()).to_vec();
        let mut tx = self.begin(actor, "editar").await?;
        let previous: Option<(Vec<u8>, serde_json::Value)> = sqlx::query_as("SELECT request_hash,response_json FROM clientes_create_receipts WHERE actor_id=? AND key_hash=?").bind(actor).bind(&key_hash).fetch_optional(&mut *tx).await?;
        if let Some((old_hash, value)) = previous {
            if old_hash != hash {
                return Err(CoreError::Conflict);
            }
            let result = serde_json::from_value(value).map_err(|_| CoreError::Invalid)?;
            tx.commit().await?;
            return Ok(result);
        }
        let mut matching: Option<u64> = sqlx::query_scalar(
            "SELECT id FROM clientes_records WHERE name_hash=? ORDER BY id LIMIT 1",
        )
        .bind(name_hash(&input.cliente_name))
        .fetch_optional(&mut *tx)
        .await?;
        let digits = rtn_digits(&input.rtn);
        if matching.is_none() && !digits.is_empty() {
            matching = sqlx::query_scalar(
                "SELECT id FROM clientes_records WHERE rtn_digits=? ORDER BY id LIMIT 1",
            )
            .bind(&digits)
            .fetch_optional(&mut *tx)
            .await?;
        }
        let client = if let Some(id) = matching {
            let current = load(&mut tx, id).await?;
            let version = current.version.checked_add(1).ok_or(CoreError::Invalid)?;
            let merged = input.merge_into(current.data);
            save(&mut tx, actor, id, version, &merged).await?;
            Client {
                id,
                version,
                data: merged,
            }
        } else {
            let id = sqlx::query("INSERT INTO clientes_records (cliente_name,user_contact,email,rtn,phone_number,mobile_number,direction,name_hash,sort_key,rtn_digits,catalog_score,created_by,updated_by,created_at,updated_at) VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,UTC_TIMESTAMP(6),UTC_TIMESTAMP(6))")
                .bind(&input.cliente_name).bind(&input.user_contact).bind(&input.email).bind(&input.rtn).bind(&input.phone_number).bind(&input.mobile_number).bind(&input.direction)
                .bind(name_hash(&input.cliente_name)).bind(canonical_name(&input.cliente_name)).bind(digits).bind(input.score()).bind(actor).bind(actor).execute(&mut *tx).await?.last_insert_id();
            Client {
                id,
                version: 1,
                data: input,
            }
        };
        let result = ClientWrite {
            client,
            merged: matching.is_some(),
        };
        sqlx::query("INSERT INTO clientes_create_receipts (actor_id,key_hash,request_hash,response_json) VALUES (?,?,?,?)").bind(actor).bind(key_hash).bind(hash).bind(serde_json::to_value(&result).map_err(|_| CoreError::Invalid)?).execute(&mut *tx).await?;
        CoreAdmin::record_domain_event(
            &mut tx,
            actor,
            "clientes",
            if result.merged {
                "client.merge"
            } else {
                "client.create"
            },
            &result.client.id.to_string(),
            serde_json::json!({"version":result.client.version}),
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
        input: ClientInput,
    ) -> Result<Client, CoreError> {
        let input = input.normalize()?;
        let mut tx = self.begin(actor, "editar").await?;
        let current = load(&mut tx, id).await?;
        if current.version != expected {
            return Err(CoreError::PreconditionFailed);
        }
        let version = expected.checked_add(1).ok_or(CoreError::Invalid)?;
        save(&mut tx, actor, id, version, &input).await?;
        CoreAdmin::record_domain_event(
            &mut tx,
            actor,
            "clientes",
            "client.update",
            &id.to_string(),
            serde_json::json!({"version":version}),
        )
        .await?;
        tx.commit().await?;
        Ok(Client {
            id,
            version,
            data: input,
        })
    }
}

fn search<'a>(query: &mut QueryBuilder<'a, MySql>, alias: &str, term: Option<&'a str>) {
    if let Some(term) = term {
        query
            .push(" AND (LOCATE(")
            .push_bind(term)
            .push(format!(",{alias}.cliente_name)>0 OR LOCATE("))
            .push_bind(term)
            .push(format!(",{alias}.email)>0 OR LOCATE("))
            .push_bind(term)
            .push(format!(",{alias}.rtn)>0)"));
    }
}

fn list_query<'a>(
    fields: &str,
    filter: &'a ClientFilter,
) -> Result<QueryBuilder<'a, MySql>, CoreError> {
    let (page, size) = filter.bounds()?;
    let mut query = QueryBuilder::new(format!("SELECT {fields} FROM clientes_records c WHERE 1=1"));
    search(&mut query, "c", filter.search());
    query.push(" AND NOT EXISTS (SELECT 1 FROM clientes_records d WHERE d.name_hash=c.name_hash");
    search(&mut query, "d", filter.search());
    query.push(" AND (d.catalog_score>c.catalog_score OR (d.catalog_score=c.catalog_score AND d.id<c.id))) ORDER BY c.sort_key,c.id LIMIT ")
        .push_bind(size + 1).push(" OFFSET ").push_bind((page - 1) * size);
    Ok(query)
}

fn client_from_row(row: &MySqlRow) -> Result<Client, sqlx::Error> {
    Ok(Client {
        id: row.try_get("id")?,
        version: row.try_get("version")?,
        data: ClientInput {
            cliente_name: row.try_get("cliente_name")?,
            user_contact: row.try_get("user_contact")?,
            email: row.try_get("email")?,
            rtn: row.try_get("rtn")?,
            phone_number: row.try_get("phone_number")?,
            mobile_number: row.try_get("mobile_number")?,
            direction: row.try_get("direction")?,
        },
    })
}

async fn load(tx: &mut Transaction<'_, MySql>, id: u64) -> Result<Client, CoreError> {
    let row = sqlx::query(&format!(
        "SELECT {FIELDS} FROM clientes_records c WHERE c.id=?"
    ))
    .bind(id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(CoreError::NotFound)?;
    Ok(client_from_row(&row)?)
}

async fn save(
    tx: &mut Transaction<'_, MySql>,
    actor: u64,
    id: u64,
    version: u64,
    input: &ClientInput,
) -> Result<(), CoreError> {
    sqlx::query("UPDATE clientes_records SET cliente_name=?,user_contact=?,email=?,rtn=?,phone_number=?,mobile_number=?,direction=?,name_hash=?,sort_key=?,rtn_digits=?,catalog_score=?,updated_by=?,updated_at=UTC_TIMESTAMP(6),version=? WHERE id=?")
        .bind(&input.cliente_name).bind(&input.user_contact).bind(&input.email).bind(&input.rtn).bind(&input.phone_number).bind(&input.mobile_number).bind(&input.direction)
        .bind(name_hash(&input.cliente_name)).bind(canonical_name(&input.cliente_name)).bind(rtn_digits(&input.rtn)).bind(input.score()).bind(actor).bind(version).bind(id).execute(&mut **tx).await?;
    Ok(())
}
