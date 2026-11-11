use std::{collections::BTreeSet, sync::Arc, time::Duration};

use axum::{
    Router,
    http::{
        HeaderName, HeaderValue, Method,
        header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE},
    },
    middleware,
};
use bastion_core::CoreService;
use bastion_cotizaciones::QuoteService;
use bastion_facturas::InvoiceService;
use bastion_licitaciones::TenderService;
use bastion_mcp::{ActorQuota, MysqlQuota, MysqlTokenExchange, ToolQuota};
use bastion_mcp_core::{
    CoreScopePolicy, DraftReadExecutor, approved_read_requirements, draft_read_tool_registry,
};
use bastion_ordenes_compra::OrderService;
use tower_http::cors::{AllowOrigin, Any, CorsLayer};
use url::Url;

use crate::{
    ClientMetadataSource, DraftAuthorizationState, DraftClientRequestState, DraftConsentState,
    DraftMcpState, DraftMetadataIntegrity, DraftOauthMetadata, DraftPerimeter, TokenEndpoint,
    draft_authorization_router, draft_client_request_router, draft_consent_router,
    draft_mcp_router, draft_oauth_metadata_router, draft_token_router,
};

pub struct DraftCompositionConfig {
    pub resource: String,
    pub issuer: String,
    pub authorization_endpoint: String,
    pub token_endpoint: String,
    pub frontend_origin: String,
    pub client_origins: BTreeSet<String>,
    pub actor_quota: ActorQuota,
    pub tool_quota: ToolQuota,
    pub max_concurrent: usize,
    pub max_metadata_fetches: usize,
    pub request_timeout: Duration,
    pub perimeter: DraftPerimeter,
}
#[derive(Debug)]
pub struct DraftCompositionError;

/// Creates a fully isolated router for integration tests. It must not be
/// mounted in the live application until authorization entry, client metadata
/// refresh, edge limits, operational controls and interoperability are done.
pub fn compose_draft_router<F: ClientMetadataSource>(
    core: CoreService,
    config: DraftCompositionConfig,
    fetcher: F,
) -> Result<Router, DraftCompositionError> {
    let metadata = DraftOauthMetadata::new(
        config.resource.clone(),
        config.issuer.clone(),
        config.authorization_endpoint.clone(),
        config.token_endpoint.clone(),
    )
    .map_err(|_| DraftCompositionError)?;
    let resource = Url::parse(&config.resource).map_err(|_| DraftCompositionError)?;
    let issuer = Url::parse(&config.issuer).map_err(|_| DraftCompositionError)?;
    let authorization =
        Url::parse(&config.authorization_endpoint).map_err(|_| DraftCompositionError)?;
    let token = Url::parse(&config.token_endpoint).map_err(|_| DraftCompositionError)?;
    if resource.path() != "/mcp"
        || resource.origin() != issuer.origin()
        || authorization.path() != "/oauth/authorize"
        || token.path() != "/oauth/token"
        || !config.perimeter.has_shared_limit()
        || config.tool_quota.user_tool == 0
        || config.tool_quota.client_tool == 0
    {
        return Err(DraftCompositionError);
    }
    let frontend =
        HeaderValue::from_str(&config.frontend_origin).map_err(|_| DraftCompositionError)?;
    let client_origins: Vec<HeaderValue> = config
        .client_origins
        .iter()
        .map(|origin| HeaderValue::from_str(origin).map_err(|_| DraftCompositionError))
        .collect::<Result<_, _>>()?;
    let db = core.database();
    let pool = db.pool().clone();
    let admin = core.admin();
    let policy = CoreScopePolicy::new(admin.clone(), approved_read_requirements())
        .map_err(|_| DraftCompositionError)?;
    let exchange = MysqlTokenExchange::new(pool.clone(), policy.clone());
    let quota =
        MysqlQuota::new(pool.clone(), config.actor_quota).map_err(|_| DraftCompositionError)?;
    let integrity = Arc::new(
        DraftMetadataIntegrity::new(pool.clone(), fetcher.clone(), config.max_metadata_fetches)
            .map_err(|_| DraftCompositionError)?,
    );
    let registry = Arc::new(draft_read_tool_registry().map_err(|_| DraftCompositionError)?);
    let quote_mail =
        bastion_mcp_core::QuoteMailConfirmation::new(core.clone(), config.frontend_origin.clone());
    let quote_mail_state = crate::quote_mail_http::QuoteMailState {
        core: core.clone(),
        origin: config.frontend_origin.clone(),
        mail: quote_mail.clone(),
    };
    let executor = Arc::new(
        DraftReadExecutor::new(
            QuoteService::new(db.clone(), admin.clone()),
            InvoiceService::new(db.clone(), admin.clone()),
            OrderService::new(db.clone(), admin.clone()),
        )
        .with_tenders(TenderService::new(db.clone(), admin.clone()))
        .with_additional_reads(db, admin)
        .with_quote_mail(quote_mail),
    );
    let mcp = DraftMcpState::new(
        pool.clone(),
        exchange.clone(),
        config.resource.clone(),
        metadata.protected_resource_metadata_url().to_owned(),
        config.client_origins.clone(),
        registry,
        quota,
        config.tool_quota,
        config.max_concurrent,
        config.request_timeout,
    )
    .map_err(|_| DraftCompositionError)?
    .with_read_executor(executor)
    .with_integrity_guard(integrity.clone());
    let consent = DraftConsentState::new(
        pool.clone(),
        core.clone(),
        policy,
        Arc::new(metadata.clone()),
        config.frontend_origin.clone(),
    )
    .map_err(|_| DraftCompositionError)?;
    let authorization = DraftAuthorizationState::new(
        pool.clone(),
        config.resource.clone(),
        config.frontend_origin.clone(),
        config.client_origins.clone(),
        fetcher.clone(),
        config.max_metadata_fetches,
    )
    .map_err(|_| DraftCompositionError)?;
    let requests = DraftClientRequestState::new(
        pool,
        core,
        config.frontend_origin,
        fetcher,
        config.max_metadata_fetches,
    )
    .map_err(|_| DraftCompositionError)?;
    let browser_cors = CorsLayer::new()
        .allow_origin(AllowOrigin::list([frontend]))
        .allow_credentials(true)
        .allow_methods([Method::POST, Method::OPTIONS])
        .allow_headers([CONTENT_TYPE, HeaderName::from_static("x-csrf-token")]);
    let client_cors = CorsLayer::new()
        .allow_origin(client_origins)
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers([
            ACCEPT,
            AUTHORIZATION,
            CONTENT_TYPE,
            HeaderName::from_static("mcp-protocol-version"),
            HeaderName::from_static("mcp-method"),
            HeaderName::from_static("mcp-name"),
        ]);
    let discovery_cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::OPTIONS]);
    let browser = draft_consent_router(consent)
        .merge(crate::quote_mail_http::quote_mail_router(quote_mail_state))
        .merge(draft_client_request_router(requests))
        .layer(browser_cors);
    let protocol = draft_mcp_router(mcp)
        .merge(draft_authorization_router(authorization))
        .merge(draft_token_router(
            TokenEndpoint::new(exchange, config.resource, config.client_origins)
                .with_integrity_guard(integrity),
        ))
        .layer(client_cors);
    let discovery = draft_oauth_metadata_router(metadata).layer(discovery_cors);
    Ok(browser
        .merge(protocol)
        .merge(discovery)
        .layer(middleware::from_fn_with_state(
            config.perimeter,
            crate::perimeter::perimeter_guard,
        )))
}
