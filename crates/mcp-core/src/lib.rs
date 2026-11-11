#![forbid(unsafe_code)]

mod business_registry;
mod quote_mail;
mod read_executor;
pub use business_registry::{approved_read_requirements, draft_read_tool_registry};
pub use quote_mail::QuoteMailConfirmation;
pub use read_executor::{DraftReadExecutor, ReadToolError};

use std::collections::{BTreeMap, BTreeSet};

use bastion_core::{CoreAdmin, CoreError, oauth_session_in_transaction};
use bastion_mcp::{ConsentIdentity, ConsentPolicy, ExchangePolicy};
use sqlx::{MySql, Transaction};

/// A Core permission required by a registered MCP scope. Values come only
/// from the server's trusted tool registry, never from an OAuth request.
#[derive(Clone)]
pub struct PermissionRequirement {
    pub module_id: String,
    pub scope_key: String,
    pub capability: String,
}

/// No implicit scopes: an empty registry denies all access. Each registered
/// scope can require multiple permissions, e.g. a report and its source domain.
#[derive(Clone)]
pub struct CoreScopePolicy {
    core: CoreAdmin,
    registry: BTreeMap<String, Vec<PermissionRequirement>>,
}

#[derive(Clone)]
pub struct CoreConsentPolicy;

impl ConsentPolicy for CoreConsentPolicy {
    async fn session(
        &self,
        tx: &mut Transaction<'_, MySql>,
        token: &str,
    ) -> Result<Option<ConsentIdentity>, sqlx::Error> {
        Ok(oauth_session_in_transaction(tx, token)
            .await?
            .map(|identity| ConsentIdentity {
                user_id: identity.user_id,
                session_id: identity.session_id,
            }))
    }
}

impl CoreScopePolicy {
    pub fn new(
        core: CoreAdmin,
        registry: BTreeMap<String, Vec<PermissionRequirement>>,
    ) -> Result<Self, &'static str> {
        if registry.len() > 1024
            || registry.iter().any(|(name, rules)| {
                name.is_empty()
                    || name.len() > 191
                    || rules.is_empty()
                    || rules.len() > 16
                    || rules.iter().any(|rule| {
                        rule.module_id.is_empty()
                            || rule.scope_key.is_empty()
                            || !matches!(rule.capability.as_str(), "ver" | "editar" | "eliminar")
                    })
            })
        {
            return Err("invalid trusted scope registry");
        }
        Ok(Self { core, registry })
    }
}

impl ExchangePolicy for CoreScopePolicy {
    async fn authorize(
        &self,
        tx: &mut Transaction<'_, MySql>,
        user_id: u64,
        _client_id: u64,
        scopes: &BTreeSet<String>,
    ) -> Result<bool, sqlx::Error> {
        if scopes.is_empty() || scopes.len() > 64 {
            return Ok(false);
        }
        // Role changes and permission edits lock this same role in Core.
        let role = sqlx::query_scalar::<_,u64>("SELECT r.id FROM roles r JOIN users u ON u.role_id=r.id WHERE u.id=? AND u.status='active' FOR UPDATE")
            .bind(user_id).fetch_optional(&mut **tx).await?;
        if role.is_none() {
            return Ok(false);
        }
        for scope in scopes {
            let Some(requirements) = self.registry.get(scope) else {
                return Ok(false);
            };
            for rule in requirements {
                let module = sqlx::query_as::<_,(bool,bool,bool)>("SELECT included_in_release,environment_allowed,enabled FROM module_registry WHERE module_id=? FOR UPDATE")
                    .bind(&rule.module_id).fetch_optional(&mut **tx).await?;
                if module != Some((true, true, true)) {
                    return Ok(false);
                }
                match self
                    .core
                    .authorize_in_transaction(
                        tx,
                        user_id,
                        &rule.module_id,
                        &rule.scope_key,
                        &rule.capability,
                    )
                    .await
                {
                    Ok(()) => {}
                    Err(CoreError::Internal(error)) => return Err(error),
                    Err(_) => return Ok(false),
                }
            }
        }
        Ok(true)
    }
}
