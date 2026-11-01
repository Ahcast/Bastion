use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Value, json};

use crate::AuthenticatedAccess;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolRisk {
    ReadOnly,
    Mutation,
    Sensitive,
}

/// A server-owned descriptor; it never supplies SQL, shell, or arbitrary file
/// access. The domain handler must validate arguments and authorize the object.
pub struct ToolDescriptor {
    pub name: String,
    pub module_id: String,
    pub description: String,
    pub required_scope: String,
    pub input_schema: Value,
    pub output_schema: Option<Value>,
    pub risk: ToolRisk,
    pub idempotent: bool,
    pub max_response_bytes: usize,
    pub max_duration_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegistryError {
    Duplicate,
    Invalid,
    UnknownScope,
}

pub struct ToolRegistry {
    tools: BTreeMap<String, ToolDescriptor>,
}

impl ToolRegistry {
    /// `registered_scopes` and all descriptors come from the trusted release
    /// registry; none may be sourced from an OAuth client.
    pub fn new(
        tools: Vec<ToolDescriptor>,
        registered_scopes: &BTreeSet<String>,
    ) -> Result<Self, RegistryError> {
        if tools.len() > 256 {
            return Err(RegistryError::Invalid);
        }
        let mut index = BTreeMap::new();
        for tool in tools {
            if tool.name.is_empty()
                || tool.name.len() > 128
                || !tool
                    .name
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b))
                || tool.module_id.is_empty()
                || tool.module_id.len() > 80
                || tool.description.is_empty()
                || tool.description.len() > 2048
                || tool.input_schema.get("type").and_then(Value::as_str) != Some("object")
                || serde_json::to_vec(&tool.input_schema).map_or(true, |bytes| bytes.len() > 4096)
                || tool.output_schema.as_ref().is_some_and(|v| !v.is_object())
                || tool
                    .output_schema
                    .as_ref()
                    .is_some_and(|v| serde_json::to_vec(v).map_or(true, |bytes| bytes.len() > 4096))
                || !(1..=1_048_576).contains(&tool.max_response_bytes)
                || !(1..=120_000).contains(&tool.max_duration_ms)
            {
                return Err(RegistryError::Invalid);
            }
            if !registered_scopes.contains(&tool.required_scope) {
                return Err(RegistryError::UnknownScope);
            }
            if index.insert(tool.name.clone(), tool).is_some() {
                return Err(RegistryError::Duplicate);
            }
        }
        Ok(Self { tools: index })
    }

    /// Enumerate only tools enabled for this invocation and delegated scopes.
    /// Sensitive mutations remain hidden unless the human confirmation
    /// mechanism is available and enforced by the dispatch layer.
    pub fn visible<'a>(
        &'a self,
        access: &AuthenticatedAccess,
        enabled_modules: &BTreeSet<String>,
        confirmation_available: bool,
    ) -> Vec<&'a ToolDescriptor> {
        self.tools
            .values()
            .filter(|tool| {
                access.scopes.contains(&tool.required_scope)
                    && enabled_modules.contains(&tool.module_id)
                    && (tool.risk != ToolRisk::Sensitive || confirmation_available)
            })
            .collect()
    }

    pub fn find_visible<'a>(
        &'a self,
        name: &str,
        access: &AuthenticatedAccess,
        enabled_modules: &BTreeSet<String>,
        confirmation_available: bool,
    ) -> Option<&'a ToolDescriptor> {
        self.visible(access, enabled_modules, confirmation_available)
            .into_iter()
            .find(|tool| tool.name == name)
    }
}

impl ToolDescriptor {
    /// MCP presentation contains schemas and annotations, never internal
    /// actor IDs, quota configuration, confirmation handles or secrets.
    pub fn public_definition(&self) -> Value {
        let mut value = json!({"name":self.name,"description":self.description,"inputSchema":self.input_schema,"annotations":{"readOnlyHint":self.risk==ToolRisk::ReadOnly,"idempotentHint":self.idempotent}});
        if let Some(schema) = &self.output_schema {
            value["outputSchema"] = schema.clone();
        }
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tool(name: &str, risk: ToolRisk) -> ToolDescriptor {
        ToolDescriptor {
            name: name.into(),
            module_id: "fixture".into(),
            description: "Fixture".into(),
            required_scope: "fixture:read".into(),
            input_schema: json!({"type":"object","properties":{}}),
            output_schema: None,
            risk,
            idempotent: true,
            max_response_bytes: 1024,
            max_duration_ms: 1000,
        }
    }
    fn actor(scopes: &[&str]) -> AuthenticatedAccess {
        AuthenticatedAccess {
            user_id: 1,
            client_id: 2,
            grant_id: 3,
            scopes: scopes.iter().map(|s| (*s).to_owned()).collect(),
        }
    }

    #[test]
    fn enumeration_requires_scope_module_and_confirmation() {
        let scopes = BTreeSet::from(["fixture:read".to_owned()]);
        let modules = BTreeSet::from(["fixture".to_owned()]);
        let registry = ToolRegistry::new(
            vec![
                tool("fixture.read", ToolRisk::ReadOnly),
                tool("fixture.remove", ToolRisk::Sensitive),
            ],
            &scopes,
        )
        .unwrap();
        assert_eq!(registry.visible(&actor(&[]), &modules, true).len(), 0);
        assert_eq!(
            registry
                .visible(&actor(&["fixture:read"]), &BTreeSet::new(), true)
                .len(),
            0
        );
        assert_eq!(
            registry
                .visible(&actor(&["fixture:read"]), &modules, false)
                .len(),
            1
        );
        assert_eq!(
            registry
                .visible(&actor(&["fixture:read"]), &modules, true)
                .len(),
            2
        );
        assert!(
            registry
                .find_visible("fixture.remove", &actor(&["fixture:read"]), &modules, false)
                .is_none()
        );
    }

    #[test]
    fn rejects_undefined_scopes_duplicates_and_unbounded_tools() {
        let scopes = BTreeSet::from(["fixture:read".to_owned()]);
        assert!(matches!(
            ToolRegistry::new(
                vec![tool("fixture.read", ToolRisk::ReadOnly)],
                &BTreeSet::new()
            ),
            Err(RegistryError::UnknownScope)
        ));
        assert!(matches!(
            ToolRegistry::new(
                vec![
                    tool("same", ToolRisk::ReadOnly),
                    tool("same", ToolRisk::ReadOnly)
                ],
                &scopes
            ),
            Err(RegistryError::Duplicate)
        ));
        let mut invalid = tool("fixture.read", ToolRisk::ReadOnly);
        invalid.max_duration_ms = 0;
        assert!(matches!(
            ToolRegistry::new(vec![invalid], &scopes),
            Err(RegistryError::Invalid)
        ));
        let mut oversized = tool("fixture.large", ToolRisk::ReadOnly);
        oversized.input_schema = json!({"type":"object","description":"x".repeat(5000)});
        assert!(matches!(
            ToolRegistry::new(vec![oversized], &scopes),
            Err(RegistryError::Invalid)
        ));
    }
}
