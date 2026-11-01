#![forbid(unsafe_code)]

mod client_requests_http;
mod composition;
mod consent_http;
mod metadata_fetch;
mod modern;
mod oauth_metadata;
pub use client_requests_http::{
    ClientMetadataSource, DraftClientRequestState, draft_client_request_router,
};
pub use composition::{DraftCompositionConfig, DraftCompositionError, compose_draft_router};
pub use consent_http::{DraftConsentState, draft_consent_router};
pub use metadata_fetch::{DraftMetadataFetcher, MetadataFetchError};
pub use modern::{DraftMcpState, McpConfigurationError, draft_mcp_router};
pub use oauth_metadata::{DraftOauthMetadata, OauthMetadataError, draft_oauth_metadata_router};

use std::collections::BTreeSet;

use axum::{
    Json, Router,
    body::Bytes,
    extract::{DefaultBodyLimit, State},
    http::{
        HeaderMap, StatusCode,
        header::{AUTHORIZATION, CACHE_CONTROL, CONTENT_TYPE, ORIGIN, PRAGMA},
    },
    response::{IntoResponse, Response},
    routing::post,
};
use bastion_mcp::{
    ExchangePolicy, MysqlTokenExchange, ProtocolError, TokenExchangeError, TokenParameters,
    parse_token_parameters,
};
use serde::Serialize;

const MAX_BODY: usize = 8192;

#[derive(Clone)]
pub struct TokenEndpoint<P> {
    exchange: MysqlTokenExchange<P>,
    resource: String,
    allowed_origins: BTreeSet<String>,
}

impl<P> TokenEndpoint<P> {
    /// Resource and origins must be server-owned, validated HTTPS values.
    pub fn new(
        exchange: MysqlTokenExchange<P>,
        resource: String,
        allowed_origins: BTreeSet<String>,
    ) -> Self {
        Self {
            exchange,
            resource,
            allowed_origins,
        }
    }
}

/// Draft only: do not mount this router until per-IP/client limits, secure
/// metadata, consent UI, client compatibility and operational controls exist.
pub fn draft_token_router<P>(state: TokenEndpoint<P>) -> Router
where
    P: ExchangePolicy + Clone + Send + Sync + 'static,
{
    Router::new()
        .route("/oauth/token", post(token::<P>))
        .layer(DefaultBodyLimit::max(MAX_BODY))
        .with_state(state)
}

#[derive(Serialize)]
struct TokenResponse {
    access_token: String,
    token_type: &'static str,
    expires_in: u64,
    refresh_token: String,
    scope: String,
}
#[derive(Serialize)]
struct OAuthError {
    error: &'static str,
}

fn error(status: StatusCode, code: &'static str) -> Response {
    (
        status,
        [(CACHE_CONTROL, "no-store"), (PRAGMA, "no-cache")],
        Json(OAuthError { error: code }),
    )
        .into_response()
}

async fn token<P>(
    State(state): State<TokenEndpoint<P>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response
where
    P: ExchangePolicy + Clone + Send + Sync + 'static,
{
    // Reject ambiguous headers and browser origins not explicitly allowed.
    if headers.get_all(CONTENT_TYPE).iter().count() != 1
        || headers.get_all(ORIGIN).iter().count() > 1
        || headers.contains_key(AUTHORIZATION)
    {
        return error(StatusCode::BAD_REQUEST, "invalid_request");
    }
    if let Some(origin) = headers.get(ORIGIN) {
        let Ok(origin) = origin.to_str() else {
            return error(StatusCode::FORBIDDEN, "invalid_request");
        };
        if !state.allowed_origins.contains(origin) {
            return error(StatusCode::FORBIDDEN, "invalid_request");
        }
    }
    let content_type = headers
        .get(CONTENT_TYPE)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("");
    if !content_type
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .eq_ignore_ascii_case("application/x-www-form-urlencoded")
    {
        return error(StatusCode::BAD_REQUEST, "invalid_request");
    }
    let Ok(body) = std::str::from_utf8(&body) else {
        return error(StatusCode::BAD_REQUEST, "invalid_request");
    };
    let request = match parse_token_parameters(body, &state.resource) {
        Ok(request) => request,
        Err(reason) => {
            return error(
                StatusCode::BAD_REQUEST,
                match reason {
                    ProtocolError::UnsupportedGrantType => "unsupported_grant_type",
                    ProtocolError::InvalidTarget => "invalid_target",
                    ProtocolError::InvalidScope => "invalid_scope",
                    ProtocolError::InvalidGrant | ProtocolError::InvalidPkce => "invalid_grant",
                    _ => "invalid_request",
                },
            );
        }
    };
    let result = match &request {
        TokenParameters::AuthorizationCode { .. } => state.exchange.redeem_code(&request).await,
        TokenParameters::RefreshToken { .. } => state.exchange.rotate_refresh(&request).await,
    };
    match result {
        Ok(tokens) => {
            let response = TokenResponse {
                access_token: tokens.access_token.expose(),
                token_type: "Bearer",
                expires_in: tokens.expires_in,
                refresh_token: tokens.refresh_token.expose(),
                scope: tokens.scopes.into_iter().collect::<Vec<_>>().join(" "),
            };
            (
                StatusCode::OK,
                [(CACHE_CONTROL, "no-store"), (PRAGMA, "no-cache")],
                Json(response),
            )
                .into_response()
        }
        Err(TokenExchangeError::InvalidGrant) => error(StatusCode::BAD_REQUEST, "invalid_grant"),
        Err(TokenExchangeError::Unavailable | TokenExchangeError::Database(_)) => {
            error(StatusCode::SERVICE_UNAVAILABLE, "temporarily_unavailable")
        }
    }
}
