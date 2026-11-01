use std::sync::Arc;

use axum::{
    Json, Router,
    body::Bytes,
    extract::{DefaultBodyLimit, State},
    http::{
        HeaderMap, StatusCode,
        header::{
            CACHE_CONTROL, CONTENT_TYPE, COOKIE, ORIGIN, PRAGMA, REFERRER_POLICY,
            X_CONTENT_TYPE_OPTIONS,
        },
    },
    response::{IntoResponse, Response},
    routing::post,
};
use bastion_core::CoreService;
use bastion_mcp::{
    ConsentError, Credential, ExchangePolicy, MysqlConsent, parse_authorization_parameters,
};
use bastion_mcp_core::CoreConsentPolicy;
use serde::Deserialize;
use serde_json::json;
use sqlx::MySqlPool;
use subtle::ConstantTimeEq;
use url::Url;

use crate::{DraftOauthMetadata, OauthMetadataError};

#[derive(Clone)]
pub struct DraftConsentState<P> {
    consent: MysqlConsent<CoreConsentPolicy, P>,
    core: CoreService,
    metadata: Arc<DraftOauthMetadata>,
    origin: String,
}

impl<P: ExchangePolicy> DraftConsentState<P> {
    /// Inputs are trusted release configuration. Core and pool must refer to
    /// the same database. The router is unmounted and requires edge limits,
    /// CORS, a consent UI and end-to-end testing before release.
    pub fn new(
        pool: MySqlPool,
        core: CoreService,
        policy: P,
        metadata: Arc<DraftOauthMetadata>,
        origin: String,
    ) -> Result<Self, OauthMetadataError> {
        let url = Url::parse(&origin).map_err(|_| OauthMetadataError)?;
        if url.scheme() != "https"
            || url.host().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.path() != "/"
            || url.query().is_some()
            || url.fragment().is_some()
            || url.origin().ascii_serialization() != origin
        {
            return Err(OauthMetadataError);
        }
        Ok(Self {
            consent: MysqlConsent::new(pool, CoreConsentPolicy, policy),
            core,
            metadata,
            origin,
        })
    }
}

/// Browser-facing draft only; it is not connected to the active API.
pub fn draft_consent_router<P>(state: DraftConsentState<P>) -> Router
where
    P: ExchangePolicy + Clone + Send + Sync + 'static,
{
    Router::new()
        .route("/oauth/consent/begin", post(begin::<P>))
        .route("/oauth/consent/decision", post(decision::<P>))
        .layer(DefaultBodyLimit::max(16 * 1024))
        .with_state(state)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BeginRequest {
    authorization_query: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DecisionRequest {
    handle: String,
    decision: Decision,
}
#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum Decision {
    Approve,
    Deny,
}

pub(crate) fn response(status: StatusCode, body: serde_json::Value) -> Response {
    (
        status,
        [
            (CACHE_CONTROL, "no-store"),
            (PRAGMA, "no-cache"),
            (REFERRER_POLICY, "no-referrer"),
            (X_CONTENT_TYPE_OPTIONS, "nosniff"),
        ],
        Json(body),
    )
        .into_response()
}
pub(crate) fn error(status: StatusCode) -> Response {
    response(status, json!({"error":"request_not_completed"}))
}

pub(crate) async fn browser_session(
    core: &CoreService,
    origin: &str,
    headers: &HeaderMap,
) -> Result<String, StatusCode> {
    if headers.get_all(ORIGIN).iter().count() != 1
        || headers.get(ORIGIN).and_then(|h| h.to_str().ok()) != Some(origin)
    {
        return Err(StatusCode::FORBIDDEN);
    }
    if headers.get_all(CONTENT_TYPE).iter().count() != 1
        || !headers
            .get(CONTENT_TYPE)
            .and_then(|h| h.to_str().ok())
            .is_some_and(|h| {
                h.split(';')
                    .next()
                    .unwrap_or("")
                    .trim()
                    .eq_ignore_ascii_case("application/json")
            })
    {
        return Err(StatusCode::BAD_REQUEST);
    }
    if headers.get_all(COOKIE).iter().count() != 1
        || headers.get_all("x-csrf-token").iter().count() != 1
    {
        return Err(StatusCode::FORBIDDEN);
    }
    let cookies = headers
        .get(COOKIE)
        .and_then(|h| h.to_str().ok())
        .ok_or(StatusCode::FORBIDDEN)?;
    let header = headers
        .get("x-csrf-token")
        .and_then(|h| h.to_str().ok())
        .ok_or(StatusCode::FORBIDDEN)?;
    let mut session = None;
    let mut csrf = None;
    for pair in cookies.split(';') {
        let Some((name, value)) = pair.trim().split_once('=') else {
            continue;
        };
        let slot = match name {
            "__Host-bastion_session" => &mut session,
            "__Host-bastion_csrf" => &mut csrf,
            _ => continue,
        };
        if slot.replace(value).is_some() {
            return Err(StatusCode::FORBIDDEN);
        }
    }
    let session = session.ok_or(StatusCode::UNAUTHORIZED)?;
    let csrf = csrf.ok_or(StatusCode::FORBIDDEN)?;
    if Credential::parse(session).is_err()
        || Credential::parse(csrf).is_err()
        || !bool::from(csrf.as_bytes().ct_eq(header.as_bytes()))
    {
        return Err(StatusCode::FORBIDDEN);
    }
    match core.csrf_matches(session, csrf).await {
        Ok(true) => Ok(session.to_owned()),
        Ok(false) => Err(StatusCode::FORBIDDEN),
        Err(_) => Err(StatusCode::SERVICE_UNAVAILABLE),
    }
}

fn consent_status(error: ConsentError) -> StatusCode {
    match error {
        ConsentError::InvalidRequest => StatusCode::BAD_REQUEST,
        ConsentError::Unauthorized => StatusCode::UNAUTHORIZED,
        _ => StatusCode::SERVICE_UNAVAILABLE,
    }
}

async fn begin<P>(
    State(state): State<DraftConsentState<P>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response
where
    P: ExchangePolicy + Clone + Send + Sync + 'static,
{
    let session = match browser_session(&state.core, &state.origin, &headers).await {
        Ok(value) => value,
        Err(status) => return error(status),
    };
    let input: BeginRequest = match serde_json::from_slice(&body) {
        Ok(value) => value,
        Err(_) => return error(StatusCode::BAD_REQUEST),
    };
    let request =
        match parse_authorization_parameters(&input.authorization_query, state.metadata.resource())
        {
            Ok(value) => value,
            Err(_) => return error(StatusCode::BAD_REQUEST),
        };
    match state
        .consent
        .begin(&session, &request, state.metadata.resource())
        .await
    {
        Ok(pending) => response(
            StatusCode::OK,
            json!({"handle":pending.handle.expose(),"client_name":pending.client_name,"scopes":pending.scopes,"expires_at":pending.expires_at.and_utc().to_rfc3339()}),
        ),
        Err(reason) => error(consent_status(reason)),
    }
}

async fn decision<P>(
    State(state): State<DraftConsentState<P>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response
where
    P: ExchangePolicy + Clone + Send + Sync + 'static,
{
    let session = match browser_session(&state.core, &state.origin, &headers).await {
        Ok(value) => value,
        Err(status) => return error(status),
    };
    let input: DecisionRequest = match serde_json::from_slice(&body) {
        Ok(value) => value,
        Err(_) => return error(StatusCode::BAD_REQUEST),
    };
    let handle = match Credential::parse(&input.handle) {
        Ok(value) => value,
        Err(_) => return error(StatusCode::BAD_REQUEST),
    };
    let redirect = match input.decision {
        Decision::Approve => match state
            .consent
            .approve(&session, &handle, state.metadata.resource())
            .await
        {
            Ok(value) => state.metadata.success_redirect(value),
            Err(reason) => return error(consent_status(reason)),
        },
        Decision::Deny => match state
            .consent
            .deny(&session, &handle, state.metadata.resource())
            .await
        {
            Ok(value) => state.metadata.denied_redirect(value),
            Err(reason) => return error(consent_status(reason)),
        },
    };
    match redirect {
        Ok(value) => response(StatusCode::OK, json!({"redirect_to":value})),
        Err(_) => error(StatusCode::SERVICE_UNAVAILABLE),
    }
}
