use std::{collections::BTreeSet, fmt};

use base64::{Engine, engine::general_purpose::STANDARD};
use serde::de::{self, Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};
use serde_json::{Map, Number, Value};

const MAX_BODY: usize = 64 * 1024;
const VERSION: &str = "2026-07-28";

/// HTTP integration must reject duplicate header fields before constructing
/// this value and authenticate the bearer token before dispatching a request.
pub struct ModernHeaders<'a> {
    pub origin: Option<&'a str>,
    pub protocol_version: Option<&'a str>,
    pub method: Option<&'a str>,
    pub name: Option<&'a str>,
    pub accepts_json: bool,
    pub accepts_event_stream: bool,
}

pub struct ModernRequest {
    pub id: Value,
    pub method: String,
    pub params: Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportError {
    ForbiddenOrigin,
    InvalidRequest,
    InvalidParams,
    HeaderMismatch,
    UnsupportedVersion,
    Unacceptable,
}

/// Extract an unambiguous request ID for a protocol error without accepting
/// duplicate JSON keys or oversized payloads.
pub fn extract_request_id(body: &[u8]) -> Option<Value> {
    if body.is_empty() || body.len() > MAX_BODY {
        return None;
    }
    let mut parser = serde_json::Deserializer::from_slice(body);
    let StrictValue(value) = StrictValue::deserialize(&mut parser).ok()?;
    parser.end().ok()?;
    value
        .as_object()?
        .get("id")
        .filter(|id| id.is_string() || id.as_i64().is_some() || id.as_u64().is_some())
        .cloned()
}

/// Validate a single modern JSON-RPC request. This is not an HTTP route and
/// does not perform authentication, quotas, tool dispatch or object checks.
pub fn validate_modern_request(
    headers: &ModernHeaders<'_>,
    allowed_origins: &BTreeSet<String>,
    body: &[u8],
) -> Result<ModernRequest, TransportError> {
    if headers
        .origin
        .is_some_and(|origin| !allowed_origins.contains(origin))
    {
        return Err(TransportError::ForbiddenOrigin);
    }
    if !headers.accepts_json || !headers.accepts_event_stream {
        return Err(TransportError::Unacceptable);
    }
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
    let id = object
        .get("id")
        .filter(|id| id.is_string() || id.as_i64().is_some() || id.as_u64().is_some())
        .ok_or(TransportError::InvalidRequest)?
        .clone();
    let method = object
        .get("method")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty() && s.len() <= 256)
        .ok_or(TransportError::InvalidRequest)?;
    if !method.bytes().all(|b| (0x21..=0x7e).contains(&b)) {
        return Err(TransportError::HeaderMismatch);
    }
    let params = object
        .get("params")
        .filter(|v| v.is_object())
        .ok_or(TransportError::InvalidParams)?;
    let meta = params
        .get("_meta")
        .and_then(Value::as_object)
        .ok_or(TransportError::InvalidParams)?;
    let version = meta
        .get("io.modelcontextprotocol/protocolVersion")
        .and_then(Value::as_str)
        .ok_or(TransportError::InvalidParams)?;
    if !meta
        .get("io.modelcontextprotocol/clientCapabilities")
        .is_some_and(Value::is_object)
    {
        return Err(TransportError::InvalidParams);
    }
    if let Some(info) = meta.get("io.modelcontextprotocol/clientInfo") {
        let valid = info.as_object().is_some_and(|object| {
            ["name", "version"].iter().all(|key| {
                object
                    .get(*key)
                    .and_then(Value::as_str)
                    .is_some_and(|value| !value.is_empty())
            })
        });
        if !valid {
            return Err(TransportError::InvalidParams);
        }
    }
    if headers.protocol_version != Some(version) || headers.method != Some(method) {
        return Err(TransportError::HeaderMismatch);
    }
    if version != VERSION {
        return Err(TransportError::UnsupportedVersion);
    }
    let field = match method {
        "tools/call" | "prompts/get" => Some("name"),
        "resources/read" => Some("uri"),
        _ => None,
    };
    if let Some(field) = field {
        let name = params
            .get(field)
            .and_then(Value::as_str)
            .ok_or(TransportError::InvalidParams)?;
        let header = headers.name.ok_or(TransportError::HeaderMismatch)?;
        if decode_name(header).as_deref() != Some(name) {
            return Err(TransportError::HeaderMismatch);
        }
    }
    Ok(ModernRequest {
        id,
        method: method.to_owned(),
        params: params.clone(),
    })
}

fn decode_name(value: &str) -> Option<String> {
    if value.len() > 4096
        || value
            .bytes()
            .any(|b| !(b == b'\t' || (0x20..=0x7e).contains(&b)))
    {
        return None;
    }
    if let Some(encoded) = value
        .strip_prefix("=?base64?")
        .and_then(|v| v.strip_suffix("?="))
    {
        let bytes = STANDARD.decode(encoded).ok()?;
        if STANDARD.encode(&bytes) != encoded {
            return None;
        }
        return String::from_utf8(bytes).ok();
    }
    if value.trim_matches([' ', '\t']) != value {
        return None;
    }
    Some(value.to_owned())
}

// Reject duplicate keys at every JSON depth so that an intermediary and the
// server cannot interpret different method, version or argument values.
struct StrictValue(Value);
impl<'de> Deserialize<'de> for StrictValue {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        d.deserialize_any(StrictVisitor)
    }
}
struct StrictVisitor;
impl<'de> Visitor<'de> for StrictVisitor {
    type Value = StrictValue;
    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a JSON value without duplicate keys")
    }
    fn visit_bool<E: de::Error>(self, v: bool) -> Result<Self::Value, E> {
        Ok(StrictValue(Value::Bool(v)))
    }
    fn visit_i64<E: de::Error>(self, v: i64) -> Result<Self::Value, E> {
        Ok(StrictValue(Value::Number(v.into())))
    }
    fn visit_u64<E: de::Error>(self, v: u64) -> Result<Self::Value, E> {
        Ok(StrictValue(Value::Number(v.into())))
    }
    fn visit_f64<E: de::Error>(self, v: f64) -> Result<Self::Value, E> {
        Number::from_f64(v)
            .map(|n| StrictValue(Value::Number(n)))
            .ok_or_else(|| E::custom("invalid number"))
    }
    fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
        Ok(StrictValue(Value::String(v.to_owned())))
    }
    fn visit_string<E: de::Error>(self, v: String) -> Result<Self::Value, E> {
        Ok(StrictValue(Value::String(v)))
    }
    fn visit_none<E: de::Error>(self) -> Result<Self::Value, E> {
        Ok(StrictValue(Value::Null))
    }
    fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
        Ok(StrictValue(Value::Null))
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
        let mut values = Vec::new();
        while let Some(StrictValue(v)) = seq.next_element()? {
            values.push(v);
        }
        Ok(StrictValue(Value::Array(values)))
    }
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
        let mut values = Map::new();
        while let Some((key, StrictValue(value))) = map.next_entry::<String, StrictValue>()? {
            if values.insert(key, value).is_some() {
                return Err(de::Error::custom("duplicate JSON key"));
            }
        }
        Ok(StrictValue(Value::Object(values)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const BODY:&[u8]=br#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"buscar","arguments":{},"_meta":{"io.modelcontextprotocol/protocolVersion":"2026-07-28","io.modelcontextprotocol/clientCapabilities":{}}}}"#;
    fn headers<'a>() -> ModernHeaders<'a> {
        ModernHeaders {
            origin: None,
            protocol_version: Some(VERSION),
            method: Some("tools/call"),
            name: Some("buscar"),
            accepts_json: true,
            accepts_event_stream: true,
        }
    }
    #[test]
    fn validates_matching_headers_and_rejects_mismatches() {
        let allowed = BTreeSet::new();
        assert!(validate_modern_request(&headers(), &allowed, BODY).is_ok());
        let mut h = headers();
        h.name = Some("otro");
        assert!(matches!(
            validate_modern_request(&h, &allowed, BODY),
            Err(TransportError::HeaderMismatch)
        ));
        h = headers();
        h.origin = Some("https://evil.example");
        assert!(matches!(
            validate_modern_request(&h, &allowed, BODY),
            Err(TransportError::ForbiddenOrigin)
        ));
        h = headers();
        h.accepts_event_stream = false;
        assert!(matches!(
            validate_modern_request(&h, &allowed, BODY),
            Err(TransportError::Unacceptable)
        ));
    }
    #[test]
    fn rejects_duplicate_json_and_accepts_encoded_name() {
        let allowed = BTreeSet::new();
        let text = std::str::from_utf8(BODY).unwrap();
        let duplicate = text.replace(
            "\"name\":\"buscar\"",
            "\"name\":\"buscar\",\"name\":\"otro\"",
        );
        assert!(matches!(
            validate_modern_request(&headers(), &allowed, duplicate.as_bytes()),
            Err(TransportError::InvalidRequest)
        ));
        let duplicate_meta=text.replace("\"io.modelcontextprotocol/protocolVersion\":\"2026-07-28\"","\"io.modelcontextprotocol/protocolVersion\":\"2026-07-28\",\"io.modelcontextprotocol/protocolVersion\":\"other\"");
        assert!(matches!(
            validate_modern_request(&headers(), &allowed, duplicate_meta.as_bytes()),
            Err(TransportError::InvalidRequest)
        ));
        let missing_capabilities =
            text.replace(",\"io.modelcontextprotocol/clientCapabilities\":{}", "");
        assert!(matches!(
            validate_modern_request(&headers(), &allowed, missing_capabilities.as_bytes()),
            Err(TransportError::InvalidParams)
        ));
        let encoded = "=?base64?YnVzY2Fy?=";
        let mut h = headers();
        h.name = Some(encoded);
        assert!(validate_modern_request(&h, &allowed, BODY).is_ok());
        h.protocol_version = Some("2025-11-25");
        assert!(matches!(
            validate_modern_request(&h, &allowed, BODY),
            Err(TransportError::HeaderMismatch)
        ));
    }
}
