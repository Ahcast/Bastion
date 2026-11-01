use std::collections::{BTreeMap, BTreeSet};

use bastion_mcp::{RegistryError, ToolDescriptor, ToolRegistry, ToolRisk};
use serde_json::json;

use crate::PermissionRequirement;

/// User-approved read-only modules. A scope does not grant permissions: the
/// Core still checks the user's current `documentos/ver` permission, module
/// status and grant on every invocation.
pub fn approved_read_requirements() -> BTreeMap<String, Vec<PermissionRequirement>> {
    ["cotizaciones", "facturas", "ordenes_compra"]
        .into_iter()
        .map(|module| {
            (
                format!("{module}:read"),
                vec![PermissionRequirement {
                    module_id: module.into(),
                    scope_key: "documentos".into(),
                    capability: "ver".into(),
                }],
            )
        })
        .collect()
}

/// Descriptors only. Do not add these to a live router until typed dispatch,
/// per-object authorization, quotas, output limits and auditing are wired.
pub fn draft_read_tool_registry() -> Result<ToolRegistry, RegistryError> {
    let modules = [
        ("cotizaciones", "cotizaciones"),
        ("facturas", "facturas"),
        ("ordenes_compra", "órdenes de compra"),
    ];
    let mut tools = Vec::new();
    for (module, label) in modules {
        let scope = format!("{module}:read");
        tools.push(ToolDescriptor {
            name: format!("{module}.listar"), module_id:module.into(),
            description:format!("Consultar un listado paginado de {label} autorizado para el usuario."),
            required_scope:scope.clone(),
            input_schema:json!({"type":"object","properties":{"search":{"type":"string","maxLength":200},"page":{"type":"integer","minimum":1,"maximum":10000},"page_size":{"type":"integer","minimum":1,"maximum":25}},"additionalProperties":false}),
            output_schema:None,risk:ToolRisk::ReadOnly,idempotent:true,max_response_bytes:262144,max_duration_ms:10000,
        });
        tools.push(ToolDescriptor {
            name:format!("{module}.consultar"),module_id:module.into(),
            description:format!("Consultar una de las {label} por su identificador, sujeto a los permisos del usuario."),
            required_scope:scope,
            input_schema:json!({"type":"object","properties":{"id":{"type":"integer","minimum":1,"maximum":9007199254740991}},"required":["id"],"additionalProperties":false}),
            output_schema:None,risk:ToolRisk::ReadOnly,idempotent:true,max_response_bytes:262144,max_duration_ms:10000,
        });
    }
    let scopes: BTreeSet<String> = approved_read_requirements().into_keys().collect();
    ToolRegistry::new(tools, &scopes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bastion_mcp::AuthenticatedAccess;

    #[test]
    fn exposes_only_approved_read_tools_for_active_modules_and_scopes() {
        let requirements = approved_read_requirements();
        assert_eq!(requirements.len(), 3);
        assert!(
            requirements
                .values()
                .flatten()
                .all(|r| r.scope_key == "documentos" && r.capability == "ver")
        );
        let registry = draft_read_tool_registry().unwrap();
        let user = AuthenticatedAccess {
            user_id: 1,
            client_id: 2,
            grant_id: 3,
            scopes: BTreeSet::from(["cotizaciones:read".into(), "facturas:read".into()]),
        };
        let modules = BTreeSet::from(["cotizaciones".into(), "ordenes_compra".into()]);
        let visible = registry.visible(&user, &modules, false);
        assert_eq!(visible.len(), 2);
        assert!(
            visible
                .iter()
                .all(|t| t.module_id == "cotizaciones" && t.risk == ToolRisk::ReadOnly)
        );
        assert!(registry.visible(&user, &BTreeSet::new(), false).is_empty());
    }
}
