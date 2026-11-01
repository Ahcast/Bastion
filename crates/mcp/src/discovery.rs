use std::collections::BTreeSet;

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use serde_json::{Value, json};

use crate::{AuthenticatedAccess, ModernRequest, ToolRegistry};

const PAGE_SIZE: usize = 32;
const MAX_PAGE_BYTES: usize = 240_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiscoveryError {
    InvalidParams,
    MethodNotFound,
}

/// Pure discovery/listing logic. The caller must first validate the HTTP
/// envelope, authenticate the token, apply quotas and audit the invocation.
/// This does not register routes or execute business operations.
pub fn dispatch_discovery(
    request: &ModernRequest,
    access: &AuthenticatedAccess,
    registry: &ToolRegistry,
    enabled_modules: &BTreeSet<String>,
    confirmation_available: bool,
) -> Result<Value, DiscoveryError> {
    let info = json!({"name":"Bastion","version":env!("CARGO_PKG_VERSION")});
    match request.method.as_str() {
        "server/discover" => Ok(
            json!({"jsonrpc":"2.0","id":request.id,"result":{"resultType":"complete","supportedVersions":["2026-07-28"],"capabilities":{"tools":{}},"_meta":{"io.modelcontextprotocol/serverInfo":info}}}),
        ),
        "tools/list" => {
            let after = match request.params.get("cursor") {
                None => None,
                Some(Value::String(value)) => {
                    if value.len() > 172 {
                        return Err(DiscoveryError::InvalidParams);
                    }
                    let decoded = URL_SAFE_NO_PAD
                        .decode(value)
                        .map_err(|_| DiscoveryError::InvalidParams)?;
                    if URL_SAFE_NO_PAD.encode(&decoded) != *value {
                        return Err(DiscoveryError::InvalidParams);
                    }
                    let name =
                        String::from_utf8(decoded).map_err(|_| DiscoveryError::InvalidParams)?;
                    if name.is_empty()
                        || name.len() > 128
                        || !name
                            .bytes()
                            .all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b))
                    {
                        return Err(DiscoveryError::InvalidParams);
                    }
                    Some(name)
                }
                _ => return Err(DiscoveryError::InvalidParams),
            };
            let visible = registry.visible(access, enabled_modules, confirmation_available);
            let mut tools = Vec::new();
            let mut bytes = 0usize;
            let mut last = None;
            let mut more = false;
            for tool in visible {
                if after
                    .as_ref()
                    .is_some_and(|cursor| tool.name.as_str() <= cursor.as_str())
                {
                    continue;
                }
                let definition = tool.public_definition();
                let size = serde_json::to_vec(&definition)
                    .map_err(|_| DiscoveryError::InvalidParams)?
                    .len();
                if tools.len() >= PAGE_SIZE || (!tools.is_empty() && bytes + size > MAX_PAGE_BYTES)
                {
                    more = true;
                    break;
                }
                bytes += size;
                last = Some(tool.name.as_str());
                tools.push(definition);
            }
            let mut result = json!({"resultType":"complete","tools":tools,"_meta":{"io.modelcontextprotocol/serverInfo":info}});
            if more {
                result["nextCursor"] = Value::String(
                    URL_SAFE_NO_PAD.encode(last.ok_or(DiscoveryError::InvalidParams)?),
                );
            }
            Ok(json!({"jsonrpc":"2.0","id":request.id,"result":result}))
        }
        _ => Err(DiscoveryError::MethodNotFound),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ToolDescriptor, ToolRisk};
    fn tool(name: String, scope: &str) -> ToolDescriptor {
        ToolDescriptor {
            name,
            module_id: "fixture".into(),
            description: "Fixture".into(),
            required_scope: scope.into(),
            input_schema: json!({"type":"object"}),
            output_schema: None,
            risk: ToolRisk::ReadOnly,
            idempotent: true,
            max_response_bytes: 1024,
            max_duration_ms: 1000,
        }
    }
    #[test]
    fn lists_only_authorized_tools_with_stable_bounded_pages() {
        let scopes = BTreeSet::from(["read".to_owned(), "other".to_owned()]);
        let mut tools: Vec<_> = (0..40)
            .map(|n| tool(format!("tool{n:02}"), "read"))
            .collect();
        tools.push(tool("hidden".into(), "other"));
        let registry = ToolRegistry::new(tools, &scopes).unwrap();
        let access = AuthenticatedAccess {
            user_id: 1,
            client_id: 2,
            grant_id: 3,
            scopes: BTreeSet::from(["read".to_owned()]),
        };
        let modules = BTreeSet::from(["fixture".to_owned()]);
        let mut request = ModernRequest {
            id: json!(1),
            method: "tools/list".into(),
            params: json!({}),
        };
        let first = dispatch_discovery(&request, &access, &registry, &modules, false).unwrap();
        assert_eq!(first["result"]["tools"].as_array().unwrap().len(), 32);
        let cursor = first["result"]["nextCursor"].as_str().unwrap();
        request.params = json!({"cursor":cursor});
        let second = dispatch_discovery(&request, &access, &registry, &modules, false).unwrap();
        assert_eq!(second["result"]["tools"].as_array().unwrap().len(), 8);
        assert!(second["result"].get("nextCursor").is_none());
        request.params = json!({"cursor":"%%%"});
        assert!(matches!(
            dispatch_discovery(&request, &access, &registry, &modules, false),
            Err(DiscoveryError::InvalidParams)
        ));
    }
}
