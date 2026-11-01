use serde::Deserialize;
use serde_json::{Value, json};

use crate::TransportError;
use crate::transport::StrictValue;

const VERSION: &str = "2025-11-25";
const MAX_BODY: usize = 64 * 1024;

pub struct LegacyMessage {
    pub id: Option<Value>,
    pub method: String,
    pub params: Value,
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
            || params.get("protocolVersion").and_then(Value::as_str) != Some(VERSION)
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
}
