use std::{collections::BTreeSet, sync::Arc, time::Duration};

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
    Credential, DiscoveryError, ExchangePolicy, ModernHeaders, MysqlQuota, MysqlTokenExchange,
    QuotaError, ToolQuota, ToolRegistry, TransportError, dispatch_discovery, extract_request_id,
    validate_modern_request,
};
use serde_json::{Value, json};
use sqlx::MySqlPool;
use tokio::sync::Semaphore;
use url::Url;

const MAX_BODY: usize = 64 * 1024;

#[derive(Clone)]
pub struct DraftMcpState<P> {
    pool: MySqlPool,
    exchange: MysqlTokenExchange<P>,
    resource: String,
    challenge: HeaderValue,
    origins: BTreeSet<String>,
    registry: Arc<ToolRegistry>,
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
            quota,
            discovery_quota,
            slots: Arc::new(Semaphore::new(max_concurrent)),
            timeout,
        })
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
                        Some(json!({"supported":["2026-07-28"],"requested":requested})),
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
    let (bucket, action) = match request.method.as_str() {
        "server/discover" => ("server.discover", "mcp.discover"),
        "tools/list" => ("tools.list", "mcp.tools.list"),
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
    let result = dispatch_discovery(
        &request,
        &access,
        &state.registry,
        &modules.into_iter().collect(),
        false,
    );
    let (status, body, result_label): (StatusCode, Value, &str) = match result {
        Ok(value) => (StatusCode::OK, value, "success"),
        Err(DiscoveryError::InvalidParams) => (
            StatusCode::BAD_REQUEST,
            json!({"jsonrpc":"2.0","id":request.id,"error":{"code":-32602,"message":"Invalid params"}}),
            "denied",
        ),
        Err(DiscoveryError::MethodNotFound) => (
            StatusCode::NOT_FOUND,
            json!({"jsonrpc":"2.0","id":request.id,"error":{"code":-32601,"message":"Method not found"}}),
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
    (status, [(CACHE_CONTROL, "no-store")], Json(body)).into_response()
}
