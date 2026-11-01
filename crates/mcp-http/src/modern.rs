use std::{collections::BTreeSet, sync::Arc, time::Duration};

use crate::{MetadataIntegrity, MetadataIntegrityError};
use axum::{
    Json, Router,
    body::Bytes,
    extract::{DefaultBodyLimit, State},
    http::{
        HeaderMap, HeaderValue, StatusCode,
        header::{ACCEPT, CACHE_CONTROL, CONTENT_TYPE, RETRY_AFTER, WWW_AUTHENTICATE},
    },
    response::{IntoResponse, Response},
    routing::post,
};
use bastion_mcp::{
    Credential, DiscoveryError, ExchangePolicy, LegacyDispatch, LegacyMessage, ModernHeaders,
    ModernRequest, MysqlQuota, MysqlTokenExchange, QuotaError, ToolQuota, ToolRegistry,
    TransportError, dispatch_discovery, dispatch_legacy, extract_request_id,
    validate_legacy_message, validate_modern_request,
};
use bastion_mcp_core::{DraftReadExecutor, ReadToolError};
use serde_json::{Value, json};
use sqlx::MySqlPool;
use tokio::sync::Semaphore;
use url::Url;

const MAX_BODY: usize = 64 * 1024;

enum DraftRequest {
    Modern(ModernRequest),
    Legacy(LegacyMessage),
}

impl DraftRequest {
    fn method(&self) -> &str {
        match self {
            Self::Modern(r) => &r.method,
            Self::Legacy(r) => &r.method,
        }
    }
    fn id(&self) -> Option<&Value> {
        match self {
            Self::Modern(r) => Some(&r.id),
            Self::Legacy(r) => r.id.as_ref(),
        }
    }
    fn params(&self) -> &Value {
        match self {
            Self::Modern(r) => &r.params,
            Self::Legacy(r) => &r.params,
        }
    }
}

#[derive(Clone)]
pub struct DraftMcpState<P> {
    pool: MySqlPool,
    exchange: MysqlTokenExchange<P>,
    resource: String,
    challenge: HeaderValue,
    origins: BTreeSet<String>,
    registry: Arc<ToolRegistry>,
    read_executor: Option<Arc<DraftReadExecutor>>,
    integrity: Option<Arc<dyn MetadataIntegrity>>,
    quota: MysqlQuota,
    discovery_quota: ToolQuota,
    slots: Arc<Semaphore>,
    timeout: Duration,
}

#[derive(Debug)]
pub struct McpConfigurationError;

impl<P> DraftMcpState<P> {
    /// All inputs are server-owned. No domain tools or routes are enabled by
    /// this constructor. Production also needs IP limits and protocol coverage.
    pub fn new(
        pool: MySqlPool,
        exchange: MysqlTokenExchange<P>,
        resource: String,
        resource_metadata: String,
        origins: BTreeSet<String>,
        registry: Arc<ToolRegistry>,
        quota: MysqlQuota,
        discovery_quota: ToolQuota,
        max_concurrent: usize,
        timeout: Duration,
    ) -> Result<Self, McpConfigurationError> {
        let valid_https = |value: &str| {
            value
                .bytes()
                .all(|b| (0x21..=0x7e).contains(&b) && b != b'"' && b != b'\\')
                && Url::parse(value).is_ok_and(|url| {
                    url.scheme() == "https"
                        && url.host().is_some()
                        && url.username().is_empty()
                        && url.password().is_none()
                        && url.fragment().is_none()
                })
        };
        let valid_origin = |value: &str| {
            valid_https(value)
                && Url::parse(value).is_ok_and(|url| {
                    url.path() == "/"
                        && url.query().is_none()
                        && url.origin().ascii_serialization() == value
                })
        };
        if !valid_https(&resource)
            || !valid_https(&resource_metadata)
            || origins.iter().any(|origin| !valid_origin(origin))
            || max_concurrent == 0
            || max_concurrent > 128
            || timeout.is_zero()
            || timeout > Duration::from_secs(120)
        {
            return Err(McpConfigurationError);
        }
        let challenge =
            HeaderValue::from_str(&format!("Bearer resource_metadata=\"{resource_metadata}\""))
                .map_err(|_| McpConfigurationError)?;
        Ok(Self {
            pool,
            exchange,
            resource,
            challenge,
            origins,
            registry,
            read_executor: None,
            integrity: None,
            quota,
            discovery_quota,
            slots: Arc::new(Semaphore::new(max_concurrent)),
            timeout,
        })
    }

    /// Draft integration only. A production route also needs edge protection,
    /// OAuth consent, terminal audit on timeout, and interoperable clients.
    pub fn with_read_executor(mut self, executor: Arc<DraftReadExecutor>) -> Self {
        self.read_executor = Some(executor);
        self
    }
    pub fn with_integrity_guard(mut self, guard: Arc<dyn MetadataIntegrity>) -> Self {
        self.integrity = Some(guard);
        self
    }
}

/// Isolated draft. Do not mount before OAuth discovery, browser consent,
/// per-IP protection, compatibility and end-to-end verification are complete.
pub fn draft_mcp_router<P>(state: DraftMcpState<P>) -> Router
where
    P: ExchangePolicy + Clone + Send + Sync + 'static,
{
    Router::new()
        .route("/mcp", post(mcp::<P>))
        .layer(DefaultBodyLimit::max(MAX_BODY))
        .with_state(state)
}

fn rpc_error(status: StatusCode, code: i32, message: &'static str) -> Response {
    (
        status,
        [(CACHE_CONTROL, "no-store")],
        Json(json!({"jsonrpc":"2.0","error":{"code":code,"message":message}})),
    )
        .into_response()
}

fn protocol_error(
    status: StatusCode,
    code: i32,
    message: &'static str,
    id: Option<Value>,
    data: Option<Value>,
) -> Response {
    let mut response = json!({"jsonrpc":"2.0","error":{"code":code,"message":message}});
    if let Some(id) = id {
        response["id"] = id;
    }
    if let Some(data) = data {
        response["error"]["data"] = data;
    }
    (status, [(CACHE_CONTROL, "no-store")], Json(response)).into_response()
}

fn accepts(headers: &HeaderMap, desired: &str) -> bool {
    headers
        .get_all(ACCEPT)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(','))
        .any(|part| {
            let mut pieces = part.trim().split(';');
            let mime = pieces.next().unwrap_or("").trim();
            let mut quality = None;
            for p in pieces {
                let Some((key, q)) = p.trim().split_once('=') else {
                    return false;
                };
                if key.trim().eq_ignore_ascii_case("q") {
                    if quality.is_some() {
                        return false;
                    }
                    quality = q.trim().parse::<f32>().ok();
                    if quality.is_none() {
                        return false;
                    }
                }
            }
            mime.eq_ignore_ascii_case(desired) && quality.is_none_or(|q| q > 0.0 && q <= 1.0)
        })
}

fn single<'a>(headers: &'a HeaderMap, name: &str) -> Result<Option<&'a str>, ()> {
    let mut values = headers.get_all(name).iter();
    let Some(value) = values.next() else {
        return Ok(None);
    };
    if values.next().is_some() {
        return Err(());
    }
    value.to_str().map(Some).map_err(|_| ())
}

async fn audit(
    pool: &MySqlPool,
    actor: u64,
    client: u64,
    grant: u64,
    request_id: &str,
    action: &str,
    result: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query("INSERT INTO audit_events (actor_id,module_id,action,target_type,target_id,result,details_json,created_at) VALUES (?,'core',?,'mcp',NULL,?,JSON_OBJECT('correlation_id',?,'client_id',?,'grant_id',?),UTC_TIMESTAMP(6))")
        .bind(actor).bind(action).bind(result).bind(request_id).bind(client).bind(grant).execute(pool).await?;
    Ok(())
}

async fn audit_tool(
    pool: &MySqlPool,
    actor: u64,
    client: u64,
    grant: u64,
    request_id: &str,
    module: &str,
    tool: &str,
    object_id: Option<u64>,
    result: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query("INSERT INTO audit_events (actor_id,module_id,action,target_type,target_id,result,details_json,created_at) VALUES (?,?,'mcp.tool.call','mcp_tool',?,?,JSON_OBJECT('correlation_id',?,'client_id',?,'grant_id',?,'object_id',?),UTC_TIMESTAMP(6))")
        .bind(actor).bind(module).bind(tool).bind(result).bind(request_id).bind(client).bind(grant).bind(object_id).execute(pool).await?;
    Ok(())
}

async fn execute_read_tool<P>(
    state: &DraftMcpState<P>,
    access: &bastion_mcp::AuthenticatedAccess,
    request: &DraftRequest,
    correlation: &str,
) -> Response {
    let Some(id) = request.id() else {
        let charged = state
            .quota
            .charge(
                access.user_id,
                access.client_id,
                "notification",
                state.discovery_quota,
            )
            .await;
        let label = match &charged {
            Ok(()) => "success",
            Err(QuotaError::Limited { .. }) => "limited",
            Err(_) => "error",
        };
        if audit(
            &state.pool,
            access.user_id,
            access.client_id,
            access.grant_id,
            correlation,
            "mcp.notification",
            label,
        )
        .await
        .is_err()
        {
            return rpc_error(
                StatusCode::SERVICE_UNAVAILABLE,
                -32603,
                "Temporarily unavailable",
            );
        }
        return match charged {
            Ok(()) => (StatusCode::ACCEPTED, [(CACHE_CONTROL, "no-store")]).into_response(),
            Err(QuotaError::Limited {
                retry_after_seconds,
            }) => {
                let mut response =
                    rpc_error(StatusCode::TOO_MANY_REQUESTS, -32603, "Rate limit exceeded");
                if let Ok(value) = HeaderValue::from_str(&retry_after_seconds.to_string()) {
                    response.headers_mut().insert(RETRY_AFTER, value);
                }
                response
            }
            Err(_) => rpc_error(
                StatusCode::SERVICE_UNAVAILABLE,
                -32603,
                "Temporarily unavailable",
            ),
        };
    };
    let Some(name) = request
        .params()
        .get("name")
        .and_then(Value::as_str)
        .filter(|name| !name.is_empty() && name.len() <= 128)
    else {
        if audit(
            &state.pool,
            access.user_id,
            access.client_id,
            access.grant_id,
            correlation,
            "mcp.request.invalid",
            "denied",
        )
        .await
        .is_err()
        {
            return protocol_error(
                StatusCode::SERVICE_UNAVAILABLE,
                -32603,
                "Temporarily unavailable",
                Some(id.clone()),
                None,
            );
        }
        return protocol_error(
            StatusCode::BAD_REQUEST,
            -32602,
            "Invalid params",
            Some(id.clone()),
            None,
        );
    };
    let args = request
        .params()
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));
    if !args.is_object() {
        if audit(
            &state.pool,
            access.user_id,
            access.client_id,
            access.grant_id,
            correlation,
            "mcp.request.invalid",
            "denied",
        )
        .await
        .is_err()
        {
            return protocol_error(
                StatusCode::SERVICE_UNAVAILABLE,
                -32603,
                "Temporarily unavailable",
                Some(id.clone()),
                None,
            );
        }
        return protocol_error(
            StatusCode::BAD_REQUEST,
            -32602,
            "Invalid params",
            Some(id.clone()),
            None,
        );
    }
    let Some(executor) = &state.read_executor else {
        return protocol_error(
            StatusCode::SERVICE_UNAVAILABLE,
            -32603,
            "Temporarily unavailable",
            Some(id.clone()),
            None,
        );
    };
    let modules:Vec<String> = match sqlx::query_scalar("SELECT module_id FROM module_registry WHERE included_in_release=1 AND environment_allowed=1 AND enabled=1").fetch_all(&state.pool).await {
        Ok(value)=>value, Err(_)=>return protocol_error(StatusCode::SERVICE_UNAVAILABLE,-32603,"Temporarily unavailable",Some(id.clone()),None),
    };
    let enabled = modules.into_iter().collect();
    let Some(tool) = state.registry.find_visible(name, access, &enabled, false) else {
        if audit(
            &state.pool,
            access.user_id,
            access.client_id,
            access.grant_id,
            correlation,
            "mcp.tool.denied",
            "denied",
        )
        .await
        .is_err()
        {
            return protocol_error(
                StatusCode::SERVICE_UNAVAILABLE,
                -32603,
                "Temporarily unavailable",
                Some(id.clone()),
                None,
            );
        }
        return protocol_error(
            StatusCode::NOT_FOUND,
            -32601,
            "Tool unavailable",
            Some(id.clone()),
            None,
        );
    };
    let object_id = args.get("id").and_then(Value::as_u64);
    match state
        .quota
        .charge(
            access.user_id,
            access.client_id,
            &tool.name,
            state.discovery_quota,
        )
        .await
    {
        Ok(()) => {}
        Err(QuotaError::Limited {
            retry_after_seconds,
        }) => {
            if audit_tool(
                &state.pool,
                access.user_id,
                access.client_id,
                access.grant_id,
                correlation,
                &tool.module_id,
                &tool.name,
                object_id,
                "limited",
            )
            .await
            .is_err()
            {
                return protocol_error(
                    StatusCode::SERVICE_UNAVAILABLE,
                    -32603,
                    "Temporarily unavailable",
                    Some(id.clone()),
                    None,
                );
            }
            let mut response = protocol_error(
                StatusCode::TOO_MANY_REQUESTS,
                -32603,
                "Rate limit exceeded",
                Some(id.clone()),
                None,
            );
            if let Ok(value) = HeaderValue::from_str(&retry_after_seconds.to_string()) {
                response.headers_mut().insert(RETRY_AFTER, value);
            }
            return response;
        }
        Err(_) => {
            return protocol_error(
                StatusCode::SERVICE_UNAVAILABLE,
                -32603,
                "Temporarily unavailable",
                Some(id.clone()),
                None,
            );
        }
    }
    if audit_tool(
        &state.pool,
        access.user_id,
        access.client_id,
        access.grant_id,
        correlation,
        &tool.module_id,
        &tool.name,
        object_id,
        "started",
    )
    .await
    .is_err()
    {
        return protocol_error(
            StatusCode::SERVICE_UNAVAILABLE,
            -32603,
            "Temporarily unavailable",
            Some(id.clone()),
            None,
        );
    }
    let result = tokio::time::timeout(
        Duration::from_millis(tool.max_duration_ms),
        executor.call(access, &tool.name, args),
    )
    .await;
    let (result_label, output) = match result {
        Ok(Ok(value)) => {
            let text = match serde_json::to_string(&value) {
                Ok(text) => text,
                Err(_) => {
                    return protocol_error(
                        StatusCode::SERVICE_UNAVAILABLE,
                        -32603,
                        "Temporarily unavailable",
                        Some(id.clone()),
                        None,
                    );
                }
            };
            let output = json!({"jsonrpc":"2.0","id":id,"result":{"content":[{"type":"text","text":text}],"structuredContent":value,"isError":false}});
            if serde_json::to_vec(&output).map_or(true, |data| data.len() > tool.max_response_bytes)
            {
                ("error", None)
            } else {
                ("success", Some(output))
            }
        }
        Ok(Err(ReadToolError::Invalid)) => ("denied", None),
        Ok(Err(ReadToolError::Denied | ReadToolError::NotFound)) => ("denied", None),
        Ok(Err(ReadToolError::Limited)) => ("limited", None),
        _ => ("error", None),
    };
    if audit_tool(
        &state.pool,
        access.user_id,
        access.client_id,
        access.grant_id,
        correlation,
        &tool.module_id,
        &tool.name,
        object_id,
        result_label,
    )
    .await
    .is_err()
    {
        return protocol_error(
            StatusCode::SERVICE_UNAVAILABLE,
            -32603,
            "Temporarily unavailable",
            Some(id.clone()),
            None,
        );
    }
    match output {
        Some(output) => {
            (StatusCode::OK, [(CACHE_CONTROL, "no-store")], Json(output)).into_response()
        }
        None => protocol_error(
            StatusCode::BAD_REQUEST,
            -32000,
            "Tool request could not be completed",
            Some(id.clone()),
            None,
        ),
    }
}

async fn mcp<P>(State(state): State<DraftMcpState<P>>, headers: HeaderMap, body: Bytes) -> Response
where
    P: ExchangePolicy + Clone + Send + Sync + 'static,
{
    let Ok(_slot) = state.slots.clone().try_acquire_owned() else {
        return rpc_error(
            StatusCode::TOO_MANY_REQUESTS,
            -32603,
            "Temporarily unavailable",
        );
    };
    match tokio::time::timeout(state.timeout, handle(&state, &headers, &body)).await {
        Ok(response) => response,
        Err(_) => rpc_error(
            StatusCode::SERVICE_UNAVAILABLE,
            -32603,
            "Temporarily unavailable",
        ),
    }
}

async fn handle<P>(state: &DraftMcpState<P>, headers: &HeaderMap, body: &[u8]) -> Response
where
    P: ExchangePolicy + Clone + Send + Sync + 'static,
{
    match single(headers, "origin") {
        Err(()) => return rpc_error(StatusCode::FORBIDDEN, -32600, "Forbidden origin"),
        Ok(Some(origin)) if !state.origins.contains(origin) => {
            return rpc_error(StatusCode::FORBIDDEN, -32600, "Forbidden origin");
        }
        _ => {}
    }
    let bearer = single(headers, "authorization")
        .ok()
        .flatten()
        .and_then(|value| value.split_once(' '))
        .filter(|(scheme, token)| {
            scheme.eq_ignore_ascii_case("bearer") && !token.is_empty() && !token.contains(' ')
        })
        .and_then(|(_, token)| Credential::parse(token).ok());
    let Some(bearer) = bearer else {
        return (
            StatusCode::UNAUTHORIZED,
            [
                (WWW_AUTHENTICATE, state.challenge.clone()),
                (CACHE_CONTROL, HeaderValue::from_static("no-store")),
            ],
        )
            .into_response();
    };
    let access = match state
        .exchange
        .authenticate_access(&bearer, &state.resource)
        .await
    {
        Ok(access) => access,
        Err(bastion_mcp::TokenExchangeError::InvalidGrant) => {
            return (
                StatusCode::UNAUTHORIZED,
                [
                    (WWW_AUTHENTICATE, state.challenge.clone()),
                    (CACHE_CONTROL, HeaderValue::from_static("no-store")),
                ],
            )
                .into_response();
        }
        Err(_) => {
            return rpc_error(
                StatusCode::SERVICE_UNAVAILABLE,
                -32603,
                "Temporarily unavailable",
            );
        }
    };
    if let Some(guard) = &state.integrity {
        match guard.check_client_pk(access.client_id).await {
            Ok(()) => {}
            Err(MetadataIntegrityError::Invalid) => {
                return (
                    StatusCode::UNAUTHORIZED,
                    [
                        (WWW_AUTHENTICATE, state.challenge.clone()),
                        (CACHE_CONTROL, HeaderValue::from_static("no-store")),
                    ],
                )
                    .into_response();
            }
            Err(MetadataIntegrityError::Unavailable) => {
                return rpc_error(
                    StatusCode::SERVICE_UNAVAILABLE,
                    -32603,
                    "Temporarily unavailable",
                );
            }
        }
    }
    let request_id = match Credential::generate() {
        Ok(value) => value.expose(),
        Err(_) => {
            return rpc_error(
                StatusCode::SERVICE_UNAVAILABLE,
                -32603,
                "Temporarily unavailable",
            );
        }
    };
    let parsed = || -> Result<_, TransportError> {
        if headers.get_all(CONTENT_TYPE).iter().count() != 1
            || !headers
                .get(CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .is_some_and(|v| {
                    v.split(';')
                        .next()
                        .unwrap_or("")
                        .trim()
                        .eq_ignore_ascii_case("application/json")
                })
        {
            return Err(TransportError::InvalidRequest);
        }
        let origin = single(headers, "origin").map_err(|_| TransportError::HeaderMismatch)?;
        let version =
            single(headers, "mcp-protocol-version").map_err(|_| TransportError::HeaderMismatch)?;
        let method = single(headers, "mcp-method").map_err(|_| TransportError::HeaderMismatch)?;
        let name = single(headers, "mcp-name").map_err(|_| TransportError::HeaderMismatch)?;
        if !accepts(headers, "application/json") || !accepts(headers, "text/event-stream") {
            return Err(TransportError::Unacceptable);
        }
        if version == Some("2025-11-25") || version.is_none() {
            return validate_legacy_message(version, body).map(DraftRequest::Legacy);
        }
        validate_modern_request(
            &ModernHeaders {
                origin,
                protocol_version: version,
                method,
                name,
                accepts_json: accepts(headers, "application/json"),
                accepts_event_stream: accepts(headers, "text/event-stream"),
            },
            &state.origins,
            body,
        )
        .map(DraftRequest::Modern)
    };
    let request = match parsed() {
        Ok(request) => request,
        Err(error) => {
            if audit(
                &state.pool,
                access.user_id,
                access.client_id,
                access.grant_id,
                &request_id,
                "mcp.request.invalid",
                "denied",
            )
            .await
            .is_err()
            {
                return rpc_error(
                    StatusCode::SERVICE_UNAVAILABLE,
                    -32603,
                    "Temporarily unavailable",
                );
            }
            let id = extract_request_id(body);
            return match error {
                TransportError::ForbiddenOrigin => {
                    protocol_error(StatusCode::FORBIDDEN, -32600, "Forbidden origin", id, None)
                }
                TransportError::HeaderMismatch => {
                    protocol_error(StatusCode::BAD_REQUEST, -32020, "Header mismatch", id, None)
                }
                TransportError::UnsupportedVersion => {
                    let requested = single(headers, "mcp-protocol-version")
                        .ok()
                        .flatten()
                        .filter(|s| s.len() <= 64)
                        .unwrap_or("");
                    protocol_error(
                        StatusCode::BAD_REQUEST,
                        -32022,
                        "Unsupported protocol version",
                        id,
                        Some(
                            json!({"supported":["2026-07-28","2025-11-25"],"requested":requested}),
                        ),
                    )
                }
                TransportError::Unacceptable => protocol_error(
                    StatusCode::NOT_ACCEPTABLE,
                    -32600,
                    "Unsupported response type",
                    id,
                    None,
                ),
                TransportError::InvalidParams => {
                    protocol_error(StatusCode::BAD_REQUEST, -32602, "Invalid params", id, None)
                }
                TransportError::InvalidRequest => {
                    protocol_error(StatusCode::BAD_REQUEST, -32600, "Invalid request", id, None)
                }
            };
        }
    };
    if request.method() == "tools/call" {
        return execute_read_tool(state, &access, &request, &request_id).await;
    }
    let (bucket, action) = match request.method() {
        "server/discover" => ("server.discover", "mcp.discover"),
        "tools/list" => ("tools.list", "mcp.tools.list"),
        "initialize" => ("initialize", "mcp.initialize"),
        "notifications/initialized" => ("notification", "mcp.notification"),
        "ping" => ("ping", "mcp.ping"),
        _ => ("unsupported", "mcp.unsupported"),
    };
    match state
        .quota
        .charge(
            access.user_id,
            access.client_id,
            bucket,
            state.discovery_quota,
        )
        .await
    {
        Ok(()) => {}
        Err(QuotaError::Limited {
            retry_after_seconds,
        }) => {
            if audit(
                &state.pool,
                access.user_id,
                access.client_id,
                access.grant_id,
                &request_id,
                action,
                "limited",
            )
            .await
            .is_err()
            {
                return rpc_error(
                    StatusCode::SERVICE_UNAVAILABLE,
                    -32603,
                    "Temporarily unavailable",
                );
            }
            let mut response =
                rpc_error(StatusCode::TOO_MANY_REQUESTS, -32603, "Rate limit exceeded");
            if let Ok(value) = HeaderValue::from_str(&retry_after_seconds.to_string()) {
                response.headers_mut().insert(RETRY_AFTER, value);
            }
            return response;
        }
        Err(_) => {
            return rpc_error(
                StatusCode::SERVICE_UNAVAILABLE,
                -32603,
                "Temporarily unavailable",
            );
        }
    }
    let modules:Vec<String>=match sqlx::query_scalar("SELECT module_id FROM module_registry WHERE included_in_release=1 AND environment_allowed=1 AND enabled=1").fetch_all(&state.pool).await {Ok(modules)=>modules,Err(_)=>return rpc_error(StatusCode::SERVICE_UNAVAILABLE,-32603,"Temporarily unavailable")};
    let enabled_modules = modules.into_iter().collect();
    let empty_registry = if state.read_executor.is_none() {
        Some(ToolRegistry::new(Vec::new(), &BTreeSet::new()).expect("empty registry is valid"))
    } else {
        None
    };
    let registry = empty_registry.as_ref().unwrap_or(state.registry.as_ref());
    let result = match &request {
        DraftRequest::Modern(request) => {
            dispatch_discovery(request, &access, registry, &enabled_modules, false).map(Some)
        }
        DraftRequest::Legacy(request) => {
            dispatch_legacy(request, &access, registry, &enabled_modules).map(|response| {
                match response {
                    LegacyDispatch::Response(value) => Some(value),
                    LegacyDispatch::AcceptedNotification => None,
                }
            })
        }
    };
    let (status, body, result_label): (StatusCode, Option<Value>, &str) = match result {
        Ok(Some(value)) => (StatusCode::OK, Some(value), "success"),
        Ok(None) => (StatusCode::ACCEPTED, None, "success"),
        Err(DiscoveryError::InvalidParams) => (
            StatusCode::BAD_REQUEST,
            Some(
                json!({"jsonrpc":"2.0","id":request.id(),"error":{"code":-32602,"message":"Invalid params"}}),
            ),
            "denied",
        ),
        Err(DiscoveryError::MethodNotFound) => (
            StatusCode::NOT_FOUND,
            Some(
                json!({"jsonrpc":"2.0","id":request.id(),"error":{"code":-32601,"message":"Method not found"}}),
            ),
            "denied",
        ),
    };
    if audit(
        &state.pool,
        access.user_id,
        access.client_id,
        access.grant_id,
        &request_id,
        action,
        result_label,
    )
    .await
    .is_err()
    {
        return rpc_error(
            StatusCode::SERVICE_UNAVAILABLE,
            -32603,
            "Temporarily unavailable",
        );
    }
    match body {
        Some(body) => (status, [(CACHE_CONTROL, "no-store")], Json(body)).into_response(),
        None => (status, [(CACHE_CONTROL, "no-store")]).into_response(),
    }
}
