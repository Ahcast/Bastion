use std::collections::BTreeSet;

use chrono::{NaiveDateTime, Utc};
use serde::Serialize;
use sha2::{Digest, Sha256};
use sqlx::{FromRow, MySql, QueryBuilder};

use crate::{CoreError, Database};

#[derive(Clone)]
pub struct CoreAdmin {
    database: Database,
    local_scout_enabled: bool,
}

/// Internal identity, never deserialized from API or MCP input.
#[derive(Clone, Copy, Debug)]
pub enum DomainActor {
    User(u64),
    LocalScout,
}

impl DomainActor {
    pub fn user_id(self) -> Option<u64> {
        match self {
            Self::User(id) => Some(id),
            Self::LocalScout => None,
        }
    }
    pub fn source_key(self) -> Option<&'static str> {
        match self {
            Self::User(_) => None,
            Self::LocalScout => Some("bastion-scout"),
        }
    }
}

#[derive(Debug, Serialize, FromRow)]
pub struct PersonRecord {
    pub id: u64,
    pub full_name: String,
    pub dni: Option<String>,
    pub country_code: Option<String>,
    pub gender: Option<String>,
}

#[derive(Debug, Serialize, FromRow)]
pub struct RoleRecord {
    pub id: u64,
    pub name: String,
    pub description: Option<String>,
    pub protected: bool,
}

#[derive(Debug, Serialize, FromRow)]
pub struct UserRecord {
    pub id: u64,
    pub person_id: u64,
    pub person_name: String,
    pub role_id: u64,
    pub role_name: String,
    pub email: String,
    pub status: String,
    pub created_at: NaiveDateTime,
}

#[derive(Debug, Serialize, FromRow)]
pub struct ModuleRecord {
    pub module_id: String,
    pub display_name: String,
    pub included_in_release: bool,
    pub environment_allowed: bool,
    pub enabled: bool,
    pub settings_schema_version: u32,
}

#[derive(Debug, Serialize, FromRow)]
pub struct AuditRecord {
    pub id: u64,
    pub created_at: NaiveDateTime,
    pub actor_id: Option<u64>,
    pub actor_name: Option<String>,
    pub module_id: String,
    pub action: String,
    pub target_type: String,
    pub target_id: Option<String>,
    pub result: String,
}

#[derive(Debug, Serialize, FromRow)]
pub struct MailOutboxRecord {
    pub id: u64,
    pub kind: String,
    pub recipient_email: String,
    pub delivery_status: String,
    pub attempts: u16,
    pub last_error_code: Option<String>,
    pub created_at: NaiveDateTime,
    pub sent_at: Option<NaiveDateTime>,
}

#[derive(Debug, Serialize, FromRow)]
pub struct SettingRecord {
    pub module_id: String,
    pub scope_key: String,
    pub setting_key: String,
    pub schema_version: u32,
    pub value_json: serde_json::Value,
    pub updated_at: NaiveDateTime,
}

#[derive(Clone, Debug)]
pub struct PersonInput {
    pub full_name: String,
    pub dni: Option<String>,
    pub country_code: Option<String>,
    pub gender: Option<String>,
}

#[derive(Clone, Debug, Serialize, FromRow)]
pub struct RolePermissionInput {
    pub module_id: String,
    pub scope_key: String,
    pub capability: String,
}

impl CoreAdmin {
    /// Shared business identity; callers authorize their document operation first.
    pub async fn document_company(&self) -> Result<crate::DocumentCompany, CoreError> {
        let value: Option<serde_json::Value> = sqlx::query_scalar("SELECT value_json FROM settings WHERE module_id='core' AND scope_key='global' AND setting_key='company.profile'")
            .fetch_optional(self.database.pool()).await?;
        let company: crate::DocumentCompany =
            serde_json::from_value(value.ok_or(CoreError::Invalid)?)
                .map_err(|_| CoreError::Invalid)?;
        company.validate()?;
        Ok(company)
    }
    /// Authoritative contact snapshot for an authenticated domain actor.
    pub async fn user_contact(&self, actor: u64) -> Result<(String, String), CoreError> {
        sqlx::query_as("SELECT p.full_name,u.email FROM users u JOIN persons p ON p.id=u.person_id WHERE u.id=? AND u.status='active'")
            .bind(actor).fetch_optional(self.database.pool()).await?.ok_or(CoreError::Unauthorized)
    }

    pub async fn record_domain_event(
        tx: &mut sqlx::Transaction<'_, MySql>,
        actor: u64,
        module: &str,
        action: &str,
        target: &str,
        details: serde_json::Value,
    ) -> Result<(), CoreError> {
        Self::record_actor_event(
            tx,
            DomainActor::User(actor),
            module,
            action,
            target,
            details,
        )
        .await
    }

    pub async fn record_actor_event(
        tx: &mut sqlx::Transaction<'_, MySql>,
        actor: DomainActor,
        module: &str,
        action: &str,
        target: &str,
        mut details: serde_json::Value,
    ) -> Result<(), CoreError> {
        if matches!(actor, DomainActor::LocalScout) {
            if module != "procesos" || action != "import" {
                return Err(CoreError::Forbidden);
            }
            details["actor"] = serde_json::json!("bastion-scout");
        }
        sqlx::query("INSERT INTO audit_events (actor_id,module_id,action,target_type,target_id,result,details_json,created_at) VALUES (?,?,?,'domain_record',?,'success',?,UTC_TIMESTAMP(6))")
            .bind(actor.user_id()).bind(module).bind(action).bind(target).bind(details).execute(&mut **tx).await?;
        Ok(())
    }

    pub(crate) fn new(database: Database, local_scout_enabled: bool) -> Self {
        Self {
            database,
            local_scout_enabled,
        }
    }

    pub async fn authorize_actor(
        &self,
        actor: DomainActor,
        module: &str,
        scope: &str,
        capability: &str,
    ) -> Result<(), CoreError> {
        match actor {
            DomainActor::User(id) => self.authorize(id, module, scope, capability).await,
            DomainActor::LocalScout
                if self.local_scout_enabled
                    && module == "procesos"
                    && scope == "importacion"
                    && capability == "editar" =>
            {
                Ok(())
            }
            DomainActor::LocalScout => Err(CoreError::Forbidden),
        }
    }

    pub async fn cleanup_retention(&self) -> Result<(), CoreError> {
        sqlx::query(
            "DELETE FROM audit_events WHERE created_at < UTC_TIMESTAMP(6) - INTERVAL 1 YEAR",
        )
        .execute(&self.database.pool)
        .await?;
        sqlx::query(
            "DELETE FROM security_events WHERE created_at < UTC_TIMESTAMP(6) - INTERVAL 30 DAY",
        )
        .execute(&self.database.pool)
        .await?;
        sqlx::query(
            "DELETE FROM otp_challenges WHERE expires_at < UTC_TIMESTAMP(6) - INTERVAL 1 DAY",
        )
        .execute(&self.database.pool)
        .await?;
        sqlx::query("DELETE FROM bootstrap_setups WHERE expires_at < UTC_TIMESTAMP(6) OR consumed_at IS NOT NULL").execute(&self.database.pool).await?;
        Ok(())
    }

    pub async fn list_persons(
        &self,
        actor: u64,
        query: Option<&str>,
        page: u32,
        page_size: u32,
    ) -> Result<Vec<PersonRecord>, CoreError> {
        self.authorize(actor, "core", "personas", "ver").await?;
        let (limit, offset) = page_bounds(page, page_size)?;
        let mut q = QueryBuilder::<MySql>::new(
            "SELECT id,full_name,dni,country_code,gender FROM persons WHERE 1=1",
        );
        if let Some(search) = query.filter(|s| !s.trim().is_empty()) {
            if search.chars().count() > 80 {
                return Err(CoreError::Invalid);
            }
            q.push(" AND full_name LIKE ")
                .push_bind(format!("%{}%", escape_like(search.trim())));
        }
        q.push(" ORDER BY full_name,id LIMIT ")
            .push_bind(limit)
            .push(" OFFSET ")
            .push_bind(offset);
        q.build_query_as::<PersonRecord>()
            .fetch_all(&self.database.pool)
            .await
            .map_err(Into::into)
    }

    pub async fn create_person(
        &self,
        actor: u64,
        input: PersonInput,
    ) -> Result<PersonRecord, CoreError> {
        self.authorize(actor, "core", "personas", "editar").await?;
        let input = validate_person(input)?;
        let mut tx = self.database.pool.begin().await?;
        let result =
            sqlx::query("INSERT INTO persons (full_name,dni,country_code,gender) VALUES (?,?,?,?)")
                .bind(&input.full_name)
                .bind(&input.dni)
                .bind(&input.country_code)
                .bind(&input.gender)
                .execute(&mut *tx)
                .await;
        let id = match result {
            Ok(result) => result.last_insert_id(),
            Err(error) => return Err(map_conflict(error)),
        };
        audit_tx(
            &mut tx,
            Some(actor),
            "core",
            "person.create",
            "person",
            Some(id.to_string()),
        )
        .await?;
        let person = sqlx::query_as::<_, PersonRecord>(
            "SELECT id,full_name,dni,country_code,gender FROM persons WHERE id=?",
        )
        .bind(id)
        .fetch_one(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(person)
    }

    pub async fn update_person(
        &self,
        actor: u64,
        id: u64,
        input: PersonInput,
    ) -> Result<PersonRecord, CoreError> {
        self.authorize(actor, "core", "personas", "editar").await?;
        let input = validate_person(input)?;
        let mut tx = self.database.pool.begin().await?;
        let result =
            sqlx::query("UPDATE persons SET full_name=?,dni=?,country_code=?,gender=? WHERE id=?")
                .bind(&input.full_name)
                .bind(&input.dni)
                .bind(&input.country_code)
                .bind(&input.gender)
                .bind(id)
                .execute(&mut *tx)
                .await;
        match result {
            Ok(r) if r.rows_affected() > 0 => {}
            Ok(_) => return Err(CoreError::Invalid),
            Err(error) => return Err(map_conflict(error)),
        }
        audit_tx(
            &mut tx,
            Some(actor),
            "core",
            "person.update",
            "person",
            Some(id.to_string()),
        )
        .await?;
        let person = sqlx::query_as::<_, PersonRecord>(
            "SELECT id,full_name,dni,country_code,gender FROM persons WHERE id=?",
        )
        .bind(id)
        .fetch_one(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(person)
    }

    pub async fn delete_person(&self, actor: u64, id: u64) -> Result<(), CoreError> {
        self.authorize(actor, "core", "personas", "eliminar")
            .await?;
        let mut tx = self.database.pool.begin().await?;
        let result = sqlx::query("DELETE FROM persons WHERE id=?")
            .bind(id)
            .execute(&mut *tx)
            .await;
        match result {
            Ok(r) if r.rows_affected() > 0 => {}
            Ok(_) => return Err(CoreError::Invalid),
            Err(_) => return Err(CoreError::Conflict),
        }
        audit_tx(
            &mut tx,
            Some(actor),
            "core",
            "person.delete",
            "person",
            Some(id.to_string()),
        )
        .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn list_roles(&self, actor: u64) -> Result<Vec<RoleRecord>, CoreError> {
        self.require_admin(actor).await?;
        sqlx::query_as::<_, RoleRecord>(
            "SELECT id,name,description,protected FROM roles ORDER BY protected DESC,name,id",
        )
        .fetch_all(&self.database.pool)
        .await
        .map_err(Into::into)
    }

    pub async fn list_role_permissions(
        &self,
        actor: u64,
        role_id: u64,
    ) -> Result<Vec<RolePermissionInput>, CoreError> {
        self.require_admin(actor).await?;
        let exists: Option<u64> = sqlx::query_scalar("SELECT id FROM roles WHERE id=?")
            .bind(role_id)
            .fetch_optional(&self.database.pool)
            .await?;
        if exists.is_none() {
            return Err(CoreError::Invalid);
        }
        sqlx::query_as::<_,RolePermissionInput>("SELECT module_id,scope_key,capability FROM role_permissions WHERE role_id=? ORDER BY module_id,scope_key,capability")
            .bind(role_id).fetch_all(&self.database.pool).await.map_err(Into::into)
    }

    pub async fn create_role(
        &self,
        actor: u64,
        name: &str,
        description: Option<&str>,
    ) -> Result<RoleRecord, CoreError> {
        self.require_admin(actor).await?;
        let name = valid_name(name, 100)?;
        let description = description.map(|s| s.trim()).filter(|s| !s.is_empty());
        if description.is_some_and(|s| s.len() > 500) {
            return Err(CoreError::Invalid);
        }
        let id = match sqlx::query("INSERT INTO roles (name,description,protected) VALUES (?,?,0)")
            .bind(&name)
            .bind(description)
            .execute(&self.database.pool)
            .await
        {
            Ok(r) => r.last_insert_id(),
            Err(e) => return Err(map_conflict(e)),
        };
        self.audit(Some(actor), "role.create", "role", &id.to_string())
            .await?;
        sqlx::query_as::<_, RoleRecord>(
            "SELECT id,name,description,protected FROM roles WHERE id=?",
        )
        .bind(id)
        .fetch_one(&self.database.pool)
        .await
        .map_err(Into::into)
    }

    pub async fn update_role(
        &self,
        actor: u64,
        id: u64,
        name: Option<&str>,
        description: Option<Option<&str>>,
        permissions: Option<Vec<RolePermissionInput>>,
    ) -> Result<RoleRecord, CoreError> {
        self.require_admin(actor).await?;
        let mut tx = self.database.pool.begin().await?;
        let protected: Option<bool> =
            sqlx::query_scalar("SELECT protected FROM roles WHERE id=? FOR UPDATE")
                .bind(id)
                .fetch_optional(&mut *tx)
                .await?;
        let Some(protected) = protected else {
            return Err(CoreError::Invalid);
        };
        if protected {
            return Err(CoreError::Forbidden);
        };
        let new_name = name.map(|n| valid_name(n, 100)).transpose()?;
        if let Some(Some(d)) = description {
            if d.trim().len() > 500 {
                return Err(CoreError::Invalid);
            }
        }
        if let Some(name) = new_name {
            sqlx::query("UPDATE roles SET name=? WHERE id=?")
                .bind(name)
                .bind(id)
                .execute(&mut *tx)
                .await
                .map_err(map_conflict)?;
        }
        if let Some(description) = description {
            sqlx::query("UPDATE roles SET description=? WHERE id=?")
                .bind(description.map(str::trim).filter(|s| !s.is_empty()))
                .bind(id)
                .execute(&mut *tx)
                .await?;
        }
        if let Some(permissions) = permissions {
            let existing = sqlx::query_as::<_, RolePermissionInput>(
                "SELECT module_id,scope_key,capability FROM role_permissions WHERE role_id=?",
            )
            .bind(id)
            .fetch_all(&mut *tx)
            .await?;
            let key = |permission: &RolePermissionInput| {
                (
                    permission.module_id.clone(),
                    permission.scope_key.clone(),
                    permission.capability.clone(),
                )
            };
            let existing = existing.iter().map(key).collect::<BTreeSet<_>>();
            let mut supplied = BTreeSet::new();
            for permission in &permissions {
                let identity = key(permission);
                if !supplied.insert(identity.clone()) {
                    return Err(CoreError::Invalid);
                }
                // Preserve exact legacy entries, including temporarily unavailable
                // modules. This exception cannot create or broaden a permission.
                if existing.contains(&identity) {
                    continue;
                }
                validate_permission(permission)?;
                let usable:Option<String>=sqlx::query_scalar("SELECT module_id FROM module_registry WHERE module_id=? AND included_in_release=1 AND environment_allowed=1")
                    .bind(&permission.module_id).fetch_optional(&mut *tx).await?;
                if usable.is_none() {
                    return Err(CoreError::Invalid);
                }
            }
            sqlx::query("DELETE FROM role_permissions WHERE role_id=?")
                .bind(id)
                .execute(&mut *tx)
                .await?;
            for permission in permissions {
                sqlx::query("INSERT INTO role_permissions (role_id,module_id,scope_hash,scope_key,capability) VALUES (?,?,?,?,?)")
                    .bind(id).bind(permission.module_id).bind(hash_key(&permission.scope_key)).bind(permission.scope_key).bind(permission.capability).execute(&mut *tx).await?;
            }
        }
        audit_tx(
            &mut tx,
            Some(actor),
            "core",
            "role.update",
            "role",
            Some(id.to_string()),
        )
        .await?;
        let role = sqlx::query_as::<_, RoleRecord>(
            "SELECT id,name,description,protected FROM roles WHERE id=?",
        )
        .bind(id)
        .fetch_one(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(role)
    }

    pub async fn delete_role(&self, actor: u64, id: u64) -> Result<(), CoreError> {
        self.require_admin(actor).await?;
        let mut tx = self.database.pool.begin().await?;
        let role=sqlx::query_as::<_,(bool,i64)>("SELECT r.protected,(SELECT COUNT(*) FROM users u WHERE u.role_id=r.id) FROM roles r WHERE r.id=? FOR UPDATE").bind(id).fetch_optional(&mut *tx).await?.ok_or(CoreError::Invalid)?;
        if role.0 {
            return Err(CoreError::Forbidden);
        }
        if role.1 > 0 {
            return Err(CoreError::Conflict);
        }
        sqlx::query("DELETE FROM roles WHERE id=?")
            .bind(id)
            .execute(&mut *tx)
            .await?;
        audit_tx(
            &mut tx,
            Some(actor),
            "core",
            "role.delete",
            "role",
            Some(id.to_string()),
        )
        .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn list_users(
        &self,
        actor: u64,
        query: Option<&str>,
        page: u32,
        page_size: u32,
    ) -> Result<Vec<UserRecord>, CoreError> {
        self.require_admin(actor).await?;
        let (limit, offset) = page_bounds(page, page_size)?;
        let mut q = QueryBuilder::<MySql>::new(
            "SELECT u.id,u.person_id,p.full_name AS person_name,u.role_id,r.name AS role_name,u.email,u.status,u.created_at FROM users u JOIN persons p ON p.id=u.person_id JOIN roles r ON r.id=u.role_id WHERE 1=1",
        );
        if let Some(search) = query.filter(|s| !s.trim().is_empty()) {
            if search.chars().count() > 100 {
                return Err(CoreError::Invalid);
            }
            let pattern = format!("%{}%", escape_like(search.trim()));
            q.push(" AND (p.full_name LIKE ")
                .push_bind(pattern.clone())
                .push(" OR u.email LIKE ")
                .push_bind(pattern)
                .push(")");
        }
        q.push(" ORDER BY p.full_name,u.id LIMIT ")
            .push_bind(limit)
            .push(" OFFSET ")
            .push_bind(offset);
        q.build_query_as::<UserRecord>()
            .fetch_all(&self.database.pool)
            .await
            .map_err(Into::into)
    }

    pub async fn create_user(
        &self,
        actor: u64,
        person_id: u64,
        role_id: u64,
        email: &str,
    ) -> Result<UserRecord, CoreError> {
        self.require_admin(actor).await?;
        let email = valid_email(email)?;
        let mut tx = self.database.pool.begin().await?;
        let person = sqlx::query_scalar::<_, u64>("SELECT id FROM persons WHERE id=? FOR UPDATE")
            .bind(person_id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(CoreError::Invalid)?;
        let linked: Option<u64> = sqlx::query_scalar("SELECT id FROM users WHERE person_id=?")
            .bind(person)
            .fetch_optional(&mut *tx)
            .await?;
        if linked.is_some() {
            return Err(CoreError::Conflict);
        }
        let role_exists: Option<u64> = sqlx::query_scalar("SELECT id FROM roles WHERE id=?")
            .bind(role_id)
            .fetch_optional(&mut *tx)
            .await?;
        if role_exists.is_none() {
            return Err(CoreError::Invalid);
        }
        let created=sqlx::query("INSERT INTO users (person_id,role_id,email,status,created_by) VALUES (?,?,?,'invitation_pending',?)")
            .bind(person_id).bind(role_id).bind(email).bind(actor).execute(&mut *tx).await;
        let id = match created {
            Ok(r) => r.last_insert_id(),
            Err(e) => return Err(map_conflict(e)),
        };
        audit_tx(
            &mut tx,
            Some(actor),
            "core",
            "user.create",
            "user",
            Some(id.to_string()),
        )
        .await?;
        let user=sqlx::query_as::<_,UserRecord>("SELECT u.id,u.person_id,p.full_name AS person_name,u.role_id,r.name AS role_name,u.email,u.status,u.created_at FROM users u JOIN persons p ON p.id=u.person_id JOIN roles r ON r.id=u.role_id WHERE u.id=?").bind(id).fetch_one(&mut *tx).await?;
        tx.commit().await?;
        Ok(user)
    }

    pub async fn update_user(
        &self,
        actor: u64,
        id: u64,
        person_id: Option<u64>,
        role_id: Option<u64>,
        enabled: Option<bool>,
    ) -> Result<UserRecord, CoreError> {
        self.require_admin(actor).await?;
        let mut tx = self.database.pool.begin().await?;
        let current=sqlx::query_as::<_,(u64,String,Option<String>)>("SELECT u.role_id,u.status,r.system_key FROM users u JOIN roles r ON r.id=u.role_id WHERE u.id=? FOR UPDATE").bind(id).fetch_optional(&mut *tx).await?.ok_or(CoreError::Invalid)?;
        let mut new_role = current.0;
        if let Some(role) = role_id {
            if sqlx::query_scalar::<_, u64>("SELECT id FROM roles WHERE id=?")
                .bind(role)
                .fetch_optional(&mut *tx)
                .await?
                .is_none()
            {
                return Err(CoreError::Invalid);
            }
            new_role = role;
        }
        let disabling = enabled == Some(false);
        let removing_admin = current.2.as_deref() == Some("system_administrator")
            && (disabling || new_role != current.0);
        if removing_admin {
            let remaining:i64=sqlx::query_scalar("SELECT COUNT(*) FROM users u JOIN roles r ON r.id=u.role_id WHERE u.status='active' AND r.system_key='system_administrator' AND u.id<>?").bind(id).fetch_one(&mut *tx).await?;
            if remaining < 1 {
                return Err(CoreError::Conflict);
            }
        }
        let target_status = if disabling {
            Some("disabled")
        } else if enabled == Some(true) && current.1 == "disabled" {
            Some("invitation_pending")
        } else {
            None
        };
        if let Some(person) = person_id {
            let exists: Option<u64> =
                sqlx::query_scalar("SELECT id FROM persons WHERE id=? FOR UPDATE")
                    .bind(person)
                    .fetch_optional(&mut *tx)
                    .await?;
            if exists.is_none() {
                return Err(CoreError::Invalid);
            }
            let linked: Option<u64> =
                sqlx::query_scalar("SELECT id FROM users WHERE person_id=? AND id<>?")
                    .bind(person)
                    .bind(id)
                    .fetch_optional(&mut *tx)
                    .await?;
            if linked.is_some() {
                return Err(CoreError::Conflict);
            }
            sqlx::query("UPDATE users SET person_id=? WHERE id=?")
                .bind(person)
                .bind(id)
                .execute(&mut *tx)
                .await?;
        }
        if new_role != current.0 {
            sqlx::query("UPDATE users SET role_id=? WHERE id=?")
                .bind(new_role)
                .bind(id)
                .execute(&mut *tx)
                .await?;
        }
        if let Some(status) = target_status {
            sqlx::query("UPDATE users SET status=?,password_hash=IF(?='invitation_pending',NULL,password_hash),totp_enabled=IF(?='invitation_pending',0,totp_enabled),totp_secret_enc=IF(?='invitation_pending',NULL,totp_secret_enc) WHERE id=?")
                .bind(status).bind(status).bind(status).bind(status).bind(id).execute(&mut *tx).await?;
            if status == "disabled" {
                let now = Utc::now().naive_utc();
                sqlx::query("UPDATE sessions SET revoked_at=?,revocation_reason='account_disabled' WHERE user_id=? AND revoked_at IS NULL").bind(now).bind(id).execute(&mut *tx).await?;
                sqlx::query("UPDATE trusted_devices SET revoked_at=? WHERE user_id=? AND revoked_at IS NULL").bind(now).bind(id).execute(&mut *tx).await?;
                sqlx::query("UPDATE otp_challenges SET consumed_at=? WHERE user_id=? AND consumed_at IS NULL").bind(now).bind(id).execute(&mut *tx).await?;
            }
        }
        audit_tx(
            &mut tx,
            Some(actor),
            "core",
            "user.update",
            "user",
            Some(id.to_string()),
        )
        .await?;
        let user=sqlx::query_as::<_,UserRecord>("SELECT u.id,u.person_id,p.full_name AS person_name,u.role_id,r.name AS role_name,u.email,u.status,u.created_at FROM users u JOIN persons p ON p.id=u.person_id JOIN roles r ON r.id=u.role_id WHERE u.id=?").bind(id).fetch_one(&mut *tx).await?;
        tx.commit().await?;
        Ok(user)
    }

    pub async fn list_modules(&self, actor: u64) -> Result<Vec<ModuleRecord>, CoreError> {
        self.require_admin(actor).await?;
        sqlx::query_as::<_,ModuleRecord>("SELECT module_id,display_name,included_in_release,environment_allowed,enabled,settings_schema_version FROM module_registry ORDER BY module_id").fetch_all(&self.database.pool).await.map_err(Into::into)
    }

    pub async fn set_module_enabled(
        &self,
        actor: u64,
        module_id: &str,
        enabled: bool,
    ) -> Result<(), CoreError> {
        self.require_admin(actor).await?;
        let mut tx = self.database.pool.begin().await?;
        let allowed: Option<(bool, bool, bool)> = sqlx::query_as(
            "SELECT included_in_release,environment_allowed,enabled FROM module_registry WHERE module_id=? FOR UPDATE",
        )
        .bind(module_id)
        .fetch_optional(&mut *tx)
        .await?;
        let Some((included, environment_allowed, previous)) = allowed else {
            return Err(CoreError::Invalid);
        };
        if module_id == "core" && !enabled {
            return Err(CoreError::Forbidden);
        }
        if enabled && (!included || !environment_allowed) {
            return Err(CoreError::Forbidden);
        }
        sqlx::query("UPDATE module_registry SET enabled=?,updated_by=? WHERE module_id=?")
            .bind(enabled)
            .bind(actor)
            .bind(module_id)
            .execute(&mut *tx)
            .await?;
        Self::record_domain_event(&mut tx, actor, "core", "module.toggle", module_id,
            serde_json::json!({"previous": previous, "enabled": enabled, "environment_allowed":environment_allowed})).await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn get_core_settings(&self, actor: u64) -> Result<Vec<SettingRecord>, CoreError> {
        self.require_admin(actor).await?;
        sqlx::query_as::<_,SettingRecord>("SELECT module_id,scope_key,setting_key,schema_version,value_json,updated_at FROM settings WHERE module_id='core' AND scope_key='global' ORDER BY setting_key").fetch_all(&self.database.pool).await.map_err(Into::into)
    }

    pub async fn set_core_setting(
        &self,
        actor: u64,
        key: &str,
        value: serde_json::Value,
    ) -> Result<(), CoreError> {
        self.require_admin(actor).await?;
        validate_core_setting(key, &value)?;
        if key == "smtp.designated_admin_user_id" {
            let selected:Option<u64>=sqlx::query_scalar("SELECT u.id FROM users u JOIN roles r ON r.id=u.role_id WHERE u.id=? AND u.status='active' AND r.system_key='system_administrator'")
                .bind(value.as_u64().ok_or(CoreError::Invalid)?).fetch_optional(&self.database.pool).await?;
            if selected.is_none() {
                return Err(CoreError::Invalid);
            }
        }
        let module = sqlx::query_scalar::<_, String>(
            "SELECT module_id FROM module_registry WHERE module_id='core'",
        )
        .fetch_optional(&self.database.pool)
        .await?;
        if module.is_none() {
            return Err(CoreError::Conflict);
        }
        sqlx::query("INSERT INTO settings (lookup_hash,module_id,scope_key,setting_key,schema_version,value_json,updated_by) VALUES (?,'core','global',?,1,CAST(? AS JSON),?) ON DUPLICATE KEY UPDATE value_json=VALUES(value_json),schema_version=VALUES(schema_version),updated_by=VALUES(updated_by)")
            .bind(hash_key(&format!("core\0global\0{key}"))).bind(key).bind(value.to_string()).bind(actor).execute(&self.database.pool).await?;
        self.audit(Some(actor), "setting.update", "setting", key)
            .await
    }

    pub async fn list_audit(
        &self,
        actor: u64,
        from: Option<NaiveDateTime>,
        to: Option<NaiveDateTime>,
        module: Option<&str>,
        action: Option<&str>,
        result: Option<&str>,
        page: u32,
        page_size: u32,
    ) -> Result<Vec<AuditRecord>, CoreError> {
        self.require_admin(actor).await?;
        let (limit, offset) = page_bounds(page, page_size)?;
        if from.zip(to).is_some_and(|(a, b)| a > b) {
            return Err(CoreError::Invalid);
        }
        let mut q = QueryBuilder::<MySql>::new(
            "SELECT a.id,a.created_at,a.actor_id,COALESCE(p.full_name,CASE WHEN a.actor_id IS NULL AND a.module_id='procesos' AND JSON_UNQUOTE(JSON_EXTRACT(a.details_json,'$.actor'))='bastion-scout' THEN 'bastion-scout (local)' ELSE NULL END) AS actor_name,a.module_id,a.action,a.target_type,a.target_id,a.result FROM audit_events a LEFT JOIN users u ON u.id=a.actor_id LEFT JOIN persons p ON p.id=u.person_id WHERE a.created_at>=UTC_TIMESTAMP(6)-INTERVAL 1 YEAR",
        );
        if let Some(v) = from {
            q.push(" AND a.created_at>=").push_bind(v);
        }
        if let Some(v) = to {
            q.push(" AND a.created_at<=").push_bind(v);
        }
        if let Some(v) = module.filter(|s| !s.is_empty()) {
            q.push(" AND a.module_id=").push_bind(v);
        }
        if let Some(v) = action.filter(|s| !s.is_empty()) {
            q.push(" AND a.action=").push_bind(v);
        }
        if let Some(v) = result.filter(|s| !s.is_empty()) {
            q.push(" AND a.result=").push_bind(v);
        }
        q.push(" ORDER BY a.created_at DESC,a.id DESC LIMIT ")
            .push_bind(limit)
            .push(" OFFSET ")
            .push_bind(offset);
        q.build_query_as::<AuditRecord>()
            .fetch_all(&self.database.pool)
            .await
            .map_err(Into::into)
    }

    pub async fn list_mail_outbox(
        &self,
        actor: u64,
        page: u32,
        page_size: u32,
    ) -> Result<Vec<MailOutboxRecord>, CoreError> {
        self.require_admin(actor).await?;
        let (limit, offset) = page_bounds(page, page_size)?;
        sqlx::query_as::<_,MailOutboxRecord>("SELECT id,kind,recipient_email,delivery_status,attempts,last_error_code,created_at,sent_at FROM mail_outbox ORDER BY created_at DESC,id DESC LIMIT ? OFFSET ?")
            .bind(limit).bind(offset).fetch_all(&self.database.pool).await.map_err(Into::into)
    }

    pub async fn retry_mail(&self, actor: u64, id: u64) -> Result<(), CoreError> {
        self.require_admin(actor).await?;
        let mut tx = self.database.pool.begin().await?;
        let token_id:Option<Option<u64>>=sqlx::query_scalar("SELECT account_token_id FROM mail_outbox WHERE id=? AND delivery_status='failed' FOR UPDATE")
            .bind(id).fetch_optional(&mut *tx).await?;
        let Some(token_id) = token_id else {
            return Err(CoreError::Conflict);
        };
        if let Some(token_id) = token_id {
            let usable:Option<u64>=sqlx::query_scalar("SELECT id FROM account_tokens WHERE id=? AND consumed_at IS NULL AND (expires_at IS NULL OR expires_at>UTC_TIMESTAMP(6))")
                .bind(token_id).fetch_optional(&mut *tx).await?;
            if usable.is_none() {
                return Err(CoreError::Conflict);
            }
        }
        sqlx::query("UPDATE mail_outbox SET delivery_status='pending',available_at=UTC_TIMESTAMP(6),last_error_code=NULL WHERE id=? AND delivery_status='failed'")
            .bind(id).execute(&mut *tx).await?;
        self.audit_tx(
            &mut tx,
            Some(actor),
            "mail.retry",
            "mail_outbox",
            &id.to_string(),
        )
        .await?;
        tx.commit().await?;
        Ok(())
    }

    async fn require_admin(&self, actor: u64) -> Result<(), CoreError> {
        let is_admin:Option<u64>=sqlx::query_scalar("SELECT u.id FROM users u JOIN roles r ON r.id=u.role_id WHERE u.id=? AND u.status='active' AND r.system_key='system_administrator'").bind(actor).fetch_optional(&self.database.pool).await?;
        if is_admin.is_some() {
            Ok(())
        } else {
            Err(CoreError::Forbidden)
        }
    }

    pub async fn authorize(
        &self,
        actor: u64,
        module: &str,
        scope: &str,
        capability: &str,
    ) -> Result<(), CoreError> {
        if !matches!(capability, "ver" | "editar" | "eliminar") {
            return Err(CoreError::Forbidden);
        }
        if self.require_admin(actor).await.is_ok() {
            return Ok(());
        }
        let allowed:Option<i64>=sqlx::query_scalar("SELECT 1 FROM users u JOIN role_permissions rp ON rp.role_id=u.role_id WHERE u.id=? AND u.status='active' AND rp.module_id=? AND rp.capability=? AND (rp.scope_key=? OR rp.scope_key='*') LIMIT 1")
            .bind(actor).bind(module).bind(capability).bind(scope).fetch_optional(&self.database.pool).await?;
        if allowed.is_some() {
            Ok(())
        } else {
            Err(CoreError::Forbidden)
        }
    }

    async fn audit(
        &self,
        actor: Option<u64>,
        action: &str,
        target_type: &str,
        target_id: &str,
    ) -> Result<(), CoreError> {
        sqlx::query("INSERT INTO audit_events (actor_id,module_id,action,target_type,target_id,result,created_at) VALUES (?,'core',?,?,?,'success',?)")
            .bind(actor).bind(action).bind(target_type).bind(target_id).bind(Utc::now().naive_utc()).execute(&self.database.pool).await?;
        Ok(())
    }

    async fn audit_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, MySql>,
        actor: Option<u64>,
        action: &str,
        target_type: &str,
        target_id: &str,
    ) -> Result<(), CoreError> {
        audit_tx(
            tx,
            actor,
            "core",
            action,
            target_type,
            Some(target_id.to_owned()),
        )
        .await
    }
}

async fn audit_tx(
    tx: &mut sqlx::Transaction<'_, MySql>,
    actor: Option<u64>,
    module: &str,
    action: &str,
    target_type: &str,
    target_id: Option<String>,
) -> Result<(), CoreError> {
    sqlx::query("INSERT INTO audit_events (actor_id,module_id,action,target_type,target_id,result,created_at) VALUES (?,?,?,?,?,'success',?)")
        .bind(actor).bind(module).bind(action).bind(target_type).bind(target_id).bind(Utc::now().naive_utc()).execute(&mut **tx).await?;
    Ok(())
}

fn page_bounds(page: u32, page_size: u32) -> Result<(u32, u64), CoreError> {
    let page = page.max(1);
    let size = page_size.clamp(1, 100);
    let offset = u64::from(page - 1)
        .checked_mul(u64::from(size))
        .ok_or(CoreError::Invalid)?;
    Ok((size, offset))
}
fn escape_like(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}
fn valid_name(value: &str, max: usize) -> Result<String, CoreError> {
    let value = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if value.is_empty() || value.chars().count() > max {
        Err(CoreError::Invalid)
    } else {
        Ok(value)
    }
}
fn valid_email(value: &str) -> Result<String, CoreError> {
    let value = value.trim().to_lowercase();
    if value.len() > 254
        || !value.is_ascii()
        || !value.contains('@')
        || value.chars().any(char::is_control)
    {
        Err(CoreError::Invalid)
    } else {
        Ok(value)
    }
}
fn hash_key(value: &str) -> Vec<u8> {
    Sha256::digest(value.as_bytes()).to_vec()
}
pub(crate) fn validate_person(mut input: PersonInput) -> Result<PersonInput, CoreError> {
    input.full_name = valid_name(&input.full_name, 180)?;
    input.dni = input
        .dni
        .map(|v| v.trim().to_owned())
        .filter(|v| !v.is_empty());
    input.country_code = input
        .country_code
        .map(|v| v.trim().to_ascii_uppercase())
        .filter(|v| !v.is_empty());
    input.gender = input
        .gender
        .map(|v| v.trim().to_owned())
        .filter(|v| !v.is_empty());
    if input.dni.as_ref().is_some_and(|v| v.chars().count() > 40)
        || input.country_code.as_ref().is_some_and(|v| v.len() != 2)
        || input
            .gender
            .as_ref()
            .is_some_and(|v| v.chars().count() > 32)
    {
        return Err(CoreError::Invalid);
    }
    Ok(input)
}
fn validate_permission(p: &RolePermissionInput) -> Result<(), CoreError> {
    if p.module_id.is_empty()
        || p.module_id.len() > 80
        || p.scope_key.is_empty()
        || p.scope_key.len() > 120
        || !matches!(p.capability.as_str(), "ver" | "editar" | "eliminar")
    {
        return Err(CoreError::Invalid);
    }
    Ok(())
}
fn validate_core_setting(key: &str, value: &serde_json::Value) -> Result<(), CoreError> {
    match key {
        "company.profile" => {
            let profile: crate::DocumentCompany =
                serde_json::from_value(value.clone()).map_err(|_| CoreError::Invalid)?;
            profile.validate()?;
        }
        "smtp.host" => {
            let s = value.as_str().ok_or(CoreError::Invalid)?;
            if s.is_empty()
                || s.len() > 253
                || s.contains(['/', ':', '@', '\\'])
                || s.parse::<std::net::IpAddr>().is_ok()
                || s.chars().any(char::is_whitespace)
            {
                return Err(CoreError::Invalid);
            }
        }
        "smtp.port" => {
            let n = value.as_u64().ok_or(CoreError::Invalid)?;
            if !matches!(n, 465 | 587) {
                return Err(CoreError::Invalid);
            }
        }
        "smtp.tls_mode" => {
            if !matches!(
                value.as_str(),
                Some("implicit_tls") | Some("starttls_required")
            ) {
                return Err(CoreError::Invalid);
            }
        }
        "smtp.designated_admin_user_id" => {
            if value.as_u64().is_none() {
                return Err(CoreError::Invalid);
            }
        }
        _ => return Err(CoreError::Invalid),
    }
    Ok(())
}
fn map_conflict(error: sqlx::Error) -> CoreError {
    if let sqlx::Error::Database(db) = &error {
        if db.code().as_deref() == Some("23000") {
            return CoreError::Conflict;
        }
    }
    CoreError::Internal(error)
}

#[cfg(test)]
mod permission_tests {
    use super::*;

    #[tokio::test]
    async fn mysql57_role_permissions_preserve_legacy_without_allowing_new_unavailable_grants() {
        let Ok(url) = std::env::var("BASTION_CORE_TEST_URL") else {
            eprintln!(
                "Role permission integration test requires isolated BASTION_CORE_TEST_URL; CI supplies it."
            );
            return;
        };
        let pool = sqlx::mysql::MySqlPoolOptions::new()
            .max_connections(3)
            .connect(&url)
            .await
            .unwrap();
        let database = Database { pool: pool.clone() };
        database.migrate().await.unwrap();
        let administrator_role: u64 =
            sqlx::query_scalar("SELECT id FROM roles WHERE system_key='system_administrator'")
                .fetch_one(&pool)
                .await
                .unwrap();
        let person =
            sqlx::query("INSERT INTO persons (full_name) VALUES ('Permission administrator')")
                .execute(&pool)
                .await
                .unwrap()
                .last_insert_id();
        let administrator = sqlx::query("INSERT INTO users (person_id,role_id,email,status) VALUES (?,?,'permissions-admin@example.test','active')").bind(person).bind(administrator_role).execute(&pool).await.unwrap().last_insert_id();
        let admin = CoreAdmin::new(database, false);
        let role = admin
            .create_role(administrator, "Permission matrix fixture", None)
            .await
            .unwrap();
        let person =
            sqlx::query("INSERT INTO persons (full_name) VALUES ('Permission ordinary user')")
                .execute(&pool)
                .await
                .unwrap()
                .last_insert_id();
        let ordinary = sqlx::query("INSERT INTO users (person_id,role_id,email,status) VALUES (?,?,'permissions-user@example.test','active')").bind(person).bind(role.id).execute(&pool).await.unwrap().last_insert_id();
        let permission = |module: &str, scope: &str, capability: &str| RolePermissionInput {
            module_id: module.into(),
            scope_key: scope.into(),
            capability: capability.into(),
        };
        let legacy = permission("legacy_unavailable", "archive", "download");
        sqlx::query("INSERT INTO role_permissions (role_id,module_id,scope_hash,scope_key,capability) VALUES (?,?,?,?,?)").bind(role.id).bind(&legacy.module_id).bind(hash_key(&legacy.scope_key)).bind(&legacy.scope_key).bind(&legacy.capability).execute(&pool).await.unwrap();
        let grants = vec![legacy.clone(), permission("core", "personas", "ver")];
        admin
            .update_role(administrator, role.id, None, None, Some(grants.clone()))
            .await
            .unwrap();
        assert_eq!(
            admin
                .list_role_permissions(administrator, role.id)
                .await
                .unwrap()
                .len(),
            2
        );
        assert!(matches!(
            admin.list_roles(ordinary).await,
            Err(CoreError::Forbidden)
        ));
        assert!(matches!(
            admin.list_role_permissions(ordinary, role.id).await,
            Err(CoreError::Forbidden)
        ));
        assert!(matches!(
            admin
                .update_role(ordinary, role.id, None, None, Some(vec![]))
                .await,
            Err(CoreError::Forbidden)
        ));
        assert!(matches!(
            admin
                .update_role(administrator, administrator_role, None, None, Some(vec![]))
                .await,
            Err(CoreError::Forbidden)
        ));
        for invalid in [
            vec![legacy.clone(), permission("legacy_unavailable", "*", "ver")],
            vec![permission("core", "*", "crear")],
            vec![legacy.clone(), legacy.clone()],
        ] {
            assert!(matches!(
                admin
                    .update_role(
                        administrator,
                        role.id,
                        Some("Must roll back"),
                        None,
                        Some(invalid)
                    )
                    .await,
                Err(CoreError::Invalid)
            ));
            assert_eq!(
                admin
                    .list_role_permissions(administrator, role.id)
                    .await
                    .unwrap()
                    .len(),
                2,
                "Invalid replacement must not delete the saved permissions"
            );
        }
        let name: String = sqlx::query_scalar("SELECT name FROM roles WHERE id=?")
            .bind(role.id)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(name, "Permission matrix fixture");
        assert!(matches!(
            admin
                .authorize(ordinary, "legacy_unavailable", "archive", "download")
                .await,
            Err(CoreError::Forbidden)
        ));
        admin
            .update_role(administrator, role.id, None, None, Some(vec![]))
            .await
            .unwrap();
        assert!(
            admin
                .list_role_permissions(administrator, role.id)
                .await
                .unwrap()
                .is_empty()
        );
        let audits: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM audit_events WHERE action='role.update' AND target_id=?",
        )
        .bind(role.id.to_string())
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(audits, 2);
    }
}
