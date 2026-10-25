use bastion_core::{CoreAdmin, CoreError, Database};
use sha2::{Digest, Sha256};
use sqlx::{MySql, QueryBuilder, Row, Transaction, mysql::MySqlRow};

use crate::{CatalogSupplier, Supplier, SupplierFilter, SupplierInput, SupplierPage};

const FIELDS: &str = "s.id,s.version,s.supplier_name,s.user_contact,s.email,s.rtn,s.phone_number,s.mobile_number,s.direction";

#[derive(Clone)]
pub struct SupplierService {
    database: Database,
    admin: CoreAdmin,
}

impl SupplierService {
    pub fn new(database: Database, admin: CoreAdmin) -> Self {
        Self { database, admin }
    }

    pub async fn migrate(database: &Database) -> Result<(), CoreError> {
        let mut migrator = sqlx::migrate!("./migrations");
        migrator.dangerous_set_table_name("_proveedores_migrations");
        migrator
            .run(database.pool())
            .await
            .map_err(CoreError::Migration)
    }

    pub async fn synchronize_release(&self, allowed: bool) -> Result<(), CoreError> {
        let exists: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM information_schema.tables WHERE table_schema=DATABASE() AND table_name='_proveedores_migrations'")
            .fetch_one(self.database.pool()).await?;
        let ready = if exists > 0 {
            let applied = sqlx::query_as::<_, (i64, bool, Vec<u8>)>(
                "SELECT version,success,checksum FROM _proveedores_migrations",
            )
            .fetch_all(self.database.pool())
            .await?;
            let expected = sqlx::migrate!("./migrations");
            applied.len() == expected.iter().count()
                && expected.iter().all(|migration| {
                    applied.iter().any(|(version, success, checksum)| {
                        *version == migration.version
                            && *success
                            && checksum.as_slice() == migration.checksum.as_ref()
                    })
                })
        } else {
            false
        };
        sqlx::query("INSERT INTO module_registry (module_id,display_name,included_in_release,environment_allowed,enabled) VALUES ('proveedores','Proveedores',1,?,0) ON DUPLICATE KEY UPDATE included_in_release=1,environment_allowed=VALUES(environment_allowed)")
            .bind(allowed && ready)
            .execute(self.database.pool())
            .await?;
        Ok(())
    }

    async fn begin(
        &self,
        actor: u64,
        capability: &str,
    ) -> Result<Transaction<'_, MySql>, CoreError> {
        self.admin
            .authorize(actor, "proveedores", "catalogo", capability)
            .await?;
        let mut tx = self.database.pool().begin().await?;
        let enabled: Option<bool> = sqlx::query_scalar("SELECT (included_in_release AND environment_allowed AND enabled) FROM module_registry WHERE module_id='proveedores' FOR UPDATE")
            .fetch_optional(&mut *tx)
            .await?;
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
        sqlx::query("INSERT INTO proveedores_actor_limits (actor_id,window_start,requests) VALUES (?,UTC_TIMESTAMP(6),1) ON DUPLICATE KEY UPDATE requests=IF(window_start<=UTC_TIMESTAMP(6)-INTERVAL 1 MINUTE,1,requests+1),window_start=IF(window_start<=UTC_TIMESTAMP(6)-INTERVAL 1 MINUTE,UTC_TIMESTAMP(6),window_start)")
            .bind(actor)
            .execute(&mut *tx)
            .await?;
        let count: u32 =
            sqlx::query_scalar("SELECT requests FROM proveedores_actor_limits WHERE actor_id=?")
                .bind(actor)
                .fetch_one(&mut *tx)
                .await?;
        tx.commit().await?;
        if count > 120 {
            return Err(CoreError::Locked);
        }
        Ok(())
    }

    pub async fn list(
        &self,
        actor: u64,
        filter: &SupplierFilter,
    ) -> Result<SupplierPage, CoreError> {
        let (page, page_size) = filter.bounds()?;
        let mut tx = self.begin(actor, "ver").await?;
        let rows = list_query(FIELDS, filter)?
            .build()
            .fetch_all(&mut *tx)
            .await?;
        let has_more = rows.len() > page_size as usize;
        let items = rows
            .iter()
            .take(page_size as usize)
            .map(supplier_from_row)
            .collect::<Result<_, _>>()?;
        tx.commit().await?;
        Ok(SupplierPage {
            items,
            page,
            page_size,
            has_more,
        })
    }

    pub async fn catalog(
        &self,
        actor: u64,
        filter: &SupplierFilter,
    ) -> Result<SupplierPage<CatalogSupplier>, CoreError> {
        let (page, page_size) = filter.bounds()?;
        let mut tx = self.begin(actor, "ver").await?;
        let mut items: Vec<CatalogSupplier> = list_query("s.id,s.supplier_name", filter)?
            .build_query_as()
            .fetch_all(&mut *tx)
            .await?;
        let has_more = items.len() > page_size as usize;
        items.truncate(page_size as usize);
        tx.commit().await?;
        Ok(SupplierPage {
            items,
            page,
            page_size,
            has_more,
        })
    }

    pub async fn detail(&self, actor: u64, id: u64) -> Result<Supplier, CoreError> {
        let mut tx = self.begin(actor, "ver").await?;
        let supplier = load(&mut tx, id).await?;
        tx.commit().await?;
        Ok(supplier)
    }

    pub async fn create(
        &self,
        actor: u64,
        key: &str,
        input: SupplierInput,
    ) -> Result<Supplier, CoreError> {
        let input = input.normalize()?;
        if !(16..=128).contains(&key.len())
            || !key
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
        {
            return Err(CoreError::Invalid);
        }
        let request_hash =
            Sha256::digest(serde_json::to_vec(&input).map_err(|_| CoreError::Invalid)?).to_vec();
        let key_hash = Sha256::digest(key.as_bytes()).to_vec();
        let mut tx = self.begin(actor, "editar").await?;
        let previous: Option<(Vec<u8>, serde_json::Value)> = sqlx::query_as(
            "SELECT request_hash,response_json FROM proveedores_create_receipts WHERE actor_id=? AND key_hash=?",
        )
        .bind(actor)
        .bind(&key_hash)
        .fetch_optional(&mut *tx)
        .await?;
        if let Some((old_hash, response)) = previous {
            if old_hash != request_hash {
                return Err(CoreError::Conflict);
            }
            let supplier = serde_json::from_value(response).map_err(|_| CoreError::Invalid)?;
            tx.commit().await?;
            return Ok(supplier);
        }

        let id = sqlx::query("INSERT INTO proveedores_records (supplier_name,user_contact,email,rtn,phone_number,mobile_number,direction,created_by,updated_by,created_at,updated_at) VALUES (?,?,?,?,?,?,?,?,?,UTC_TIMESTAMP(6),UTC_TIMESTAMP(6))")
            .bind(&input.supplier_name)
            .bind(&input.user_contact)
            .bind(&input.email)
            .bind(&input.rtn)
            .bind(&input.phone_number)
            .bind(&input.mobile_number)
            .bind(&input.direction)
            .bind(actor)
            .bind(actor)
            .execute(&mut *tx)
            .await?
            .last_insert_id();
        let supplier = Supplier {
            id,
            version: 1,
            data: input,
        };
        sqlx::query("INSERT INTO proveedores_create_receipts (actor_id,key_hash,request_hash,response_json) VALUES (?,?,?,?)")
            .bind(actor)
            .bind(&key_hash)
            .bind(&request_hash)
            .bind(serde_json::to_value(&supplier).map_err(|_| CoreError::Invalid)?)
            .execute(&mut *tx)
            .await?;
        CoreAdmin::record_domain_event(
            &mut tx,
            actor,
            "proveedores",
            "supplier.create",
            &id.to_string(),
            serde_json::json!({"version": supplier.version}),
        )
        .await?;
        tx.commit().await?;
        Ok(supplier)
    }

    pub async fn replace(
        &self,
        actor: u64,
        id: u64,
        expected: u64,
        input: SupplierInput,
    ) -> Result<Supplier, CoreError> {
        let input = input.normalize()?;
        let mut tx = self.begin(actor, "editar").await?;
        let current = load(&mut tx, id).await?;
        if current.version != expected {
            return Err(CoreError::PreconditionFailed);
        }
        let version = expected.checked_add(1).ok_or(CoreError::Invalid)?;
        sqlx::query("UPDATE proveedores_records SET supplier_name=?,user_contact=?,email=?,rtn=?,phone_number=?,mobile_number=?,direction=?,updated_by=?,updated_at=UTC_TIMESTAMP(6),version=? WHERE id=?")
            .bind(&input.supplier_name)
            .bind(&input.user_contact)
            .bind(&input.email)
            .bind(&input.rtn)
            .bind(&input.phone_number)
            .bind(&input.mobile_number)
            .bind(&input.direction)
            .bind(actor)
            .bind(version)
            .bind(id)
            .execute(&mut *tx)
            .await?;
        CoreAdmin::record_domain_event(
            &mut tx,
            actor,
            "proveedores",
            "supplier.update",
            &id.to_string(),
            serde_json::json!({"version": version}),
        )
        .await?;
        tx.commit().await?;
        Ok(Supplier {
            id,
            version,
            data: input,
        })
    }
}

fn list_query(fields: &str, filter: &SupplierFilter) -> Result<QueryBuilder<MySql>, CoreError> {
    let (page, size) = filter.bounds()?;
    let mut query = QueryBuilder::new(format!(
        "SELECT {fields} FROM proveedores_records s WHERE 1=1"
    ));
    if let Some(term) = filter.search() {
        query
            .push(" AND (LOCATE(")
            .push_bind(term)
            .push(",s.supplier_name)>0 OR LOCATE(")
            .push_bind(term)
            .push(",s.user_contact)>0 OR LOCATE(")
            .push_bind(term)
            .push(",s.email)>0 OR LOCATE(")
            .push_bind(term)
            .push(",s.rtn)>0 OR LOCATE(")
            .push_bind(term)
            .push(",s.phone_number)>0 OR LOCATE(")
            .push_bind(term)
            .push(",s.mobile_number)>0 OR LOCATE(")
            .push_bind(term)
            .push(",s.direction)>0)");
    }
    query
        .push(" ORDER BY s.supplier_name,s.id LIMIT ")
        .push_bind(size + 1)
        .push(" OFFSET ")
        .push_bind((page - 1) * size);
    Ok(query)
}

fn supplier_from_row(row: &MySqlRow) -> Result<Supplier, sqlx::Error> {
    Ok(Supplier {
        id: row.try_get("id")?,
        version: row.try_get("version")?,
        data: SupplierInput {
            supplier_name: row.try_get("supplier_name")?,
            user_contact: row.try_get("user_contact")?,
            email: row.try_get("email")?,
            rtn: row.try_get("rtn")?,
            phone_number: row.try_get("phone_number")?,
            mobile_number: row.try_get("mobile_number")?,
            direction: row.try_get("direction")?,
        },
    })
}

async fn load(tx: &mut Transaction<'_, MySql>, id: u64) -> Result<Supplier, CoreError> {
    let row = sqlx::query("SELECT s.id,s.version,s.supplier_name,s.user_contact,s.email,s.rtn,s.phone_number,s.mobile_number,s.direction FROM proveedores_records s WHERE s.id=?")
        .bind(id)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(CoreError::NotFound)?;
    Ok(supplier_from_row(&row)?)
}
