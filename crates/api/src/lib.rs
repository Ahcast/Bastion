#![forbid(unsafe_code)]

use std::{
    io,
    net::{IpAddr, SocketAddr},
    time::Duration,
};

use axum::{
    Json, Router,
    extract::{ConnectInfo, Extension, Path, Query, Request, State},
    http::{
        HeaderName, HeaderValue, Method, StatusCode,
        header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE, COOKIE, ORIGIN},
    },
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use bastion_core::{AuthCookies, CoreError, CoreService, LoginStart};
use ipnet::IpNet;
use serde::{Deserialize, Serialize};
use subtle::ConstantTimeEq;
use tower_http::{
    cors::CorsLayer,
    limit::RequestBodyLimitLayer,
    request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer},
    set_header::SetResponseHeaderLayer,
};
use url::Url;

mod clientes;
mod colaboracion;
mod cotizaciones;
mod facturas;
mod gastos;
mod ordenes_compra;
mod procesos;
mod proveedores;
mod reportes;
mod sar;

const GENERAL_REQUEST_BODY_LIMIT: usize = 1024 * 1024;

#[derive(Clone)]
struct ApiState {
    core: CoreService,
    procesos: bastion_procesos::ProcessService,
    cotizaciones: bastion_cotizaciones::QuoteService,
    clientes: bastion_clientes::ClientService,
    proveedores: bastion_proveedores::SupplierService,
    colaboracion: bastion_colaboracion::CollaborationService,
    facturas: bastion_facturas::InvoiceService,
    ordenes_compra: bastion_ordenes_compra::OrderService,
    gastos: bastion_gastos::ExpenseService,
    reportes: bastion_reportes::ReportService,
    report_jobs: std::sync::Arc<tokio::sync::Semaphore>,
    procesos_jobs: std::sync::Arc<tokio::sync::Semaphore>,
    scout_csv_path: std::path::PathBuf,
    scout_control: bastion_procesos::ScoutControl,
    scout_control_jobs: std::sync::Arc<tokio::sync::Semaphore>,
}
#[derive(Clone)]
struct CsrfGuardState {
    core: CoreService,
    origin: String,
    trusted_proxy_cidrs: Vec<IpNet>,
}
#[derive(Clone)]
struct ClientIp(String);

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
    dependency: Option<&'static str>,
}

#[derive(Serialize)]
struct ProblemDetails {
    #[serde(rename = "type")]
    problem_type: &'static str,
    title: &'static str,
    status: u16,
    detail: &'static str,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BootstrapRequest {
    bootstrap_secret: String,
    full_name: String,
    email: String,
    password: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BootstrapCompleteRequest {
    setup_token: String,
    code: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LoginRequest {
    email: String,
    password: String,
    keep_me_logged_in: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TotpRequest {
    code: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ProfileRequest {
    full_name: String,
    dni: Option<String>,
    country_code: Option<String>,
    gender: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PasswordChangeRequest {
    current_password: String,
    new_password: String,
    totp_code: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SmtpCredentialRequest {
    smtp_email: String,
    smtp_password: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct InvitationAcceptRequest {
    token: String,
    password: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PasswordResetRequest {
    email: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PasswordResetCompleteRequest {
    token: String,
    new_password: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ListQuery {
    q: Option<String>,
    page: Option<u32>,
    page_size: Option<u32>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PersonRequest {
    full_name: String,
    dni: Option<String>,
    country_code: Option<String>,
    gender: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateUserRequest {
    person_id: u64,
    role_id: u64,
    email: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UpdateUserRequest {
    person_id: Option<u64>,
    role_id: Option<u64>,
    enabled: Option<bool>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateRoleRequest {
    name: String,
    description: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PermissionRequest {
    module_id: String,
    scope_key: String,
    capability: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UpdateRoleRequest {
    name: Option<String>,
    description: Option<String>,
    permissions: Option<Vec<PermissionRequest>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ModuleToggleRequest {
    enabled: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CoreSettingRequest {
    value: serde_json::Value,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AuditQuery {
    from: Option<String>,
    to: Option<String>,
    module: Option<String>,
    action: Option<String>,
    result: Option<String>,
    page: Option<u32>,
    page_size: Option<u32>,
}

pub fn router(
    core: CoreService,
    allowed_origin: String,
    trusted_proxy_cidrs: Vec<IpNet>,
    scout_csv_path: std::path::PathBuf,
) -> io::Result<Router> {
    let allowed_origin = validate_origin(&allowed_origin)?;
    let origin_for_guard = allowed_origin
        .to_str()
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "invalid CORS origin"))?
        .to_owned();
    let request_id = HeaderName::from_static("x-request-id");
    let cors = CorsLayer::new()
        .allow_origin(allowed_origin)
        .allow_methods([
            Method::GET,
            Method::HEAD,
            Method::POST,
            Method::PUT,
            Method::PATCH,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([
            ACCEPT,
            AUTHORIZATION,
            CONTENT_TYPE,
            HeaderName::from_static("x-csrf-token"),
            HeaderName::from_static("idempotency-key"),
            HeaderName::from_static("if-match"),
        ])
        .expose_headers([
            HeaderName::from_static("etag"),
            HeaderName::from_static("x-request-id"),
            HeaderName::from_static("content-disposition"),
        ])
        .allow_credentials(true)
        .max_age(Duration::from_secs(600));
    let api_state = ApiState {
        core: core.clone(),
        procesos: bastion_procesos::ProcessService::new(core.database(), core.admin()),
        cotizaciones: bastion_cotizaciones::QuoteService::new(core.database(), core.admin()),
        clientes: bastion_clientes::ClientService::new(core.database(), core.admin()),
        proveedores: bastion_proveedores::SupplierService::new(core.database(), core.admin()),
        colaboracion: bastion_colaboracion::CollaborationService::new(
            core.database(),
            core.admin(),
        ),
        facturas: bastion_facturas::InvoiceService::new(core.database(), core.admin()),
        ordenes_compra: bastion_ordenes_compra::OrderService::new(core.database(), core.admin()),
        gastos: bastion_gastos::ExpenseService::new(core.database(), core.admin()),
        reportes: bastion_reportes::ReportService::new(core.database(), core.admin()),
        report_jobs: std::sync::Arc::new(tokio::sync::Semaphore::new(2)),
        procesos_jobs: std::sync::Arc::new(tokio::sync::Semaphore::new(1)),
        scout_csv_path,
        scout_control: bastion_procesos::ScoutControl::from_env()?,
        scout_control_jobs: std::sync::Arc::new(tokio::sync::Semaphore::new(1)),
    };
    Ok(Router::new()
        .route("/health/live", get(liveness))
        .route("/health/ready", get(readiness))
        .route("/api/v1/auth/csrf", get(issue_csrf))
        .route("/api/v1/bootstrap/status", get(bootstrap_status))
        .route("/api/v1/bootstrap", post(begin_bootstrap))
        .route("/api/v1/bootstrap/complete", post(complete_bootstrap))
        .route("/api/v1/auth/activate", post(begin_invitation_activation))
        .route(
            "/api/v1/auth/activate/complete",
            post(complete_invitation_activation),
        )
        .route("/api/v1/auth/password-reset", post(request_password_reset))
        .route(
            "/api/v1/auth/password-reset/complete",
            post(complete_password_reset),
        )
        .route("/api/v1/auth/login", post(start_login))
        .route("/api/v1/auth/totp", post(verify_login_totp))
        .route("/api/v1/auth/recovery-code", post(verify_login_recovery))
        .route("/api/v1/auth/reauth-totp", post(reauth_totp))
        .route("/api/v1/auth/me", get(current_user))
        .route("/api/v1/auth/access", get(access_catalog))
        .route("/api/v1/auth/logout", post(logout))
        .route("/api/v1/persons", get(list_persons).post(create_person))
        .route(
            "/api/v1/persons/{id}",
            axum::routing::patch(update_person).delete(delete_person),
        )
        .route("/api/v1/admin/users", get(list_users).post(create_user))
        .route(
            "/api/v1/admin/users/{id}",
            axum::routing::patch(update_user),
        )
        .route(
            "/api/v1/admin/users/{id}/invitation",
            post(resend_invitation),
        )
        .route("/api/v1/admin/roles", get(list_roles).post(create_role))
        .route(
            "/api/v1/admin/roles/{id}",
            axum::routing::patch(update_role).delete(delete_role),
        )
        .route(
            "/api/v1/admin/roles/{id}/permissions",
            get(list_role_permissions),
        )
        .route("/api/v1/settings/core", get(get_core_settings))
        .route("/api/v1/settings/sar", get(sar::get).put(sar::put))
        .route(
            "/api/v1/settings/core/{key}",
            axum::routing::put(set_core_setting),
        )
        .route("/api/v1/settings/modules", get(list_modules))
        .route(
            "/api/v1/settings/modules/{module_id}",
            axum::routing::patch(toggle_module),
        )
        .route("/api/v1/settings/audit", get(list_audit))
        .route("/api/v1/settings/mail-outbox", get(list_mail_outbox))
        .route("/api/v1/settings/mail-outbox/{id}/retry", post(retry_mail))
        .route("/api/v1/account/sessions", get(list_sessions))
        .route(
            "/api/v1/account/sessions/{id}",
            axum::routing::delete(revoke_session),
        )
        .route(
            "/api/v1/account/sessions/revoke-others",
            post(revoke_other_sessions),
        )
        .route("/api/v1/account/devices", get(list_devices))
        .route(
            "/api/v1/account/devices/{id}",
            axum::routing::delete(revoke_device),
        )
        .route(
            "/api/v1/account/security-events",
            get(list_security_history),
        )
        .route(
            "/api/v1/account/profile",
            get(get_profile).put(update_profile),
        )
        .route("/api/v1/account/password", post(change_password))
        .route(
            "/api/v1/account/smtp",
            get(get_smtp_credentials).put(save_smtp_credentials),
        )
        .layer(RequestBodyLimitLayer::new(GENERAL_REQUEST_BODY_LIMIT))
        .merge(procesos::routes(api_state.clone()))
        .merge(cotizaciones::routes(api_state.clone()))
        .merge(clientes::routes(api_state.clone()))
        .merge(proveedores::routes(api_state.clone()))
        .merge(colaboracion::routes(api_state.clone()))
        .merge(facturas::routes(api_state.clone()))
        .merge(ordenes_compra::routes(api_state.clone()))
        .merge(gastos::routes(api_state.clone()))
        .merge(reportes::routes(api_state.clone()))
        .fallback(not_found)
        .with_state(api_state)
        .layer(middleware::from_fn(problem_details))
        .layer(middleware::from_fn_with_state(
            CsrfGuardState {
                core: core.clone(),
                origin: origin_for_guard,
                trusted_proxy_cidrs,
            },
            csrf_guard,
        ))
        .layer(cors)
        .layer(PropagateRequestIdLayer::new(request_id.clone()))
        .layer(SetRequestIdLayer::new(request_id.clone(), MakeRequestUuid))
        .layer(middleware::from_fn(strip_untrusted_request_id))
        .layer(SetResponseHeaderLayer::overriding(
            HeaderName::from_static("cache-control"),
            HeaderValue::from_static("no-store"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            HeaderName::from_static("x-content-type-options"),
            HeaderValue::from_static("nosniff"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            HeaderName::from_static("x-frame-options"),
            HeaderValue::from_static("DENY"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            HeaderName::from_static("referrer-policy"),
            HeaderValue::from_static("no-referrer"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            HeaderName::from_static("content-security-policy"),
            HeaderValue::from_static("default-src 'none'; frame-ancestors 'none'; base-uri 'none'"),
        )))
}

fn validate_origin(value: &str) -> io::Result<HeaderValue> {
    let invalid = || {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid CORS origin configuration",
        )
    };
    let parsed = Url::parse(value).map_err(|_| invalid())?;
    let local_http = parsed.scheme() == "http"
        && matches!(
            parsed.host_str(),
            Some("localhost") | Some("127.0.0.1") | Some("::1")
        );
    if (parsed.scheme() != "https" && !local_http)
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.path() != "/"
        || parsed.query().is_some()
        || parsed.fragment().is_some()
    {
        return Err(invalid());
    }
    let origin = parsed.origin().ascii_serialization();
    if origin != value {
        return Err(invalid());
    }
    HeaderValue::from_str(&origin).map_err(|_| invalid())
}

async fn liveness() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        dependency: None,
    })
}
async fn readiness(State(state): State<ApiState>) -> (StatusCode, Json<HealthResponse>) {
    match state.core.check_ready().await {
        Ok(()) => (
            StatusCode::OK,
            Json(HealthResponse {
                status: "ok",
                dependency: Some("mysql"),
            }),
        ),
        Err(_) => {
            tracing::warn!(dependency = "mysql", "readiness check failed");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(HealthResponse {
                    status: "unavailable",
                    dependency: Some("mysql"),
                }),
            )
        }
    }
}
async fn not_found() -> StatusCode {
    StatusCode::NOT_FOUND
}

async fn issue_csrf(headers: axum::http::HeaderMap) -> Response {
    // Reuse the browser's token: replacing it on reload or in another tab
    // would break the hash bound to an existing authenticated session.
    let existing = reusable_csrf_token(&headers);
    let token = existing.clone().unwrap_or_else(random_token);
    let mut response = Json(serde_json::json!({"csrf_token":token.clone()})).into_response();
    if existing.is_none() {
        set_cookie(
            &mut response,
            "__Host-bastion_csrf",
            &token,
            "Path=/; Secure; SameSite=Lax",
            None,
        );
    }
    response
}

fn reusable_csrf_token(headers: &axum::http::HeaderMap) -> Option<String> {
    let token = cookie_value(headers, "__Host-bastion_csrf")?;
    let decoded =
        base64::Engine::decode(&base64::engine::general_purpose::URL_SAFE_NO_PAD, &token).ok()?;
    (decoded.len() == 32).then_some(token)
}

fn requires_session_csrf_binding(method: &Method, path: &str) -> bool {
    // These operations establish a new identity using their own password,
    // challenge or one-time token. A stale/revoked session cookie must not
    // prevent them. Exact Origin and cookie/header CSRF checks still apply.
    !(method == Method::POST
        && matches!(
            path,
            "/api/v1/bootstrap"
                | "/api/v1/bootstrap/complete"
                | "/api/v1/auth/activate"
                | "/api/v1/auth/activate/complete"
                | "/api/v1/auth/password-reset"
                | "/api/v1/auth/password-reset/complete"
                | "/api/v1/auth/login"
                | "/api/v1/auth/totp"
                | "/api/v1/auth/recovery-code"
        ))
}

async fn bootstrap_status(
    State(state): State<ApiState>,
) -> Result<Json<serde_json::Value>, ApiError> {
    Ok(Json(
        serde_json::json!({"setup_required":state.core.bootstrap_status().await.map_err(ApiError)?}),
    ))
}

async fn begin_bootstrap(
    State(state): State<ApiState>,
    Json(body): Json<BootstrapRequest>,
) -> Result<Response, ApiError> {
    let setup = state
        .core
        .begin_bootstrap(
            &body.bootstrap_secret,
            &body.full_name,
            &body.email,
            &body.password,
        )
        .await
        .map_err(ApiError)?;
    Ok(Json(setup).into_response())
}

async fn complete_bootstrap(
    State(state): State<ApiState>,
    Json(body): Json<BootstrapCompleteRequest>,
) -> Result<Json<bastion_core::BootstrapComplete>, ApiError> {
    state
        .core
        .complete_bootstrap(&body.setup_token, &body.code)
        .await
        .map(Json)
        .map_err(ApiError)
}

async fn start_login(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Extension(client_ip): Extension<ClientIp>,
    Json(body): Json<LoginRequest>,
) -> Result<Response, ApiError> {
    let ip = client_ip.0;
    let agent = headers
        .get("user-agent")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let device = cookie_value(&headers, "__Host-bastion_device");
    match state
        .core
        .start_login(
            &body.email,
            &body.password,
            &ip,
            agent,
            body.keep_me_logged_in,
            device.as_deref(),
        )
        .await
        .map_err(ApiError)?
    {
        LoginStart::TotpRequired { challenge_token } => {
            let mut response =
                Json(serde_json::json!({"status":"totp_required","expires_in_seconds":300}))
                    .into_response();
            set_cookie(
                &mut response,
                "__Host-bastion_challenge",
                &challenge_token,
                "Path=/; Secure; HttpOnly; SameSite=Lax",
                Some(300),
            );
            Ok(response)
        }
        LoginStart::Authenticated(cookies) => authenticated_response(&state.core, cookies).await,
    }
}

async fn verify_login_totp(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Json(body): Json<TotpRequest>,
) -> Result<Response, ApiError> {
    let challenge = cookie_value(&headers, "__Host-bastion_challenge")
        .ok_or(ApiError(CoreError::Unauthorized))?;
    let device = cookie_value(&headers, "__Host-bastion_device");
    let cookies = state
        .core
        .verify_login_totp(&challenge, &body.code, device.as_deref())
        .await
        .map_err(ApiError)?;
    authenticated_response(&state.core, cookies).await
}

async fn verify_login_recovery(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Json(body): Json<TotpRequest>,
) -> Result<Response, ApiError> {
    let challenge = cookie_value(&headers, "__Host-bastion_challenge")
        .ok_or(ApiError(CoreError::Unauthorized))?;
    let device = cookie_value(&headers, "__Host-bastion_device");
    let cookies = state
        .core
        .verify_login_recovery_code(&challenge, &body.code, device.as_deref())
        .await
        .map_err(ApiError)?;
    authenticated_response(&state.core, cookies).await
}

async fn current_user(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
    let session = cookie_value(&headers, "__Host-bastion_session")
        .ok_or(ApiError(CoreError::Unauthorized))?;
    let user = state.core.current_user(&session).await.map_err(ApiError)?;
    Ok(Json(serde_json::json!({"user":user})))
}

async fn access_catalog(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
) -> Result<Json<bastion_core::AccessCatalog>, ApiError> {
    let session = cookie_value(&headers, "__Host-bastion_session")
        .ok_or(ApiError(CoreError::Unauthorized))?;
    state
        .core
        .access_catalog(&session)
        .await
        .map(Json)
        .map_err(ApiError)
}

async fn get_profile(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
) -> Result<Json<bastion_core::ProfileInfo>, ApiError> {
    let session = cookie_value(&headers, "__Host-bastion_session")
        .ok_or(ApiError(CoreError::Unauthorized))?;
    state
        .core
        .profile(&session)
        .await
        .map(Json)
        .map_err(ApiError)
}

async fn update_profile(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Json(body): Json<ProfileRequest>,
) -> Result<Json<bastion_core::ProfileInfo>, ApiError> {
    let session = cookie_value(&headers, "__Host-bastion_session")
        .ok_or(ApiError(CoreError::Unauthorized))?;
    let input = bastion_core::PersonInput {
        full_name: body.full_name,
        dni: body.dni,
        country_code: body.country_code,
        gender: body.gender,
    };
    state
        .core
        .update_profile(&session, input)
        .await
        .map(Json)
        .map_err(ApiError)
}

async fn change_password(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Extension(client_ip): Extension<ClientIp>,
    Json(body): Json<PasswordChangeRequest>,
) -> Result<Response, ApiError> {
    let session = cookie_value(&headers, "__Host-bastion_session")
        .ok_or(ApiError(CoreError::Unauthorized))?;
    let agent = headers
        .get("user-agent")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    state
        .core
        .change_password(
            &session,
            &body.current_password,
            &body.new_password,
            &body.totp_code,
            &client_ip.0,
            agent,
        )
        .await
        .map_err(ApiError)?;
    let mut response = StatusCode::NO_CONTENT.into_response();
    for name in [
        "__Host-bastion_session",
        "__Host-bastion_device",
        "__Host-bastion_challenge",
    ] {
        set_cookie(
            &mut response,
            name,
            "",
            "Path=/; Secure; HttpOnly; SameSite=Lax",
            Some(0),
        );
    }
    Ok(response)
}

async fn get_smtp_credentials(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
    let session = cookie_value(&headers, "__Host-bastion_session")
        .ok_or(ApiError(CoreError::Unauthorized))?;
    let settings = state
        .core
        .smtp_credentials(&session)
        .await
        .map_err(ApiError)?;
    Ok(Json(serde_json::json!({"credentials":settings})))
}

async fn save_smtp_credentials(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Json(body): Json<SmtpCredentialRequest>,
) -> Result<Json<bastion_core::SmtpCredentialInfo>, ApiError> {
    let session = cookie_value(&headers, "__Host-bastion_session")
        .ok_or(ApiError(CoreError::Unauthorized))?;
    state
        .core
        .save_smtp_credentials(&session, &body.smtp_email, body.smtp_password.as_deref())
        .await
        .map(Json)
        .map_err(ApiError)
}

async fn begin_invitation_activation(
    State(state): State<ApiState>,
    Json(body): Json<InvitationAcceptRequest>,
) -> Result<Json<bastion_core::BootstrapBegin>, ApiError> {
    state
        .core
        .begin_invitation_activation(&body.token, &body.password)
        .await
        .map(Json)
        .map_err(ApiError)
}

async fn complete_invitation_activation(
    State(state): State<ApiState>,
    Json(body): Json<BootstrapCompleteRequest>,
) -> Result<Json<bastion_core::BootstrapComplete>, ApiError> {
    state
        .core
        .complete_invitation_activation(&body.setup_token, &body.code)
        .await
        .map(Json)
        .map_err(ApiError)
}

async fn request_password_reset(
    State(state): State<ApiState>,
    Extension(client_ip): Extension<ClientIp>,
    Json(body): Json<PasswordResetRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>), ApiError> {
    state
        .core
        .request_password_reset(&body.email, &client_ip.0)
        .await
        .map_err(ApiError)?;
    Ok((
        StatusCode::ACCEPTED,
        Json(serde_json::json!({"status":"if_account_exists_email_will_be_sent"})),
    ))
}

async fn complete_password_reset(
    State(state): State<ApiState>,
    Extension(client_ip): Extension<ClientIp>,
    Json(body): Json<PasswordResetCompleteRequest>,
) -> Result<Response, ApiError> {
    state
        .core
        .complete_password_reset(&body.token, &body.new_password, &client_ip.0)
        .await
        .map_err(ApiError)?;
    Ok(StatusCode::NO_CONTENT.into_response())
}

async fn reauth_totp(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Extension(client_ip): Extension<ClientIp>,
    Json(body): Json<TotpRequest>,
) -> Result<StatusCode, ApiError> {
    let session = cookie_value(&headers, "__Host-bastion_session")
        .ok_or(ApiError(CoreError::Unauthorized))?;
    let agent = headers
        .get("user-agent")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    state
        .core
        .verify_session_totp(&session, &body.code, &client_ip.0, agent)
        .await
        .map_err(ApiError)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn logout(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
) -> Result<Response, ApiError> {
    let session = cookie_value(&headers, "__Host-bastion_session").unwrap_or_default();
    state.core.logout(&session).await.map_err(ApiError)?;
    let mut response = StatusCode::NO_CONTENT.into_response();
    for name in ["__Host-bastion_session", "__Host-bastion_challenge"] {
        set_cookie(
            &mut response,
            name,
            "",
            "Path=/; Secure; HttpOnly; SameSite=Lax",
            Some(0),
        );
    }
    Ok(response)
}

async fn actor(
    state: &ApiState,
    headers: &axum::http::HeaderMap,
) -> Result<bastion_core::AuthenticatedUser, ApiError> {
    let token =
        cookie_value(headers, "__Host-bastion_session").ok_or(ApiError(CoreError::Unauthorized))?;
    state.core.current_user(&token).await.map_err(ApiError)
}

fn page(query: &ListQuery) -> (u32, u32) {
    (query.page.unwrap_or(1), query.page_size.unwrap_or(25))
}
fn person_input(body: PersonRequest) -> bastion_core::PersonInput {
    bastion_core::PersonInput {
        full_name: body.full_name,
        dni: body.dni,
        country_code: body.country_code,
        gender: body.gender,
    }
}

async fn list_persons(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Query(query): Query<ListQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let actor = actor(&state, &headers).await?;
    let (page, size) = page(&query);
    let items = state
        .core
        .admin()
        .list_persons(actor.id, query.q.as_deref(), page, size)
        .await
        .map_err(ApiError)?;
    Ok(Json(
        serde_json::json!({"items":items,"page":page.max(1),"page_size":size.clamp(1,100)}),
    ))
}

async fn create_person(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Json(body): Json<PersonRequest>,
) -> Result<Response, ApiError> {
    let actor = actor(&state, &headers).await?;
    let item = state
        .core
        .admin()
        .create_person(actor.id, person_input(body))
        .await
        .map_err(ApiError)?;
    let mut response = Json(item).into_response();
    *response.status_mut() = StatusCode::CREATED;
    Ok(response)
}

async fn update_person(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Path(id): Path<u64>,
    Json(body): Json<PersonRequest>,
) -> Result<Json<bastion_core::PersonRecord>, ApiError> {
    let actor = actor(&state, &headers).await?;
    state
        .core
        .admin()
        .update_person(actor.id, id, person_input(body))
        .await
        .map(Json)
        .map_err(ApiError)
}

async fn delete_person(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Path(id): Path<u64>,
) -> Result<StatusCode, ApiError> {
    let actor = actor(&state, &headers).await?;
    state
        .core
        .admin()
        .delete_person(actor.id, id)
        .await
        .map_err(ApiError)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn list_users(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Query(query): Query<ListQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let actor = actor(&state, &headers).await?;
    let (page, size) = page(&query);
    let items = state
        .core
        .admin()
        .list_users(actor.id, query.q.as_deref(), page, size)
        .await
        .map_err(ApiError)?;
    Ok(Json(
        serde_json::json!({"items":items,"page":page.max(1),"page_size":size.clamp(1,100)}),
    ))
}

async fn create_user(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Json(body): Json<CreateUserRequest>,
) -> Result<Response, ApiError> {
    let actor = actor(&state, &headers).await?;
    let item = state
        .core
        .create_user_with_invitation(actor.id, body.person_id, body.role_id, &body.email)
        .await
        .map_err(ApiError)?;
    let mut response = Json(item).into_response();
    *response.status_mut() = StatusCode::CREATED;
    Ok(response)
}

async fn resend_invitation(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Path(id): Path<u64>,
) -> Result<StatusCode, ApiError> {
    let actor = actor(&state, &headers).await?;
    state
        .core
        .dispatch_invitation(actor.id, id)
        .await
        .map_err(ApiError)?;
    Ok(StatusCode::ACCEPTED)
}

async fn update_user(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Path(id): Path<u64>,
    Json(body): Json<UpdateUserRequest>,
) -> Result<Json<bastion_core::UserRecord>, ApiError> {
    let actor = actor(&state, &headers).await?;
    state
        .core
        .admin()
        .update_user(actor.id, id, body.person_id, body.role_id, body.enabled)
        .await
        .map(Json)
        .map_err(ApiError)
}

async fn list_roles(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
) -> Result<Json<Vec<bastion_core::RoleRecord>>, ApiError> {
    let actor = actor(&state, &headers).await?;
    state
        .core
        .admin()
        .list_roles(actor.id)
        .await
        .map(Json)
        .map_err(ApiError)
}

async fn list_role_permissions(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Path(id): Path<u64>,
) -> Result<Json<Vec<bastion_core::RolePermissionInput>>, ApiError> {
    let actor = actor(&state, &headers).await?;
    state
        .core
        .admin()
        .list_role_permissions(actor.id, id)
        .await
        .map(Json)
        .map_err(ApiError)
}

async fn create_role(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Json(body): Json<CreateRoleRequest>,
) -> Result<Response, ApiError> {
    let actor = actor(&state, &headers).await?;
    let role = state
        .core
        .admin()
        .create_role(actor.id, &body.name, body.description.as_deref())
        .await
        .map_err(ApiError)?;
    let mut response = Json(role).into_response();
    *response.status_mut() = StatusCode::CREATED;
    Ok(response)
}

async fn update_role(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Path(id): Path<u64>,
    Json(body): Json<UpdateRoleRequest>,
) -> Result<Json<bastion_core::RoleRecord>, ApiError> {
    let actor = actor(&state, &headers).await?;
    let permissions = body.permissions.map(|items| {
        items
            .into_iter()
            .map(|p| bastion_core::RolePermissionInput {
                module_id: p.module_id,
                scope_key: p.scope_key,
                capability: p.capability,
            })
            .collect()
    });
    state
        .core
        .admin()
        .update_role(
            actor.id,
            id,
            body.name.as_deref(),
            body.description.as_deref().map(Some),
            permissions,
        )
        .await
        .map(Json)
        .map_err(ApiError)
}

async fn delete_role(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Path(id): Path<u64>,
) -> Result<StatusCode, ApiError> {
    let actor = actor(&state, &headers).await?;
    state
        .core
        .admin()
        .delete_role(actor.id, id)
        .await
        .map_err(ApiError)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn list_modules(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
) -> Result<Json<Vec<bastion_core::ModuleRecord>>, ApiError> {
    let actor = actor(&state, &headers).await?;
    state
        .core
        .admin()
        .list_modules(actor.id)
        .await
        .map(Json)
        .map_err(ApiError)
}

async fn toggle_module(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Path(module_id): Path<String>,
    Json(body): Json<ModuleToggleRequest>,
) -> Result<StatusCode, ApiError> {
    let actor = actor(&state, &headers).await?;
    state
        .core
        .admin()
        .set_module_enabled(actor.id, &module_id, body.enabled)
        .await
        .map_err(ApiError)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn get_core_settings(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
) -> Result<Json<Vec<bastion_core::SettingRecord>>, ApiError> {
    let actor = actor(&state, &headers).await?;
    state
        .core
        .admin()
        .get_core_settings(actor.id)
        .await
        .map(Json)
        .map_err(ApiError)
}

async fn set_core_setting(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Path(key): Path<String>,
    Json(body): Json<CoreSettingRequest>,
) -> Result<StatusCode, ApiError> {
    let actor = actor(&state, &headers).await?;
    state
        .core
        .admin()
        .set_core_setting(actor.id, &key, body.value)
        .await
        .map_err(ApiError)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn list_audit(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Query(query): Query<AuditQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let actor = actor(&state, &headers).await?;
    let from = query
        .from
        .map(parse_datetime)
        .transpose()
        .map_err(|_| ApiError(CoreError::Invalid))?;
    let to = query
        .to
        .map(parse_datetime)
        .transpose()
        .map_err(|_| ApiError(CoreError::Invalid))?;
    let page = query.page.unwrap_or(1);
    let size = query.page_size.unwrap_or(25);
    let items = state
        .core
        .admin()
        .list_audit(
            actor.id,
            from,
            to,
            query.module.as_deref(),
            query.action.as_deref(),
            query.result.as_deref(),
            page,
            size,
        )
        .await
        .map_err(ApiError)?;
    Ok(Json(
        serde_json::json!({"items":items,"page":page.max(1),"page_size":size.clamp(1,100)}),
    ))
}

async fn list_mail_outbox(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Query(query): Query<ListQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let actor = actor(&state, &headers).await?;
    let (page, size) = page(&query);
    let items = state
        .core
        .admin()
        .list_mail_outbox(actor.id, page, size)
        .await
        .map_err(ApiError)?;
    Ok(Json(
        serde_json::json!({"items":items,"page":page.max(1),"page_size":size.clamp(1,100)}),
    ))
}

async fn retry_mail(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Path(id): Path<u64>,
) -> Result<StatusCode, ApiError> {
    let actor = actor(&state, &headers).await?;
    state
        .core
        .admin()
        .retry_mail(actor.id, id)
        .await
        .map_err(ApiError)?;
    Ok(StatusCode::ACCEPTED)
}

async fn list_sessions(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
) -> Result<Json<Vec<bastion_core::SessionInfo>>, ApiError> {
    let user = actor(&state, &headers).await?;
    let token = cookie_value(&headers, "__Host-bastion_session")
        .ok_or(ApiError(CoreError::Unauthorized))?;
    state
        .core
        .list_sessions(user.id, &token)
        .await
        .map(Json)
        .map_err(ApiError)
}

async fn revoke_session(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Path(id): Path<u64>,
) -> Result<StatusCode, ApiError> {
    let user = actor(&state, &headers).await?;
    let token = cookie_value(&headers, "__Host-bastion_session")
        .ok_or(ApiError(CoreError::Unauthorized))?;
    state
        .core
        .revoke_session(&token, id, user.id)
        .await
        .map_err(ApiError)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn revoke_other_sessions(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
    let user = actor(&state, &headers).await?;
    let token = cookie_value(&headers, "__Host-bastion_session")
        .ok_or(ApiError(CoreError::Unauthorized))?;
    let count = state
        .core
        .revoke_other_sessions(user.id, &token)
        .await
        .map_err(ApiError)?;
    Ok(Json(serde_json::json!({"revoked":count})))
}

async fn list_devices(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
) -> Result<Json<Vec<bastion_core::TrustedDeviceInfo>>, ApiError> {
    let user = actor(&state, &headers).await?;
    state
        .core
        .list_trusted_devices(user.id)
        .await
        .map(Json)
        .map_err(ApiError)
}

async fn revoke_device(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Path(id): Path<u64>,
) -> Result<StatusCode, ApiError> {
    let user = actor(&state, &headers).await?;
    state
        .core
        .revoke_trusted_device(user.id, id)
        .await
        .map_err(ApiError)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn list_security_history(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Query(query): Query<ListQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let user = actor(&state, &headers).await?;
    let (page, size) = page(&query);
    let items = state
        .core
        .security_history(user.id, page, size)
        .await
        .map_err(ApiError)?;
    Ok(Json(
        serde_json::json!({"items":items,"page":page.max(1),"page_size":size.clamp(1,100)}),
    ))
}

fn parse_datetime(value: String) -> Result<chrono::NaiveDateTime, ()> {
    chrono::DateTime::parse_from_rfc3339(&value)
        .map(|v| v.with_timezone(&chrono::Utc).naive_utc())
        .map_err(|_| ())
}

async fn authenticated_response(
    core: &CoreService,
    cookies: AuthCookies,
) -> Result<Response, ApiError> {
    let user = core
        .current_user(&cookies.session_token)
        .await
        .map_err(ApiError)?;
    let mut response = Json(
        serde_json::json!({"status":"authenticated","csrf_token":cookies.csrf_token,"user":user}),
    )
    .into_response();
    set_cookie(
        &mut response,
        "__Host-bastion_session",
        &cookies.session_token,
        "Path=/; Secure; HttpOnly; SameSite=Lax",
        if cookies.keep_logged_in {
            Some(315_360_000)
        } else {
            None
        },
    );
    set_cookie(
        &mut response,
        "__Host-bastion_csrf",
        &cookies.csrf_token,
        "Path=/; Secure; SameSite=Lax",
        if cookies.keep_logged_in {
            Some(315_360_000)
        } else {
            None
        },
    );
    set_cookie(
        &mut response,
        "__Host-bastion_device",
        &cookies.device_token,
        "Path=/; Secure; HttpOnly; SameSite=Lax",
        Some(1_296_000),
    );
    set_cookie(
        &mut response,
        "__Host-bastion_challenge",
        "",
        "Path=/; Secure; HttpOnly; SameSite=Lax",
        Some(0),
    );
    Ok(response)
}

fn set_cookie(
    response: &mut Response,
    name: &str,
    value: &str,
    attributes: &str,
    max_age: Option<i64>,
) {
    let age = max_age
        .map(|seconds| format!("; Max-Age={seconds}"))
        .unwrap_or_default();
    if let Ok(cookie) = HeaderValue::from_str(&format!("{name}={value}; {attributes}{age}")) {
        response
            .headers_mut()
            .append(axum::http::header::SET_COOKIE, cookie);
    }
}

fn cookie_value(headers: &axum::http::HeaderMap, name: &str) -> Option<String> {
    let cookies = headers.get(COOKIE)?.to_str().ok()?;
    let mut found = None;
    for part in cookies.split(';') {
        let (key, value) = part.trim().split_once('=')?;
        if key == name {
            if found.is_some() {
                return None;
            }
            found = Some(value.to_owned());
        }
    }
    found
}

fn random_token() -> String {
    let mut bytes = [0u8; 32];
    rand::RngCore::fill_bytes(&mut rand::rngs::OsRng, &mut bytes);
    base64::Engine::encode(&base64::engine::general_purpose::URL_SAFE_NO_PAD, bytes)
}

async fn csrf_guard(State(guard): State<CsrfGuardState>, request: Request, next: Next) -> Response {
    let Some(client_ip) = resolve_client_ip(&request, &guard.trusted_proxy_cidrs) else {
        return problem(
            StatusCode::BAD_REQUEST,
            "Invalid Client Address",
            "The client address could not be verified.",
        );
    };
    let mut request = request;
    request.extensions_mut().insert(ClientIp(client_ip));
    if matches!(
        *request.method(),
        Method::GET | Method::HEAD | Method::OPTIONS
    ) {
        return next.run(request).await;
    }
    let request_origin = request.headers().get(ORIGIN).and_then(|v| v.to_str().ok());
    if request_origin != Some(guard.origin.as_str()) {
        return problem(
            StatusCode::FORBIDDEN,
            "Origin Rejected",
            "The request origin is not allowed.",
        );
    }
    let cookie = request
        .headers()
        .get(COOKIE)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| parse_cookie(s, "__Host-bastion_csrf"));
    let header = request
        .headers()
        .get("x-csrf-token")
        .and_then(|v| v.to_str().ok());
    let valid = cookie
        .zip(header)
        .is_some_and(|(a, b)| a.as_bytes().ct_eq(b.as_bytes()).into());
    if !valid {
        return problem(
            StatusCode::FORBIDDEN,
            "CSRF Validation Failed",
            "The request could not be verified.",
        );
    }
    let session = request
        .headers()
        .get(COOKIE)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| parse_cookie(s, "__Host-bastion_session"));
    if let Some(session) =
        session.filter(|_| requires_session_csrf_binding(request.method(), request.uri().path()))
    {
        let token = header.unwrap_or_default();
        match guard.core.csrf_matches(session, token).await {
            Ok(true) => {}
            Ok(false) => {
                return problem(
                    StatusCode::FORBIDDEN,
                    "CSRF Validation Failed",
                    "The request could not be verified.",
                );
            }
            Err(_) => {
                return problem(
                    StatusCode::SERVICE_UNAVAILABLE,
                    "Temporarily Unavailable",
                    "The request could not be verified.",
                );
            }
        }
    }
    next.run(request).await
}

fn resolve_client_ip(request: &Request, proxies: &[IpNet]) -> Option<String> {
    let peer = request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|v| v.0.ip())?;
    let trusted = |ip: &IpAddr| proxies.iter().any(|network| network.contains(ip));
    if !trusted(&peer) {
        return Some(peer.to_string());
    }

    let mut values = request.headers().get_all("x-forwarded-for").iter();
    let Some(value) = values.next() else {
        return Some(peer.to_string());
    };
    // Require one bounded, unambiguous header from the trusted ingress. Reject
    // malformed forwarding rather than recording the proxy as a shared client.
    if values.next().is_some() || value.as_bytes().len() > 1024 {
        return None;
    }
    let chain = value
        .to_str()
        .ok()?
        .split(',')
        .take(17)
        .map(|part| part.trim().parse::<IpAddr>())
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    if chain.len() > 16 {
        return None;
    }
    // Only skip explicitly trusted hops, starting at the socket peer. A
    // client-supplied prefix to the left of the first untrusted hop is ignored.
    let mut client = peer;
    for ip in chain.into_iter().rev() {
        if !trusted(&client) {
            break;
        }
        client = ip;
    }
    Some(client.to_string())
}

fn parse_cookie<'a>(cookies: &'a str, name: &str) -> Option<&'a str> {
    let mut found = None;
    for item in cookies.split(';') {
        let (key, value) = item.trim().split_once('=')?;
        if key == name {
            if found.is_some() {
                return None;
            }
            found = Some(value);
        }
    }
    found
}

async fn problem_details(request: Request, next: Next) -> Response {
    let response = next.run(request).await;
    let status = response.status();
    let (title, detail) = match status {
        StatusCode::NOT_FOUND => ("Not Found", "The requested resource was not found."),
        StatusCode::PAYLOAD_TOO_LARGE => (
            "Payload Too Large",
            "The request body exceeds the configured limit.",
        ),
        StatusCode::METHOD_NOT_ALLOWED => ("Method Not Allowed", "The HTTP method is not allowed."),
        _ => return response,
    };
    problem(status, title, detail)
}

fn problem(status: StatusCode, title: &'static str, detail: &'static str) -> Response {
    let body = ProblemDetails {
        problem_type: "about:blank",
        title,
        status: status.as_u16(),
        detail,
    };
    let mut response = Json(body).into_response();
    *response.status_mut() = status;
    response.headers_mut().insert(
        "content-type",
        HeaderValue::from_static("application/problem+json"),
    );
    response
}

async fn strip_untrusted_request_id(mut request: Request, next: Next) -> Response {
    request
        .headers_mut()
        .remove(HeaderName::from_static("x-request-id"));
    next.run(request).await
}

struct ApiError(CoreError);
impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, title, detail) = match self.0 {
            CoreError::PreconditionFailed => (
                StatusCode::PRECONDITION_FAILED,
                "Precondition Failed",
                "El proceso cambió. Recarga sus datos antes de guardar.",
            ),
            CoreError::NotFound => (
                StatusCode::NOT_FOUND,
                "Not Found",
                "The resource is unavailable.",
            ),
            CoreError::Invalid => (
                StatusCode::BAD_REQUEST,
                "Invalid Request",
                "The request could not be processed.",
            ),
            CoreError::InvalidInvitationLink => (
                StatusCode::BAD_REQUEST,
                "Invalid Invitation Link",
                "El enlace de invitación está incompleto o alterado. Solicita reenviar la invitación y abre el enlace del correo más reciente.",
            ),
            CoreError::Unauthorized => (
                StatusCode::UNAUTHORIZED,
                "Authentication Failed",
                "Authentication could not be completed.",
            ),
            CoreError::Forbidden => (
                StatusCode::FORBIDDEN,
                "Forbidden",
                "The operation is not allowed.",
            ),
            CoreError::Conflict => (
                StatusCode::CONFLICT,
                "Conflict",
                "The request conflicts with current state.",
            ),
            CoreError::Locked => (
                StatusCode::TOO_MANY_REQUESTS,
                "Temporarily Unavailable",
                "Try again later.",
            ),
            CoreError::ReauthenticationRequired => (
                StatusCode::PRECONDITION_REQUIRED,
                "Additional Authentication Required",
                "Verify your authenticator to continue.",
            ),
            CoreError::SessionReplaced => (
                StatusCode::UNAUTHORIZED,
                "Session Replaced",
                "This account signed in from another session.",
            ),
            CoreError::Overloaded => (
                StatusCode::SERVICE_UNAVAILABLE,
                "Temporarily Unavailable",
                "Try again shortly.",
            ),
            CoreError::Internal(_) | CoreError::Migration(_) => {
                tracing::error!("core request failed");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal Server Error",
                    "The request failed. Use the request ID when contacting support.",
                )
            }
        };
        problem(status, title, detail)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn malformed_invitation_returns_actionable_error_without_secret_details() {
        let response = ApiError(CoreError::InvalidInvitationLink).into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = axum::body::to_bytes(response.into_body(), 4096)
            .await
            .unwrap();
        let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(body["title"], "Invalid Invitation Link");
        assert!(
            body["detail"]
                .as_str()
                .unwrap()
                .contains("reenviar la invitación")
        );
        assert!(body.get("token").is_none());
        assert!(body.get("password").is_none());
    }

    #[tokio::test]
    async fn csrf_refresh_preserves_session_token_across_reloads_and_tabs() {
        let token = random_token();
        for session in ["", "; __Host-bastion_session=stale-or-revoked-session"] {
            let mut headers = axum::http::HeaderMap::new();
            headers.insert(
                COOKIE,
                format!("__Host-bastion_csrf={token}{session}")
                    .parse()
                    .unwrap(),
            );
            for _ in 0..2 {
                let response = issue_csrf(headers.clone()).await;
                // In particular, do not remove Max-Age from persistent cookies.
                assert!(
                    !response
                        .headers()
                        .contains_key(axum::http::header::SET_COOKIE)
                );
                let body = axum::body::to_bytes(response.into_body(), 1024)
                    .await
                    .unwrap();
                let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
                assert_eq!(payload["csrf_token"], token);
            }
        }
    }

    #[tokio::test]
    async fn csrf_initialization_replaces_missing_invalid_or_duplicate_cookie() {
        for cookie in [
            "",
            "__Host-bastion_csrf=invalid",
            "__Host-bastion_csrf=a; __Host-bastion_csrf=b",
        ] {
            let mut headers = axum::http::HeaderMap::new();
            headers.insert(COOKIE, cookie.parse().unwrap());
            let response = issue_csrf(headers).await;
            let set_cookie = response.headers()[axum::http::header::SET_COOKIE]
                .to_str()
                .unwrap()
                .to_owned();
            assert!(set_cookie.contains("; Path=/; Secure; SameSite=Lax"));
            assert!(!set_cookie.contains("Domain="));
            let body = axum::body::to_bytes(response.into_body(), 1024)
                .await
                .unwrap();
            let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
            let token = payload["csrf_token"].as_str().unwrap();
            let mut issued = axum::http::HeaderMap::new();
            issued.insert(
                COOKIE,
                format!("__Host-bastion_csrf={token}").parse().unwrap(),
            );
            assert_eq!(reusable_csrf_token(&issued).as_deref(), Some(token));
            assert!(set_cookie.starts_with(&format!("__Host-bastion_csrf={token};")));
        }
    }

    #[test]
    fn stale_sessions_do_not_bind_preauth_flows_but_protected_changes_stay_bound() {
        for path in [
            "/api/v1/auth/login",
            "/api/v1/auth/totp",
            "/api/v1/auth/recovery-code",
            "/api/v1/auth/activate",
            "/api/v1/auth/activate/complete",
            "/api/v1/auth/password-reset",
            "/api/v1/auth/password-reset/complete",
            "/api/v1/bootstrap",
            "/api/v1/bootstrap/complete",
        ] {
            assert!(!requires_session_csrf_binding(&Method::POST, path));
            assert!(requires_session_csrf_binding(&Method::PUT, path));
        }
        for path in [
            "/api/v1/auth/logout",
            "/api/v1/auth/reauth-totp",
            "/api/v1/account/password",
            "/api/v1/account/sessions/revoke-others",
            "/api/v1/account/profile",
            "/api/v1/settings/core/smtp",
            "/api/v1/auth/login/extra",
        ] {
            assert!(requires_session_csrf_binding(&Method::POST, path));
        }
    }

    fn client_request(peer: &str, forwarded: Option<&str>) -> Request {
        let mut request = Request::new(axum::body::Body::empty());
        request
            .extensions_mut()
            .insert(ConnectInfo(SocketAddr::new(peer.parse().unwrap(), 12345)));
        if let Some(value) = forwarded {
            request
                .headers_mut()
                .insert("x-forwarded-for", value.parse().unwrap());
        }
        request
    }

    fn ingress_proxies() -> Vec<IpNet> {
        vec!["127.0.0.1/32".parse().unwrap(), "::1/128".parse().unwrap()]
    }

    #[test]
    fn direct_clients_cannot_spoof_forwarding_headers() {
        let mut request = client_request("198.51.100.25", Some("203.0.113.99"));
        request
            .headers_mut()
            .insert("cf-connecting-ip", "203.0.113.98".parse().unwrap());
        assert_eq!(
            resolve_client_ip(&request, &ingress_proxies()).as_deref(),
            Some("198.51.100.25")
        );
        let request = client_request("127.0.0.1", Some("203.0.113.99"));
        assert_eq!(
            resolve_client_ip(&request, &[]).as_deref(),
            Some("127.0.0.1")
        );
    }

    #[test]
    fn trusted_ingress_resolves_ipv4_and_ipv6_and_ignores_cf_header() {
        for (peer, client) in [("127.0.0.1", "198.51.100.25"), ("::1", "2001:db8::25")] {
            let mut request = client_request(peer, Some(client));
            request
                .headers_mut()
                .insert("cf-connecting-ip", "203.0.113.98".parse().unwrap());
            assert_eq!(
                resolve_client_ip(&request, &ingress_proxies()).as_deref(),
                Some(client)
            );
        }
    }

    #[test]
    fn forwarding_walk_stops_at_first_untrusted_hop() {
        let request = client_request("127.0.0.1", Some("203.0.113.99, 198.51.100.25"));
        assert_eq!(
            resolve_client_ip(&request, &ingress_proxies()).as_deref(),
            Some("198.51.100.25")
        );
        let mut proxies = ingress_proxies();
        proxies.push("192.0.2.10/32".parse().unwrap());
        let request = client_request("127.0.0.1", Some("203.0.113.99, 198.51.100.25, 192.0.2.10"));
        assert_eq!(
            resolve_client_ip(&request, &proxies).as_deref(),
            Some("198.51.100.25")
        );
    }

    #[test]
    fn malformed_ambiguous_or_excessive_forwarding_is_rejected() {
        let long_chain = std::iter::repeat_n("198.51.100.25", 17)
            .collect::<Vec<_>>()
            .join(",");
        for value in [
            "",
            "unknown",
            "198.51.100.25,",
            "198.51.100.25:443",
            &long_chain,
            &" ".repeat(1025),
        ] {
            assert!(
                resolve_client_ip(
                    &client_request("127.0.0.1", Some(value)),
                    &ingress_proxies()
                )
                .is_none()
            );
        }
        let mut request = client_request("127.0.0.1", Some("198.51.100.25"));
        request
            .headers_mut()
            .append("x-forwarded-for", "203.0.113.99".parse().unwrap());
        assert!(resolve_client_ip(&request, &ingress_proxies()).is_none());
    }

    #[test]
    fn missing_socket_peer_never_invents_localhost() {
        let request = Request::new(axum::body::Body::empty());
        assert!(resolve_client_ip(&request, &ingress_proxies()).is_none());
        assert_eq!(
            resolve_client_ip(&client_request("127.0.0.1", None), &ingress_proxies()).as_deref(),
            Some("127.0.0.1")
        );
    }

    #[test]
    fn origin_validation_rejects_wildcards_paths_and_http() {
        assert!(validate_origin("https://app.castilloit.net").is_ok());
        assert!(validate_origin("*").is_err());
        assert!(validate_origin("https://app.castilloit.net/path").is_err());
        assert!(validate_origin("http://app.castilloit.net").is_err());
    }

    #[test]
    fn cookie_parser_rejects_ambiguous_duplicate() {
        assert_eq!(
            parse_cookie("a=1; __Host-bastion_csrf=token", "__Host-bastion_csrf"),
            Some("token")
        );
        assert_eq!(
            parse_cookie(
                "__Host-bastion_csrf=a; __Host-bastion_csrf=b",
                "__Host-bastion_csrf"
            ),
            None
        );
    }
}
