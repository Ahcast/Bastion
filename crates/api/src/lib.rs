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

mod procesos;

const GENERAL_REQUEST_BODY_LIMIT: usize = 1024 * 1024;

#[derive(Clone)]
struct ApiState {
    core: CoreService,
    procesos: bastion_procesos::ProcessService,
    procesos_jobs: std::sync::Arc<tokio::sync::Semaphore>,
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
        ])
        .allow_credentials(true)
        .max_age(Duration::from_secs(600));
    let api_state = ApiState {
        core: core.clone(),
        procesos: bastion_procesos::ProcessService::new(core.database(), core.admin()),
        procesos_jobs: std::sync::Arc::new(tokio::sync::Semaphore::new(1)),
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
) -> Result<(StatusCode, Json<s…40027 tokens truncated…&str) -> Result<Vec<u8>, CoreError> {
        let nonce = random_bytes::<12>();
        let ciphertext = self
            .cipher
            .encrypt(Nonce::from_slice(&nonce), value.as_bytes())
            .map_err(|_| CoreError::Invalid)?;
        let mut packed = nonce.to_vec();
        packed.extend(ciphertext);
        Ok(packed)
    }

    fn decrypt(&self, packed: &[u8]) -> Result<String, CoreError> {
        if packed.len() < 12 {
            return Err(CoreError::Invalid);
        }
        let (nonce, ciphertext) = packed.split_at(12);
        let plain = self
            .cipher
            .decrypt(Nonce::from_slice(nonce), ciphertext)
            .map_err(|_| CoreError::Invalid)?;
        String::from_utf8(plain).map_err(|_| CoreError::Invalid)
    }

    async fn log_security(
        &self,
        user_id: Option<u64>,
        kind: &str,
        result: &str,
        ip: &str,
        agent: String,
    ) -> Result<(), CoreError> {
        sqlx::query("INSERT INTO security_events (user_id,event_type,result,ip_address,device_label,created_at) VALUES (?,?,?,?,?,?)")
            .bind(user_id).bind(kind).bind(result).bind(ip).bind(agent).bind(Utc::now().naive_utc()).execute(&self.database.pool).await?;
        Ok(())
    }

    async fn audit(
        &self,
        actor: Option<u64>,
        module: &str,
        action: &str,
        target_type: &str,
        target_id: String,
        result: &str,
    ) -> Result<(), CoreError> {
        sqlx::query("INSERT INTO audit_events (actor_id,module_id,action,target_type,target_id,result,created_at) VALUES (?,?,?,?,?,?,?)")
            .bind(actor).bind(module).bind(action).bind(target_type).bind(target_id).bind(result).bind(Utc::now().naive_utc()).execute(&self.database.pool).await?;
        Ok(())
    }
}

async fn revoke_user_access(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    now: NaiveDateTime,
    reason: &str,
) -> Result<(), CoreError> {
    sqlx::query("UPDATE sessions SET revoked_at=?,revocation_reason=? WHERE user_id=? AND revoked_at IS NULL").bind(now).bind(reason).bind(user_id).execute(&mut **tx).await?;
    sqlx::query("UPDATE trusted_devices SET revoked_at=? WHERE user_id=? AND revoked_at IS NULL")
        .bind(now)
        .bind(user_id)
        .execute(&mut **tx)
        .await?;
    sqlx::query("UPDATE otp_challenges SET consumed_at=? WHERE user_id=? AND consumed_at IS NULL")
        .bind(now)
        .bind(user_id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

async fn insert_audit(
    tx: &mut Transaction<'_, MySql>,
    actor: Option<u64>,
    module: &str,
    action: &str,
    target_type: &str,
    target_id: Option<String>,
    result: &str,
) -> Result<(), CoreError> {
    sqlx::query("INSERT INTO audit_events (actor_id,module_id,action,target_type,target_id,result,created_at) VALUES (?,?,?,?,?,?,?)")
        .bind(actor).bind(module).bind(action).bind(target_type).bind(target_id).bind(result).bind(Utc::now().naive_utc()).execute(&mut **tx).await?;
    Ok(())
}

fn random_bytes<const N: usize>() -> [u8; N] {
    let mut bytes = [0u8; N];
    OsRng.fill_bytes(&mut bytes);
    bytes
}
fn random_token() -> String {
    base64::Engine::encode(
        &base64::engine::general_purpose::URL_SAFE_NO_PAD,
        random_bytes::<32>(),
    )
}
fn token_hash(value: &str) -> [u8; 32] {
    Sha256::digest(value.as_bytes()).into()
}
fn constant_time_text_eq(left: &str, right: &str) -> bool {
    left.as_bytes().ct_eq(right.as_bytes()).into()
}
fn hash_password(password: &str) -> Result<String, CoreError> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|_| CoreError::Invalid)
}
fn verify_password(password: &str, encoded: &str) -> bool {
    PasswordHash::new(encoded).ok().is_some_and(|parsed| {
        Argon2::default()
            .verify_password(password.as_bytes(), &parsed)
            .is_ok()
    })
}
fn map_unique_conflict(error: sqlx::Error) -> CoreError {
    if let sqlx::Error::Database(db) = &error {
        if db.code().as_deref() == Some("23000") {
            return CoreError::Conflict;
        }
    }
    CoreError::Internal(error)
}
fn validate_password(password: &str) -> Result<(), CoreError> {
    if password.chars().count() < 8 || password.len() > 1024 {
        return Err(CoreError::Invalid);
    }
    Ok(())
}
fn validate_password_input(password: &str) -> Result<(), CoreError> {
    if password.len() > 1024 {
        Err(CoreError::Invalid)
    } else {
        Ok(())
    }
}
fn normalize_email(email: &str) -> Result<String, CoreError> {
    let normalized = email.trim().to_lowercase();
    if normalized.is_empty()
        || normalized.len() > 254
        || !normalized.is_ascii()
        || !normalized.contains('@')
        || normalized.chars().any(char::is_control)
    {
        return Err(CoreError::Invalid);
    }
    Ok(normalized)
}
fn safe_agent(agent: &str) -> String {
    agent
        .chars()
        .filter(|c| !c.is_control())
        .take(180)
        .collect()
}
async fn smtp_host_resolves_publicly(host: &str, port: u16) -> bool {
    if host.is_empty()
        || host.len() > 253
        || host.parse::<IpAddr>().is_ok()
        || host
            .chars()
            .any(|c| c.is_control() || c.is_whitespace() || matches!(c, '/' | '\\' | '@' | ':'))
    {
        return false;
    }
    let resolved = tokio::time::timeout(
        Duration::from_secs(5),
        tokio::net::lookup_host((host, port)),
    )
    .await;
    let Ok(Ok(addresses)) = resolved else {
        return false;
    };
    let addresses = addresses.collect::<Vec<_>>();
    !addresses.is_empty() && addresses.iter().all(|address| is_public_ip(address.ip()))
}
fn is_public_ip(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(ip) => is_public_ipv4(ip),
        IpAddr::V6(ip) => {
            if let Some(mapped) = ip.to_ipv4_mapped() {
                return is_public_ipv4(mapped);
            }
            let octets = ip.octets();
            let global_unicast = octets[0] & 0xe0 == 0x20;
            let special_2001 = octets[0] == 0x20 && octets[1] == 0x01 && octets[2] < 0x02;
            let six_to_four = octets[0] == 0x20 && octets[1] == 0x02;
            let documentation = octets[0] == 0x3f && octets[1] == 0xff;
            global_unicast && !special_2001 && !six_to_four && !documentation
        }
    }
}
fn is_public_ipv4(ip: Ipv4Addr) -> bool {
    let [a, b, c, _] = ip.octets();
    !(a == 0
        || a == 10
        || a == 127
        || (a == 169 && b == 254)
        || (a == 172 && (16..=31).contains(&b))
        || (a == 192 && b == 168)
        || (a == 100 && (64..=127).contains(&b))
        || (a == 192 && b == 0)
        || (a == 192 && b == 0 && c == 2)
        || (a == 198 && (b == 18 || b == 19))
        || (a == 198 && b == 51 && c == 100)
        || (a == 203 && b == 0 && c == 113)
        || a >= 224)
}
fn recovery_code() -> String {
    hex::encode(random_bytes::<8>()).to_uppercase()
}
fn verify_totp(secret: &str, code: &str, last_counter: i64) -> Option<u64> {
    if code.len() != 6 || !code.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let secret = BASE32_NOPAD.decode(secret.as_bytes()).ok()?;
    let current = (Utc::now().timestamp().max(0) as u64) / 30;
    for counter in [
        current.saturating_sub(1),
        current,
        current.saturating_add(1),
    ] {
        if counter as i64 <= last_counter {
            continue;
        }
        let mut mac = <HmacSha1 as Mac>::new_from_slice(&secret).ok()?;
        mac.update(&counter.to_be_bytes());
        let digest = mac.finalize().into_bytes();
        let offset = (digest[19] & 0x0f) as usize;
        let binary = (u32::from(digest[offset] & 0x7f) << 24)
            | (u32::from(digest[offset + 1]) << 16)
            | (u32::from(digest[offset + 2]) << 8)
            | u32::from(digest[offset + 3]);
        let expected = format!("{:06}", binary % 1_000_000);
        if bool::from(expected.as_bytes().ct_eq(code.as_bytes())) {
            return Some(counter);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_passwords_below_eight_characters_and_accepts_passphrases() {
        assert!(validate_password("short7").is_err());
        assert!(validate_password("a long passphrase with unicode ñ").is_ok());
        assert!(validate_password(&"x".repeat(1025)).is_err());
    }

    #[test]
    fn hashes_are_one_way_and_verification_uses_argon2() {
        let hash = hash_password("correct horse battery staple").expect("hash password");
        assert!(verify_password("correct horse battery staple", &hash));
        assert!(!verify_password("wrong password", &hash));
        assert!(!hash.contains("correct horse"));
    }

    #[test]
    fn recovery_and_session_tokens_are_unpredictable_and_hashed() {
        let a = random_token();
        let b = random_token();
        assert_ne!(a, b);
        assert_eq!(a.len(), 43);
        assert_ne!(token_hash(&a), token_hash(&b));
        assert_eq!(recovery_code().len(), 16);
    }

    #[test]
    fn totp_is_single_use_by_counter() {
        let secret = BASE32_NOPAD.encode(b"12345678901234567890");
        let counter = (Utc::now().timestamp().max(0) as u64 / 30) + 1;
        let code = totp_for_counter(&secret, counter);
        assert_eq!(verify_totp(&secret, &code, -1), Some(counter));
        assert_eq!(verify_totp(&secret, &code, counter as i64), None);
    }

    #[test]
    fn smtp_ip_filter_rejects_local_and_documentation_ranges() {
        assert!(!is_public_ip("127.0.0.1".parse().unwrap()));
        assert!(!is_public_ip("10.1.2.3".parse().unwrap()));
        assert!(!is_public_ip("192.0.2.10".parse().unwrap()));
        assert!(!is_public_ip("::1".parse().unwrap()));
        assert!(is_public_ip("8.8.8.8".parse().unwrap()));
    }
}

#[cfg(test)]
fn totp_for_counter(secret: &str, counter: u64) -> String {
    let secret = BASE32_NOPAD
        .decode(secret.as_bytes())
        .expect("base32 secret");
    let mut mac = <HmacSha1 as Mac>::new_from_slice(&secret).expect("hmac key");
    mac.update(&counter.to_be_bytes());
    let digest = mac.finalize().into_bytes();
    let offset = (digest[19] & 0x0f) as usize;
    let binary = (u32::from(digest[offset] & 0x7f) << 24)
        | (u32::from(digest[offset + 1]) << 16)
        | (u32::from(digest[offset + 2]) << 8)
        | u32::from(digest[offset + 3]);
    format!("{:06}", binary % 1_000_000)
}
