use super::*;
use bastion_cotizaciones::{
    QuoteDraft, QuoteEmailInput, QuoteFilter, QuoteInput, QuoteMailService, QuotePage, QuoteStatus,
};

pub(super) fn routes(state: ApiState) -> Router<ApiState> {
    let attachments = Router::new()
        .route(
            "/api/v1/cotizaciones/{id}/attachments",
            get(attachments).post(upload_attachment),
        )
        .route(
            "/api/v1/cotizaciones/{id}/attachments/{attachment}",
            axum::routing::delete(remove_attachment),
        )
        .route(
            "/api/v1/cotizaciones/{id}/attachments/{attachment}/content",
            get(attachment_content),
        )
        .layer(axum::extract::DefaultBodyLimit::max(28 * 1024 * 1024))
        .layer(RequestBodyLimitLayer::new(28 * 1024 * 1024));
    Router::new()
        .route("/api/v1/cotizaciones/dashboard", get(dashboard))
        .route("/api/v1/cotizaciones", get(list).post(create))
        .route("/api/v1/cotizaciones/calculate", post(preview))
        .route(
            "/api/v1/cotizaciones/document-readiness",
            get(document_readiness),
        )
        .route("/api/v1/cotizaciones/{id}/pdf", get(pdf))
        .route("/api/v1/cotizaciones/{id}/emails", post(queue_email))
        .route("/api/v1/cotizaciones/{id}/emails/{job}", get(email_job))
        .route(
            "/api/v1/cotizaciones/{id}",
            get(detail).put(replace).delete(remove),
        )
        .route(
            "/api/v1/cotizaciones/{id}/status",
            axum::routing::patch(change_status),
        )
        .route("/api/v1/cotizaciones/{id}/clone", post(clone_quote))
        .layer(RequestBodyLimitLayer::new(GENERAL_REQUEST_BODY_LIMIT))
        .merge(attachments)
        .route_layer(middleware::from_fn_with_state(state, guard))
}

async fn preview(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Json(input): Json<QuoteInput>,
) -> Result<Json<bastion_cotizaciones::QuoteCalculation>, ApiError> {
    let user = actor(&state, &headers).await?;
    state
        .cotizaciones
        .preview(user.id, &input)
        .await
        .map(Json)
        .map_err(ApiError)
}

static ATTACHMENT_REQUESTS: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(4);

async fn guard(State(state): State<ApiState>, request: Request, next: Next) -> Response {
    let attachment_operation = request.uri().path().contains("/attachments");
    let document_operation = request.uri().path().ends_with("/pdf")
        || request.uri().path().ends_with("/emails")
        || attachment_operation;
    let result = tokio::time::timeout(
        Duration::from_secs(if document_operation { 60 } else { 30 }),
        async {
            let _permit = if attachment_operation {
                Some(
                    ATTACHMENT_REQUESTS
                        .try_acquire()
                        .map_err(|_| ApiError(CoreError::Overloaded))?,
                )
            } else {
                None
            };
            let user = actor(&state, request.headers()).await?;
            let capability = if request.method() == Method::DELETE && !attachment_operation {
                "eliminar"
            } else if request.method() == Method::GET || request.method() == Method::HEAD {
                "ver"
            } else {
                "editar"
            };
            state
                .cotizaciones
                .check_access(user.id, capability)
                .await
                .map_err(ApiError)?;
            state
                .cotizaciones
                .consume_request(user.id)
                .await
                .map_err(ApiError)?;
            Ok::<_, ApiError>(next.run(request).await)
        },
    )
    .await;
    match result {
        Ok(Ok(response)) => response,
        Ok(Err(error)) => error.into_response(),
        Err(_) => ApiError(CoreError::Overloaded).into_response(),
    }
}

async fn list(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Query(filter): Query<QuoteFilter>,
) -> Result<Json<QuotePage>, ApiError> {
    let user = actor(&state, &headers).await?;
    state
        .cotizaciones
        .list(user.id, &filter)
        .await
        .map(Json)
        .map_err(ApiError)
}

async fn dashboard(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Query(query): Query<bastion_core::DashboardQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let user = actor(&state, &headers).await?;
    state
        .cotizaciones
        .dashboard(user.id, query.period)
        .await
        .map(Json)
        .map_err(ApiError)
}

async fn detail(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Path(id): Path<u64>,
) -> Result<Response, ApiError> {
    let user = actor(&state, &headers).await?;
    let draft = state
        .cotizaciones
        .detail(user.id, id)
        .await
        .map_err(ApiError)?;
    Ok(response(draft, false))
}

async fn create(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Json(input): Json<QuoteInput>,
) -> Result<Response, ApiError> {
    let user = actor(&state, &headers).await?;
    let key = single_header(&headers, "idempotency-key")?;
    let draft = state
        .cotizaciones
        .create(user.id, key, input)
        .await
        .map_err(ApiError)?;
    Ok(response(draft, true))
}

async fn replace(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Path(id): Path<u64>,
    Json(input): Json<QuoteInput>,
) -> Result<Response, ApiError> {
    let user = actor(&state, &headers).await?;
    let version =
        version(single_header(&headers, "if-match")?).ok_or(ApiError(CoreError::Invalid))?;
    match state
        .cotizaciones
        .replace(user.id, id, version, input)
        .await
    {
        Ok(draft) => Ok(response(draft, false)),
        Err(CoreError::PreconditionFailed) => Ok(problem(
            StatusCode::PRECONDITION_FAILED,
            "Precondition Failed",
            "El borrador cambió. Vuelve a abrirlo antes de guardar.",
        )),
        Err(error) => Err(ApiError(error)),
    }
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct StatusInput {
    status: QuoteStatus,
}

fn expected_version(headers: &axum::http::HeaderMap) -> Result<u64, ApiError> {
    version(single_header(headers, "if-match")?).ok_or(ApiError(CoreError::Invalid))
}

async fn document_readiness(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
) -> Result<Json<bastion_cotizaciones::QuoteDocumentReadiness>, ApiError> {
    let user = actor(&state, &headers).await?;
    state
        .cotizaciones
        .document_readiness(user.id, &state.core)
        .await
        .map(Json)
        .map_err(ApiError)
}
async fn pdf(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Path(id): Path<u64>,
) -> Result<Json<bastion_cotizaciones::QuoteDocument>, ApiError> {
    let user = actor(&state, &headers).await?;
    state
        .cotizaciones
        .pdf(user.id, id, expected_version(&headers)?)
        .await
        .map(Json)
        .map_err(ApiError)
}
async fn queue_email(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Path(id): Path<u64>,
    Json(input): Json<QuoteEmailInput>,
) -> Result<(StatusCode, Json<bastion_cotizaciones::QuoteEmailJob>), ApiError> {
    let user = actor(&state, &headers).await?;
    QuoteMailService::new(state.core)
        .queue(
            user.id,
            id,
            expected_version(&headers)?,
            single_header(&headers, "idempotency-key")?,
            input,
        )
        .await
        .map(|job| (StatusCode::ACCEPTED, Json(job)))
        .map_err(ApiError)
}
async fn email_job(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Path((id, job)): Path<(u64, u64)>,
) -> Result<Json<bastion_cotizaciones::QuoteEmailJob>, ApiError> {
    let user = actor(&state, &headers).await?;
    QuoteMailService::new(state.core)
        .job(user.id, id, job)
        .await
        .map(Json)
        .map_err(ApiError)
}

async fn change_status(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Path(id): Path<u64>,
    Json(input): Json<StatusInput>,
) -> Result<Response, ApiError> {
    let user = actor(&state, &headers).await?;
    state
        .cotizaciones
        .change_status(user.id, id, expected_version(&headers)?, input.status)
        .await
        .map(|draft| response(draft, false))
        .map_err(ApiError)
}

async fn clone_quote(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Path(id): Path<u64>,
) -> Result<Response, ApiError> {
    let user = actor(&state, &headers).await?;
    state
        .cotizaciones
        .clone_quote(
            user.id,
            id,
            expected_version(&headers)?,
            single_header(&headers, "idempotency-key")?,
        )
        .await
        .map(|draft| response(draft, true))
        .map_err(ApiError)
}

async fn remove(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Path(id): Path<u64>,
) -> Result<StatusCode, ApiError> {
    let user = actor(&state, &headers).await?;
    state
        .cotizaciones
        .delete(user.id, id, expected_version(&headers)?)
        .await
        .map_err(ApiError)?;
    Ok(StatusCode::NO_CONTENT)
}

fn single_header<'a>(headers: &'a axum::http::HeaderMap, name: &str) -> Result<&'a str, ApiError> {
    if headers.get_all(name).iter().count() != 1 {
        return Err(ApiError(CoreError::Invalid));
    }
    headers
        .get(name)
        .and_then(|v| v.to_str().ok())
        .ok_or(ApiError(CoreError::Invalid))
}

fn version(value: &str) -> Option<u64> {
    let digits = value.strip_prefix('"')?.strip_suffix('"')?;
    if digits.starts_with('0') || !digits.bytes().all(|c| c.is_ascii_digit()) {
        return None;
    }
    digits.parse::<u64>().ok().filter(|v| *v > 0)
}

fn response(draft: QuoteDraft, created: bool) -> Response {
    let version = draft.version;
    let id = draft.id;
    let mut response = Json(draft).into_response();
    if let Ok(value) = HeaderValue::from_str(&format!("\"{version}\"")) {
        response.headers_mut().insert("etag", value);
    }
    if created {
        *response.status_mut() = StatusCode::CREATED;
        if let Ok(value) = HeaderValue::from_str(&format!("/api/v1/cotizaciones/{id}")) {
            response.headers_mut().insert("location", value);
        }
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_one_strong_positive_version_is_accepted() {
        assert_eq!(version("\"12\""), Some(12));
        for input in [
            "12",
            "*",
            "W/\"12\"",
            "\"0\"",
            "\"01\"",
            "\"-1\"",
            "\"1\",\"2\"",
            "\"\"",
        ] {
            assert!(version(input).is_none(), "{input}");
        }
        let mut headers = axum::http::HeaderMap::new();
        headers.append("if-match", HeaderValue::from_static("\"1\""));
        headers.append("if-match", HeaderValue::from_static("\"2\""));
        assert!(single_header(&headers, "if-match").is_err());
    }
    #[tokio::test]
    async fn mysql57_attachment_http_body_limits_csrf_and_versions() {
        use axum::body::{Body, to_bytes};
        use base64::{Engine, engine::general_purpose::STANDARD};
        use bastion_core::Database;
        use sqlx::mysql::MySqlPoolOptions;
        use tower::ServiceExt;
        let Ok(url) = std::env::var("BASTION_COTIZACIONES_ATTACHMENT_HTTP_TEST_URL") else {
            return;
        };
        assert_eq!(
            std::env::var("BASTION_DOCUMENTS_BUCKET").unwrap(),
            "bastion-synthetic-attachment-test"
        );
        let pool = MySqlPoolOptions::new()
            .max_connections(5)
            .connect(&url)
            .await
            .unwrap();
        let database = Database::from_test_pool(pool.clone());
        database.migrate().await.unwrap();
        bastion_cotizaciones::QuoteService::migrate(&database)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO persons (id,full_name) VALUES (1,'Synthetic attachment administrator')",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query("INSERT INTO users (id,person_id,role_id,email,status) VALUES (1,1,1,'admin@example.test','active')").execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO sessions (user_id,token_hash,csrf_hash,ip_address,started_at,last_activity_at,last_totp_at,expires_at) VALUES (1,UNHEX(SHA2('synthetic-attachment-session',256)),UNHEX(SHA2('synthetic-csrf',256)),'127.0.0.1',UTC_TIMESTAMP(6),UTC_TIMESTAMP(6),UTC_TIMESTAMP(6),UTC_TIMESTAMP(6)+INTERVAL 1 HOUR)").execute(&pool).await.unwrap();
        let core = CoreService::new(database.clone(), "x".repeat(32), [1; 32]).unwrap();
        let quotes = bastion_cotizaciones::QuoteService::new(database, core.admin());
        quotes.synchronize_release(true).await.unwrap();
        core.admin()
            .set_module_enabled(1, "cotizaciones", true)
            .await
            .unwrap();
        let input=serde_json::from_value(serde_json::json!({"client_name":"Cliente sintético","order_number":"","quote_date":"2026-10-23","valid_days":30,"notes":"","terms":"","discount_type":"amount","discount_value":"0","items":[{"description":"Producto","quantity":"1","unit_price":"100","discount_type":"amount","discount_value":"0","tax_type":"isv15"}]})).unwrap();
        let quote = quotes
            .create(1, "attachment-http-create-01", input)
            .await
            .unwrap();
        let app = super::super::router(
            core,
            "https://example.test".into(),
            Vec::new(),
            "/unused".into(),
        )
        .unwrap();
        let path = format!("/api/v1/cotizaciones/{}/attachments", quote.id);
        let request = |method: &str,
                       path: &str,
                       authenticated: bool,
                       csrf: bool,
                       origin: &str,
                       version: Option<&str>,
                       body: String| {
            let mut builder = axum::http::Request::builder()
                .method(method)
                .uri(path)
                .header("origin", origin)
                .header("content-type", "application/json");
            if authenticated {
                builder=builder.header("cookie","__Host-bastion_session=synthetic-attachment-session; __Host-bastion_csrf=synthetic-csrf");
            }
            if csrf {
                builder = builder.header("x-csrf-token", "synthetic-csrf");
            }
            if let Some(version) = version {
                builder = builder.header("if-match", version);
            }
            let mut request = builder
                .header("idempotency-key", "attachment-http-upload-01")
                .body(Body::from(body))
                .unwrap();
            request
                .extensions_mut()
                .insert(ConnectInfo("127.0.0.1:5010".parse::<SocketAddr>().unwrap()));
            request
        };
        assert_eq!(
            app.clone()
                .oneshot(request(
                    "GET",
                    &path,
                    false,
                    false,
                    "https://example.test",
                    None,
                    String::new()
                ))
                .await
                .unwrap()
                .status(),
            StatusCode::UNAUTHORIZED
        );
        for (csrf, origin) in [
            (false, "https://example.test"),
            (true, "https://evil.example"),
        ] {
            assert_eq!(
                app.clone()
                    .oneshot(request(
                        "POST",
                        &path,
                        true,
                        csrf,
                        origin,
                        Some("\"1\""),
                        "{}".into()
                    ))
                    .await
                    .unwrap()
                    .status(),
                StatusCode::FORBIDDEN
            );
        }
        // Exceeds Axum's default JSON limit; the dedicated attachment limit must apply.
        let bytes = vec![b'x'; 2 * 1024 * 1024 + 64];
        let payload =
            serde_json::json!({"filename":"large.bin","content_base64":STANDARD.encode(&bytes)})
                .to_string();
        assert_eq!(
            app.clone()
                .oneshot(request(
                    "POST",
                    &path,
                    true,
                    true,
                    "https://example.test",
                    None,
                    payload.clone()
                ))
                .await
                .unwrap()
                .status(),
            StatusCode::BAD_REQUEST
        );
        let response = app
            .clone()
            .oneshot(request(
                "POST",
                &path,
                true,
                true,
                "https://example.test",
                Some("\"1\""),
                payload,
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
        let result: serde_json::Value =
            serde_json::from_slice(&to_bytes(response.into_body(), 16384).await.unwrap()).unwrap();
        assert_eq!(result["quote_version"], 2);
        let id = result["attachment"]["id"].as_str().unwrap();
        let content = app
            .clone()
            .oneshot(request(
                "GET",
                &format!("{path}/{id}/content"),
                true,
                false,
                "https://example.test",
                None,
                String::new(),
            ))
            .await
            .unwrap();
        assert_eq!(content.status(), StatusCode::OK);
        let content: serde_json::Value = serde_json::from_slice(
            &to_bytes(content.into_body(), 4 * 1024 * 1024)
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(
            STANDARD
                .decode(content["content_base64"].as_str().unwrap())
                .unwrap(),
            bytes
        );
        assert!(content.get("object_key").is_none());
        assert_eq!(
            app.clone()
                .oneshot(request(
                    "DELETE",
                    &format!("{path}/{id}"),
                    true,
                    true,
                    "https://example.test",
                    Some("\"1\""),
                    String::new()
                ))
                .await
                .unwrap()
                .status(),
            StatusCode::PRECONDITION_FAILED
        );
        assert_eq!(
            app.clone()
                .oneshot(request(
                    "DELETE",
                    &format!("{path}/{id}"),
                    true,
                    true,
                    "https://example.test",
                    Some("\"2\""),
                    String::new()
                ))
                .await
                .unwrap()
                .status(),
            StatusCode::OK
        );
        assert_eq!(
            app.clone()
                .oneshot(request(
                    "POST",
                    &path,
                    true,
                    true,
                    "https://example.test",
                    Some("\"3\""),
                    "x".repeat(28 * 1024 * 1024 + 1)
                ))
                .await
                .unwrap()
                .status(),
            StatusCode::PAYLOAD_TOO_LARGE
        );
        assert_eq!(
            app.oneshot(request(
                "POST",
                "/api/v1/cotizaciones/calculate",
                true,
                true,
                "https://example.test",
                None,
                "x".repeat(2 * 1024 * 1024)
            ))
            .await
            .unwrap()
            .status(),
            StatusCode::PAYLOAD_TOO_LARGE
        );
    }
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct AttachmentQuery {
    page: Option<u32>,
}
async fn attachments(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Path(id): Path<u64>,
    Query(query): Query<AttachmentQuery>,
) -> Result<Json<bastion_cotizaciones::QuoteAttachmentPage>, ApiError> {
    let user = actor(&state, &headers).await?;
    state
        .cotizaciones
        .attachments(user.id, id, query.page.unwrap_or(1))
        .await
        .map(Json)
        .map_err(ApiError)
}
async fn upload_attachment(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Path(id): Path<u64>,
    Json(input): Json<bastion_cotizaciones::QuoteAttachmentInput>,
) -> Result<(StatusCode, Json<bastion_cotizaciones::QuoteAttachmentWrite>), ApiError> {
    let user = actor(&state, &headers).await?;
    state
        .cotizaciones
        .upload_attachment(
            user.id,
            id,
            expected_version(&headers)?,
            single_header(&headers, "idempotency-key")?,
            input,
        )
        .await
        .map(|result| (StatusCode::CREATED, Json(result)))
        .map_err(ApiError)
}
async fn attachment_content(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Path((id, attachment)): Path<(u64, String)>,
) -> Result<Json<bastion_cotizaciones::QuoteDocument>, ApiError> {
    let user = actor(&state, &headers).await?;
    state
        .cotizaciones
        .attachment_content(user.id, id, &attachment)
        .await
        .map(Json)
        .map_err(ApiError)
}
async fn remove_attachment(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Path((id, attachment)): Path<(u64, String)>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let user = actor(&state, &headers).await?;
    state
        .cotizaciones
        .remove_attachment(user.id, id, &attachment, expected_version(&headers)?)
        .await
        .map(|version| Json(serde_json::json!({"quote_version":version})))
        .map_err(ApiError)
}
