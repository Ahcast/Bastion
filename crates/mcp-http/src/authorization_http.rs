use std::{collections::BTreeSet, marker::PhantomData};

use axum::{
    Router,
    extract::{RawQuery, State},
    http::{
        HeaderMap, HeaderValue, StatusCode,
        header::{
            CACHE_CONTROL, LOCATION, ORIGIN, PRAGMA, REFERRER_POLICY, X_CONTENT_TYPE_OPTIONS,
        },
    },
    response::{IntoResponse, Response},
    routing::get,
};
use bastion_mcp::{AuthorizationParameters, ClientMetadata, parse_authorization_parameters};
use bastion_mcp_core::approved_read_requirements;
use sha2::{Digest, Sha256};
use sqlx::MySqlPool;
use url::Url;

use crate::{
    ClientMetadataSource, OauthMetadataError,
    consent_http::{diagnostic, error},
};

#[derive(Clone)]
pub struct DraftAuthorizationState<F> {
    resource: String,
    frontend_origin: String,
    client_origins: BTreeSet<String>,
    source: PhantomData<F>,
}
impl<F: ClientMetadataSource> DraftAuthorizationState<F> {
    /// Trusted configuration only. Proxy logging, egress controls and per-IP
    /// limits must be verified before this isolated route can be mounted.
    pub fn new(
        _pool: MySqlPool,
        resource: String,
        frontend_origin: String,
        client_origins: BTreeSet<String>,
        _fetcher: F,
        max_concurrent: usize,
    ) -> Result<Self, OauthMetadataError> {
        let resource_url = Url::parse(&resource).map_err(|_| OauthMetadataError)?;
        let frontend = Url::parse(&frontend_origin).map_err(|_| OauthMetadataError)?;
        if resource_url.scheme() != "https"
            || resource_url.host().is_none()
            || !resource_url.username().is_empty()
            || resource_url.password().is_some()
            || resource_url.query().is_some()
            || resource_url.fragment().is_some()
            || resource_url.as_str() != resource
            || frontend.scheme() != "https"
            || frontend.host().is_none()
            || !frontend.username().is_empty()
            || frontend.password().is_some()
            || frontend.path() != "/"
            || frontend.query().is_some()
            || frontend.fragment().is_some()
            || frontend.origin().ascii_serialization() != frontend_origin
            || !(1..=16).contains(&max_concurrent)
        {
            return Err(OauthMetadataError);
        }
        for origin in &client_origins {
            let url = Url::parse(origin).map_err(|_| OauthMetadataError)?;
            if url.scheme() != "https"
                || url.origin().ascii_serialization() != *origin
                || url.path() != "/"
                || url.query().is_some()
                || url.fragment().is_some()
                || !url.username().is_empty()
                || url.password().is_some()
            {
                return Err(OauthMetadataError);
            }
        }
        Ok(Self {
            resource,
            frontend_origin,
            client_origins,
            source: PhantomData,
        })
    }
}

/// Unmounted entry point. It never redirects to a client-supplied URI.
pub fn draft_authorization_router<F: ClientMetadataSource>(
    state: DraftAuthorizationState<F>,
) -> Router {
    Router::new()
        .route("/oauth/authorize", get(authorize::<F>))
        .with_state(state)
}

pub(crate) async fn approved(
    pool: &MySqlPool,
    request: &AuthorizationParameters,
    metadata: Option<&ClientMetadata>,
) -> Result<bool, sqlx::Error> {
    let registered: BTreeSet<_> = approved_read_requirements().into_keys().collect();
    if !request.scopes.is_subset(&registered) {
        return Ok(false);
    }
    let identity_hash: [u8; 32] = Sha256::digest(request.client_id.as_bytes()).into();
    let row=sqlx::query_as::<_,(u64,String,Vec<u8>,Option<Vec<u8>>,bool,String)>("SELECT id,client_id,metadata_hash,approved_metadata_hash,enabled,token_auth_method FROM oauth_clients WHERE client_id_hash=?")
        .bind(identity_hash.as_slice()).fetch_optional(pool).await?;
    let Some((id, identity, current, approved, enabled, method)) = row else {
        return Ok(false);
    };
    if identity != request.client_id
        || !enabled
        || method != "none"
        || approved.as_deref() != Some(current.as_slice())
    {
        return Ok(false);
    }
    if metadata.is_some_and(|value| {
        value.client_id() != identity
            || value.document_hash().as_slice() != current.as_slice()
            || !value.redirect_uris().contains(&request.redirect_uri)
    }) {
        return Ok(false);
    }
    let redirect_hash: [u8; 32] = Sha256::digest(request.redirect_uri.as_bytes()).into();
    let redirect = sqlx::query_scalar::<_, String>(
        "SELECT uri FROM oauth_client_redirects WHERE client_id=? AND uri_hash=?",
    )
    .bind(id)
    .bind(redirect_hash.as_slice())
    .fetch_optional(pool)
    .await?;
    if redirect.as_deref() != Some(request.redirect_uri.as_str()) {
        return Ok(false);
    }
    let ceiling: BTreeSet<String> = sqlx::query_scalar::<_, String>(
        "SELECT scope_name FROM oauth_client_scopes WHERE client_id=?",
    )
    .bind(id)
    .fetch_all(pool)
    .await?
    .into_iter()
    .collect();
    Ok(request.scopes.is_subset(&ceiling))
}

async fn authorize<F: ClientMetadataSource>(
    State(state): State<DraftAuthorizationState<F>>,
    headers: HeaderMap,
    RawQuery(query): RawQuery,
) -> Response {
    if headers.get_all(ORIGIN).iter().count() > 1 {
        return error(StatusCode::FORBIDDEN);
    }
    if let Some(origin) = headers.get(ORIGIN) {
        if origin
            .to_str()
            .ok()
            .is_none_or(|value| !state.client_origins.contains(value))
        {
            return error(StatusCode::FORBIDDEN);
        }
    }
    let Some(query) = query else {
        return error(StatusCode::BAD_REQUEST);
    };
    let request = match parse_authorization_parameters(&query, &state.resource) {
        Ok(value) => value,
        Err(_) => {
            return diagnostic(
                StatusCode::BAD_REQUEST,
                "invalid_authorization_request",
                "El cliente debe iniciar una solicitud nueva con PKCE S256, el recurso de Bastion y permisos explícitos. Revisa su configuración MCP/OAuth.",
            );
        }
    };
    if bastion_mcp::validate_client_identifier(&request.client_id).is_err()
        || bastion_mcp::validate_redirect_uri(&request.redirect_uri).is_err()
        || !request
            .scopes
            .is_subset(&approved_read_requirements().into_keys().collect())
    {
        return error(StatusCode::BAD_REQUEST);
    }
    // This is a local handoff, not an authorization. Client approval and fresh
    // metadata are checked after login, before creating consent or tokens.
    // Unknown clients must reach the enrollment screen. Anonymous requests
    // never trigger metadata network requests or create registration records.
    let location = match HeaderValue::from_str(&format!(
        "{}/oauth/consent#{}",
        state.frontend_origin, query
    )) {
        Ok(value) => value,
        Err(_) => return error(StatusCode::BAD_REQUEST),
    };
    (
        StatusCode::SEE_OTHER,
        [
            (LOCATION, location),
            (CACHE_CONTROL, HeaderValue::from_static("no-store")),
            (PRAGMA, HeaderValue::from_static("no-cache")),
            (REFERRER_POLICY, HeaderValue::from_static("no-referrer")),
            (X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff")),
        ],
    )
        .into_response()
}
