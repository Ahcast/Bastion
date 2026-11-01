use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::BTreeSet;

use crate::transport::StrictValue;
use crate::{
    AuthenticatedAccess, DiscoveryError, ModernRequest, ToolRegistry, TransportError,
    dispatch_discovery,
};

const VERSION: &str = "2025-11-25";
const MAX_BODY: usize = 64 * 1024;

pub struct LegacyMessage {
    pub id: Option<Value>,
    pub method: String,
    pub params: Value,
}

pub enum LegacyDispatch {
    Response(Value),
    AcceptedNotification,
}

/// Pure legacy lifecycle/discovery responses. The caller must authenticate,
/// limit and audit first; this function does not execute business tools or
/// register an HTTP route. It holds no protocol session state.
pub fn dispatch_legacy(
    message: &LegacyMessage,
    access: &AuthenticatedAccess,
    registry: &ToolRegistry,
    enabled_modules: &BTreeSet<String>,
) -> Result<LegacyDispatch, DiscoveryError> {
    let Some(id) = message.id.as_ref() else {
        return Ok(LegacyDispatch::AcceptedNotification);
    };
    let response = match message.method.as_str() {
        "initialize" => {
            json!({"jsonrpc":"2.0","id":id,"result":{"protocolVersion":VERSION,"capabilities":{"tools":{"listChanged":false}},"serverInfo":{"name":"Bastion","version":env!("CARGO_PKG_VERSION")}}})
        }
        "ping" => json!({"jsonrpc":"2.0","id":id,"result":{}}),
        "tools/list" => {
            let request = ModernRequest {
                id: id.clone(),
                method: "tools/list".into(),
                params: message.params.clone(),
            };
            let mut response =
                dispatch_discovery(&request, access, registry, enabled_modules, false)?;
            if let Some(result) = response.get_mut("result").and_then(Value::as_object_mut) {
                result.remove("resultType");
                result.remove("_meta");
            }
            response
        }
        _ => return Err(DiscoveryError::MethodNotFound),
    };
    Ok(LegacyDispatch::Response(response))
}

/// Validates only the envelope for the 2025-11-25 profile. The HTTP boundary
/// must still authenticate every request, validate origin/content/accept,
/// apply limits and audit, then implement lifecycle and dispatch separately.
pub fn validate_legacy_message(
    protocol_header: Option<&str>,
    body: &[u8],
) -> Result<LegacyMessage, TransportError> {
    if body.is_empty() || body.len() > MAX_BODY {
        return Err(TransportError::InvalidRequest);
    }
    let mut parser = serde_json::Deserializer::from_slice(body);
    let StrictValue(value) =
        StrictValue::deserialize(&mut parser).map_err(|_| TransportError::InvalidRequest)?;
    parser.end().map_err(|_| TransportError::InvalidRequest)?;
    let object = value.as_object().ok_or(TransportError::InvalidRequest)?;
    if object.get("jsonrpc").and_then(Value::as_str) != Some("2.0")
        || object.contains_key("result")
        || object.contains_key("error")
    {
        return Err(TransportError::InvalidRequest);
    }
    let method = object
        .get("method")
        .and_then(Value::as_str)
        .filter(|m| {
            !m.is_empty() && m.len() <= 256 && m.bytes().all(|b| (0x21..=0x7e).contains(&b))
        })
        .ok_or(TransportError::InvalidRequest)?;
    let id = match object.get("id") {
        Some(id) if id.is_string() || id.as_i64().is_some() || id.as_u64().is_some() => {
            Some(id.clone())
        }
        Some(_) => return Err(TransportError::InvalidRequest),
        None => None,
    };
    let params = match object.get("params") {
        Some(value) if value.is_object() => value.clone(),
        Some(_) => return Err(TransportError::InvalidParams),
        None => json!({}),
    };
    if method == "initialize" {
        if id.is_none()
            || !params
                .get("protocolVersion")
                .and_then(Value::as_str)
                .is_some_and(|version| {
                    !version.is_empty()
                        && version.len() <= 64
                        && version.bytes().all(|byte| (0x21..=0x7e).contains(&byte))
                })
            || !params.get("capabilities").is_some_and(Value::is_object)
            || !params
                .get("clientInfo")
                .and_then(Value::as_object)
                .is_some_and(|info| {
                    ["name", "version"].iter().all(|key| {
                        info.get(*key)
                            .and_then(Value::as_str)
                            .is_some_and(|s| !s.is_empty() && s.len() <= 256)
                    })
                })
        {
            return Err(TransportError::InvalidParams);
        }
        if protocol_header.is_some_and(|header| header != VERSION) {
            return Err(TransportError::UnsupportedVersion);
        }
    } else if protocol_header != Some(VERSION) {
        return Err(TransportError::UnsupportedVersion);
    }
    Ok(LegacyMessage {
        id,
        method: method.to_owned(),
        params,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_initialize_and_stateless_follow_up_without_assuming_authentication() {
        let init = br#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"client","version":"1"}}}"#;
        let request = validate_legacy_message(None, init).unwrap();
        assert_eq!(request.method, "initialize");
        assert_eq!(request.id, Some(json!(1)));
        let unknown = br#"{"jsonrpc":"2.0","id":2,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"client","version":"1"}}}"#;
        assert!(validate_legacy_message(None, unknown).is_ok());
        let notification = br#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#;
        assert!(
            validate_legacy_message(Some(VERSION), notification)
                .unwrap()
                .id
                .is_none()
        );
        assert!(matches!(
            validate_legacy_message(None, notification),
            Err(TransportError::UnsupportedVersion)
        ));
    }

    #[test]
    fn rejects_duplicate_keys_invalid_ids_and_incomplete_initialization() {
        for body in [
            br#"{"jsonrpc":"2.0","id":1,"method":"ping","method":"tools/list"}"#.as_slice(),
            br#"{"jsonrpc":"2.0","id":null,"method":"ping"}"#.as_slice(),
            br#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25"}}"#.as_slice(),
            br#"[{"jsonrpc":"2.0","id":1,"method":"ping"}]"#.as_slice(),
        ] {
            assert!(validate_legacy_message(Some(VERSION), body).is_err());
        }
    }

    #[test]
    fn legacy_lifecycle_lists_only_visible_tools_and_accepts_notifications() {
        use crate::{ToolDescriptor, ToolRisk};
        let scopes = BTreeSet::from(["read".to_owned()]);
        let registry = ToolRegistry::new(
            vec![ToolDescriptor {
                name: "fixture.read".into(),
                module_id: "fixture".into(),
                description: "Fixture".into(),
                required_scope: "read".into(),
                input_schema: json!({"type":"object"}),
                output_schema: None,
                risk: ToolRisk::ReadOnly,
                idempotent: true,
                max_response_bytes: 1024,
                max_duration_ms: 1000,
            }],
            &scopes,
        )
        .unwrap();
        let access = AuthenticatedAccess {
            user_id: 1,
            client_id: 2,
            grant_id: 3,
            scopes,
        };
        let modules = BTreeSet::from(["fixture".to_owned()]);
        let init = validate_legacy_message(None, br#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"client","version":"1"}}}"#).unwrap();
        let LegacyDispatch::Response(initialized) =
            dispatch_legacy(&init, &access, &registry, &modules).unwrap()
        else {
            panic!("expected response");
        };
        assert_eq!(initialized["result"]["protocolVersion"], VERSION);
        let list = validate_legacy_message(
            Some(VERSION),
            br#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#,
        )
        .unwrap();
        let LegacyDispatch::Response(visible) =
            dispatch_legacy(&list, &access, &registry, &modules).unwrap()
        else {
            panic!("expected response");
        };
        assert_eq!(visible["result"]["tools"][0]["name"], "fixture.read");
        let LegacyDispatch::Response(hidden) =
            dispatch_legacy(&list, &access, &registry, &BTreeSet::new()).unwrap()
        else {
            panic!("expected response");
        };
        assert!(hidden["result"]["tools"].as_array().unwrap().is_empty());
        let notification = validate_legacy_message(
            Some(VERSION),
            br#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
        )
        .unwrap();
        assert!(matches!(
            dispatch_legacy(&notification, &access, &registry, &modules),
            Ok(LegacyDispatch::AcceptedNotification)
        ));
    }
}
