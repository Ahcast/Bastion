use axum::{
    Router,
    body::Bytes,
    extract::{DefaultBodyLimit, State},
    http::{HeaderMap, StatusCode},
    response::Response,
    routing::post,
};
use bastion_core::CoreService;
use bastion_mcp::{ClientMetadata, ClientRequestError, ClientRequestStore, ReviewDecision};
use bastion_mcp_core::{CoreConsentPolicy, approved_read_requirements};
use serde::Deserialize;
use serde_json::json;
use sqlx::MySqlPool;
use std::{collections::BTreeSet, future::Future, sync::Arc};
use tokio::sync::Semaphore;
use url::Url;

use crate::{
    DraftMetadataFetcher, MetadataFetchError, OauthMetadataError,
    consent_http::{browser_error, browser_session, diagnostic, error, response},
};

#[path = "client_onboarding.rs"]
mod onboarding;

/// Only a server-owned source may be installed; the network implementation
/// validates DNS and TLS and is still subject to egress controls.
pub trait ClientMetadataSource: Clone + Send + Sync + 'static {
    fn fetch(
        &self,
        client_id: &str,
    ) -> impl Future<Output = Result<ClientMetadata, MetadataFetchError>> + Send;
}
impl ClientMetadataSource for DraftMetadataFetcher {
    async fn fetch(&self, client_id: &str) -> Result<ClientMetadata, MetadataFetchError> {
        DraftMetadataFetcher::fetch(self, client_id).await
    }
}

#[derive(Clone)]
pub struct DraftClientRequestState<F> {
    pool: MySqlPool,
    core: CoreService,
    origin: String,
    fetcher: F,
    slots: Arc<Semaphore>,
    resource: Option<String>,
}
impl<F: ClientMetadataSource> DraftClientRequestState<F> {
    /// Unmounted draft. Core and pool must refer to the same database.
    pub fn new(
        pool: MySqlPool,
        core: CoreService,
        origin: String,
        fetcher: F,
        max_concurrent: usize,
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
            || !(1..=16).contains(&max_concurrent)
        {
            return Err(OauthMetadataError);
        }
        Ok(Self {
            pool,
            core,
            origin,
            fetcher,
            slots: Arc::new(Semaphore::new(max_concurrent)),
            resource: None,
        })
    }

    pub fn with_resource(mut self, resource: String) -> Self {
        self.resource = Some(resource);
        self
    }
}

/// Draft administrative endpoints. Edge rate limits, egress rules, CORS,
/// notification delivery and the browser interface are still required.
pub fn draft_client_request_router<F: ClientMetadataSource>(
    state: DraftClientRequestState<F>,
) -> Router {
    Router::new()
        .route(
            "/oauth/clients/inspect-authorization",
            post(onboarding::inspect::<F>),
        )
        .route(
            "/oauth/connections/list",
            post(onboarding::connections::<F>),
        )
        .route(
            "/oauth/connections/disconnect",
            post(onboarding::disconnect::<F>),
        )
        .route("/oauth/clients/request", post(submit::<F>))
        .route("/oauth/clients/mine", post(mine::<F>))
        .route("/oauth/clients/pending", post(pending::<F>))
        .route("/oauth/clients/review", post(review::<F>))
        .route("/oauth/clients/admin/list", post(managed_list::<F>))
        .route("/oauth/clients/admin/scopes", post(managed_scopes::<F>))
        .route("/oauth/clients/admin/disable", post(managed_disable::<F>))
        .layer(DefaultBodyLimit::max(16 * 1024))
        .with_state(state)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Submit {
    client_id: String,
    scopes: Vec<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct List {
    before: Option<u64>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Review {
    request_id: u64,
    decision: String,
    client_id: Option<String>,
    scopes: Option<Vec<String>>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UpdateManaged {
    id: u64,
    expected_scopes: Vec<String>,
    scopes: Vec<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DisableManaged {
    id: u64,
    confirmed_client_id: String,
}

fn scopes(values: Vec<String>) -> Result<BTreeSet<String>, StatusCode> {
    let count = values.len();
    let scopes: BTreeSet<_> = values.into_iter().collect();
    let registered: BTreeSet<_> = approved_read_requirements().into_keys().collect();
    if count == 0 || count > 64 || count != scopes.len() || !scopes.is_subset(&registered) {
        return Err(StatusCode::BAD_REQUEST);
    }
    Ok(scopes)
}
fn status(reason: ClientRequestError) -> StatusCode {
    match reason {
        ClientRequestError::Forbidden => StatusCode::FORBIDDEN,
        ClientRequestError::Invalid => StatusCode::BAD_REQUEST,
        ClientRequestError::NotFound => StatusCode::NOT_FOUND,
        ClientRequestError::Conflict | ClientRequestError::AlreadyApproved => StatusCode::CONFLICT,
        ClientRequestError::Database(_) => StatusCode::SERVICE_UNAVAILABLE,
    }
}

async fn submit<F: ClientMetadataSource>(
    State(state): State<DraftClientRequestState<F>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let session = match browser_session(&state.core, &state.origin, &headers).await {
        Ok(v) => v,
        Err(s) => return browser_error(s),
    };
    let input: Submit = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(_) => return error(StatusCode::BAD_REQUEST),
    };
    let scopes = match scopes(input.scopes) {
        Ok(v) => v,
        Err(s) => return error(s),
    };
    let store = ClientRequestStore::new(state.pool.clone());
    if let Err(e) = store
        .authorize_session(&CoreConsentPolicy, &session, false)
        .await
    {
        return error(status(e));
    }
    let Ok(_slot) = state.slots.clone().try_acquire_owned() else {
        return error(StatusCode::TOO_MANY_REQUESTS);
    };
    let metadata = match state.fetcher.fetch(&input.client_id).await {
        Ok(v) => v,
        Err(_) => return error(StatusCode::BAD_REQUEST),
    };
    let registered = approved_read_requirements().into_keys().collect();
    match store
        .submit_for_session(
            &CoreConsentPolicy,
            &session,
            &metadata,
            &scopes,
            &registered,
        )
        .await
    {
        Ok(id) => response(StatusCode::OK, json!({"request_id":id,"status":"pending"})),
        Err(ClientRequestError::AlreadyApproved) => response(
            StatusCode::CONFLICT,
            json!({"error":"already_approved","detail":"El cliente ya tiene estos permisos aprobados. Vuelve a verificar la conexión para autorizar tu cuenta."}),
        ),
        Err(ClientRequestError::Conflict) => response(
            StatusCode::CONFLICT,
            json!({"error":"review_conflict","detail":"Ya existe una solicitud pendiente diferente o cambió la identidad del cliente. Pide al administrador revisar la solicitud en Ajustes → Clientes MCP y vuelve a iniciar la conexión."}),
        ),
        Err(e) => error(status(e)),
    }
}

async fn list<F: ClientMetadataSource>(
    state: DraftClientRequestState<F>,
    headers: HeaderMap,
    body: Bytes,
    admin: bool,
) -> Response {
    let session = match browser_session(&state.core, &state.origin, &headers).await {
        Ok(v) => v,
        Err(s) => return browser_error(s),
    };
    let input: List = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(_) => return error(StatusCode::BAD_REQUEST),
    };
    match ClientRequestStore::new(state.pool)
        .list_for_session(&CoreConsentPolicy, &session, input.before, admin)
        .await
    {
        Ok(page) => match serde_json::to_value(page) {
            Ok(value) => response(StatusCode::OK, value),
            Err(_) => error(StatusCode::SERVICE_UNAVAILABLE),
        },
        Err(e) => error(status(e)),
    }
}
async fn mine<F: ClientMetadataSource>(
    State(state): State<DraftClientRequestState<F>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    list(state, headers, body, false).await
}
async fn pending<F: ClientMetadataSource>(
    State(state): State<DraftClientRequestState<F>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    list(state, headers, body, true).await
}

async fn managed_list<F: ClientMetadataSource>(
    State(state): State<DraftClientRequestState<F>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let session = match browser_session(&state.core, &state.origin, &headers).await {
        Ok(v) => v,
        Err(s) => return browser_error(s),
    };
    let input: List = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(_) => return error(StatusCode::BAD_REQUEST),
    };
    match ClientRequestStore::new(state.pool)
        .list_managed_for_session(&CoreConsentPolicy, &session, input.before)
        .await
    {
        Ok(page) => match serde_json::to_value(page) {
            Ok(value) => response(StatusCode::OK, value),
            Err(_) => error(StatusCode::SERVICE_UNAVAILABLE),
        },
        Err(e) => error(status(e)),
    }
}

async fn managed_scopes<F: ClientMetadataSource>(
    State(state): State<DraftClientRequestState<F>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let session = match browser_session(&state.core, &state.origin, &headers).await {
        Ok(v) => v,
        Err(s) => return browser_error(s),
    };
    let input: UpdateManaged = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(_) => return error(StatusCode::BAD_REQUEST),
    };
    let expected_count = input.expected_scopes.len();
    let expected: BTreeSet<String> = input.expected_scopes.into_iter().collect();
    if expected_count != expected.len() || expected_count > 64 {
        return error(StatusCode::BAD_REQUEST);
    }
    let desired = match scopes(input.scopes) {
        Ok(v) => v,
        Err(s) => return error(s),
    };
    if desired
        .iter()
        .filter_map(|scope| scope.strip_suffix(":write"))
        .any(|module| !desired.contains(&format!("{module}:read")))
    {
        return error(StatusCode::BAD_REQUEST);
    }
    let registered = approved_read_requirements().into_keys().collect();
    match ClientRequestStore::new(state.pool)
        .update_managed_for_session(
            &CoreConsentPolicy,
            &session,
            input.id,
            &expected,
            &desired,
            &registered,
        )
        .await
    {
        Ok(()) => response(StatusCode::OK, json!({"status":"updated"})),
        Err(e) => error(status(e)),
    }
}

async fn managed_disable<F: ClientMetadataSource>(
    State(state): State<DraftClientRequestState<F>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let session = match browser_session(&state.core, &state.origin, &headers).await {
        Ok(v) => v,
        Err(s) => return browser_error(s),
    };
    let input: DisableManaged = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(_) => return error(StatusCode::BAD_REQUEST),
    };
    match ClientRequestStore::new(state.pool)
        .disable_managed_for_session(
            &CoreConsentPolicy,
            &session,
            input.id,
            &input.confirmed_client_id,
        )
        .await
    {
        Ok(()) => response(StatusCode::OK, json!({"status":"disabled"})),
        Err(e) => error(status(e)),
    }
}

async fn review<F: ClientMetadataSource>(
    State(state): State<DraftClientRequestState<F>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let session = match browser_session(&state.core, &state.origin, &headers).await {
        Ok(v) => v,
        Err(s) => return browser_error(s),
    };
    let input: Review = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(_) => return error(StatusCode::BAD_REQUEST),
    };
    if input.request_id == 0 {
        return error(StatusCode::BAD_REQUEST);
    }
    let store = ClientRequestStore::new(state.pool.clone());
    let registered = approved_read_requirements().into_keys().collect();
    if let Err(e) = store
        .authorize_session(&CoreConsentPolicy, &session, true)
        .await
    {
        return error(status(e));
    }
    let result = match (input.decision.as_str(), input.client_id, input.scopes) {
        ("reject", None, None) => {
            store
                .review_for_session(
                    &CoreConsentPolicy,
                    &session,
                    input.request_id,
                    ReviewDecision::Reject,
                    &registered,
                )
                .await
        }
        ("approve", Some(client_id), Some(requested)) => {
            let scopes = match scopes(requested) {
                Ok(v) => v,
                Err(s) => return error(s),
            };
            let Ok(_slot) = state.slots.clone().try_acquire_owned() else {
                return error(StatusCode::TOO_MANY_REQUESTS);
            };
            let metadata = match state.fetcher.fetch(&client_id).await {
                Ok(v) => v,
                Err(_) => return error(StatusCode::BAD_REQUEST),
            };
            store
                .review_for_session(
                    &CoreConsentPolicy,
                    &session,
                    input.request_id,
                    ReviewDecision::Approve {
                        metadata: &metadata,
                        scopes: &scopes,
                    },
                    &registered,
                )
                .await
        }
        _ => return error(StatusCode::BAD_REQUEST),
    };
    match result {
        Ok(()) => response(StatusCode::OK, json!({"status":"reviewed"})),
        Err(e) => error(status(e)),
    }
}
