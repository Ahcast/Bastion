use super::*;
use bastion_licitaciones::{
    CopyFormInput, DeleteInput, EmptyInput, Expedient, GlobalVariablesInput, LotInput, NameInput,
    PageQuery, PdfOrderInput, ValuesInput,
};

pub(super) fn routes(state: ApiState) -> Router<ApiState> {
    Router::new()
        .route("/api/v1/licitaciones", get(list).post(create))
        .route(
            "/api/v1/licitaciones/{id}",
            get(detail).patch(rename).delete(remove),
        )
        .route("/api/v1/licitaciones/{id}/formularios", post(copy_form))
        .route(
            "/api/v1/licitaciones/{id}/variables",
            axum::routing::put(global_variables),
        )
        .route("/api/v1/licitaciones/{id}/lotes", get(lot_summaries))
        .route(
            "/api/v1/licitaciones/{id}/lotes/{numero}",
            get(lot).put(save_lot),
        )
        .route(
            "/api/v1/licitaciones/{id}/pdf/orden",
            get(pdf_order).put(save_pdf_order),
        )
        .route("/api/v1/licitaciones/{id}/pdf/generar", post(generate_pdf))
        .route("/api/v1/licitaciones/{id}/pdf/ultimo", get(latest_pdf))
        .route("/api/v1/licitaciones/{id}/pdf/trabajos/{job}", get(pdf_job))
        .route(
            "/api/v1/licitaciones/{id}/formularios/{copy_id}/valores",
            axum::routing::put(values),
        )
        .layer(RequestBodyLimitLayer::new(256 * 1024))
        .route_layer(middleware::from_fn_with_state(state.clone(), guard))
        .merge(file_routes(state))
}
fn file_routes(state: ApiState) -> Router<ApiState> {
    Router::new()
        .route(
            "/api/v1/licitaciones/{id}/archivos",
            get(files).post(upload_file),
        )
        .route(
            "/api/v1/licitaciones/{id}/archivos/desde-colaboracion",
            post(copy_file),
        )
        .route(
            "/api/v1/licitaciones/{id}/archivos/{attachment}/preview",
            get(preview_file),
        )
        .route(
            "/api/v1/licitaciones/{id}/archivos/{attachment}/download",
            get(download_file),
        )
        .route(
            "/api/v1/licitaciones/{id}/pdf/trabajos/{job}/archivo",
            get(download_final_pdf),
        )
        .route(
            "/api/v1/licitaciones/{id}/archivos/{attachment}",
            axum::routing::delete(remove_file),
        )
        .layer(axum::extract::DefaultBodyLimit::max(134_000_000))
        .layer(RequestBodyLimitLayer::new(134_000_000))
        .route_layer(middleware::from_fn_with_state(state, file_guard))
}
async fn file_guard(State(state): State<ApiState>, request: Request, next: Next) -> Response {
    match tokio::time::timeout(Duration::from_secs(900), async {
        let user = actor(&state, request.headers()).await?;
        let capability = match *request.method() {
            Method::GET | Method::HEAD => "ver",
            Method::DELETE => "eliminar",
            _ => "editar",
        };
        state
            .licitaciones
            .check_access(user.id, capability)
            .await
            .map_err(ApiError)?;
        state
            .licitaciones
            .consume_request(user.id)
            .await
            .map_err(ApiError)?;
        let path = request.uri().path();
        let large = (*request.method() == Method::POST && path.ends_with("/archivos"))
            || path.ends_with("/preview")
            || path.ends_with("/download");
        let _permit = if large {
            Some(
                super::LARGE_FILE_SLOTS
                    .try_acquire()
                    .map_err(|_| ApiError(CoreError::Overloaded))?,
            )
        } else {
            None
        };
        Ok::<_, ApiError>(next.run(request).await)
    })
    .await
    {
        Ok(Ok(response)) => response,
        Ok(Err(e)) => e.into_response(),
        Err(_) => ApiError(CoreError::Overloaded).into_response(),
    }
}
struct PdfStream {
    inner: tokio::fs::File,
    _permit: tokio::sync::SemaphorePermit<'static>,
}
impl futures_core::Stream for PdfStream {
    type Item = Result<axum::body::Bytes, std::io::Error>;
    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        let mut bytes = vec![0u8; 64 * 1024];
        let mut buffer = tokio::io::ReadBuf::new(&mut bytes);
        match tokio::io::AsyncRead::poll_read(std::pin::Pin::new(&mut self.inner), cx, &mut buffer)
        {
            std::task::Poll::Pending => std::task::Poll::Pending,
            std::task::Poll::Ready(Err(error)) => std::task::Poll::Ready(Some(Err(error))),
            std::task::Poll::Ready(Ok(())) => {
                let count = buffer.filled().len();
                if count == 0 {
                    return std::task::Poll::Ready(None);
                }
                bytes.truncate(count);
                std::task::Poll::Ready(Some(Ok(axum::body::Bytes::from(bytes))))
            }
        }
    }
}
async fn download_final_pdf(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Path((id, job)): Path<(u64, String)>,
) -> Result<Response, ApiError> {
    let user = actor(&state, &headers).await?;
    let permit = super::LARGE_FILE_SLOTS
        .try_acquire()
        .map_err(|_| ApiError(CoreError::Overloaded))?;
    let (file, size) = state
        .licitaciones
        .download_pdf_job(user.id, id, &job)
        .await
        .map_err(ApiError)?;
    let body = PdfStream {
        inner: tokio::fs::File::from_std(file),
        _permit: permit,
    };
    let mut response = Response::new(axum::body::Body::from_stream(body));
    let headers = response.headers_mut();
    headers.insert(
        axum::http::header::CONTENT_TYPE,
        HeaderValue::from_static("application/pdf"),
    );
    headers.insert(
        axum::http::header::CONTENT_DISPOSITION,
        HeaderValue::from_static("attachment; filename=\"licitacion.pdf\""),
    );
    headers.insert(
        axum::http::header::CACHE_CONTROL,
        HeaderValue::from_static("private, no-store"),
    );
    headers.insert(
        axum::http::header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(
        axum::http::header::CONTENT_LENGTH,
        HeaderValue::from_str(&size.to_string()).map_err(|_| ApiError(CoreError::Invalid))?,
    );
    Ok(response)
}

async fn files(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Path(id): Path<u64>,
) -> Result<Json<bastion_licitaciones::AttachmentList>, ApiError> {
    let user = actor(&state, &headers).await?;
    state
        .licitaciones
        .attachments(user.id, id)
        .await
        .map(Json)
        .map_err(ApiError)
}
async fn upload_file(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Path(id): Path<u64>,
    Json(input): Json<bastion_licitaciones::AttachmentUpload>,
) -> Result<Response, ApiError> {
    let user = actor(&state, &headers).await?;
    state
        .licitaciones
        .upload_attachment(
            user.id,
            id,
            expected(&headers)?,
            single_header(&headers, "idempotency-key")?,
            input,
        )
        .await
        .map(response)
        .map_err(ApiError)
}
async fn copy_file(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Path(id): Path<u64>,
    Json(input): Json<bastion_licitaciones::AttachmentCopy>,
) -> Result<Response, ApiError> {
    let user = actor(&state, &headers).await?;
    state
        .licitaciones
        .copy_attachment(
            user.id,
            id,
            expected(&headers)?,
            single_header(&headers, "idempotency-key")?,
            input,
        )
        .await
        .map(response)
        .map_err(ApiError)
}
async fn preview_file(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Path((id, file)): Path<(u64, String)>,
) -> Result<Json<bastion_licitaciones::AttachmentContent>, ApiError> {
    let user = actor(&state, &headers).await?;
    state
        .licitaciones
        .read_attachment(user.id, id, &file, true)
        .await
        .map(Json)
        .map_err(ApiError)
}
async fn download_file(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Path((id, file)): Path<(u64, String)>,
) -> Result<Json<bastion_licitaciones::AttachmentContent>, ApiError> {
    let user = actor(&state, &headers).await?;
    state
        .licitaciones
        .read_attachment(user.id, id, &file, false)
        .await
        .map(Json)
        .map_err(ApiError)
}
async fn remove_file(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Path((id, file)): Path<(u64, String)>,
    Json(input): Json<bastion_licitaciones::AttachmentDelete>,
) -> Result<Response, ApiError> {
    let user = actor(&state, &headers).await?;
    state
        .licitaciones
        .remove_attachment(user.id, id, &file, expected(&headers)?, &input.confirm_id)
        .await
        .map(response)
        .map_err(ApiError)
}
async fn guard(State(state): State<ApiState>, request: Request, next: Next) -> Response {
    match tokio::time::timeout(Duration::from_secs(30), async {
        let user = actor(&state, request.headers()).await?;
        let capability = match *request.method() {
            Method::GET | Method::HEAD => "ver",
            Method::DELETE => "eliminar",
            _ => "editar",
        };
        state
            .licitaciones
            .check_access(user.id, capability)
            .await
            .map_err(ApiError)?;
        state
            .licitaciones
            .consume_request(user.id)
            .await
            .map_err(ApiError)?;
        Ok::<_, ApiError>(next.run(request).await)
    })
    .await
    {
        Ok(Ok(response)) => response,
        Ok(Err(error)) => error.into_response(),
        Err(_) => ApiError(CoreError::Overloaded).into_response(),
    }
}
async fn list(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Query(filter): Query<PageQuery>,
) -> Result<Json<bastion_licitaciones::ExpedientPage>, ApiError> {
    let user = actor(&state, &headers).await?;
    state
        .licitaciones
        .list(user.id, &filter)
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
    state
        .licitaciones
        .detail(user.id, id)
        .await
        .map(response)
        .map_err(ApiError)
}
async fn create(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Json(_): Json<EmptyInput>,
) -> Result<Response, ApiError> {
    let user = actor(&state, &headers).await?;
    let result = state
        .licitaciones
        .create(user.id, single_header(&headers, "idempotency-key")?)
        .await
        .map_err(ApiError)?;
    let id = result.id;
    let mut output = response(result);
    *output.status_mut() = StatusCode::CREATED;
    output.headers_mut().insert(
        "location",
        HeaderValue::from_str(&format!("/api/v1/licitaciones/{id}")).expect("integer location"),
    );
    Ok(output)
}
async fn rename(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Path(id): Path<u64>,
    Json(input): Json<NameInput>,
) -> Result<Response, ApiError> {
    let user = actor(&state, &headers).await?;
    state
        .licitaciones
        .rename(user.id, id, expected(&headers)?, input)
        .await
        .map(response)
        .map_err(ApiError)
}
async fn copy_form(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Path(id): Path<u64>,
    Json(input): Json<CopyFormInput>,
) -> Result<Response, ApiError> {
    let user = actor(&state, &headers).await?;
    state
        .licitaciones
        .copy_form(
            user.id,
            id,
            expected(&headers)?,
            single_header(&headers, "idempotency-key")?,
            input,
        )
        .await
        .map(response)
        .map_err(ApiError)
}
async fn values(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Path((id, copy_id)): Path<(u64, u64)>,
    Json(input): Json<ValuesInput>,
) -> Result<Response, ApiError> {
    let user = actor(&state, &headers).await?;
    state
        .licitaciones
        .save_values(user.id, id, copy_id, expected(&headers)?, input)
        .await
        .map(response)
        .map_err(ApiError)
}
async fn global_variables(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Path(id): Path<u64>,
    Json(input): Json<GlobalVariablesUpdate>,
) -> Result<Response, ApiError> {
    let user = actor(&state, &headers).await?;
    state
        .licitaciones
        .save_global_variables(
            user.id,
            id,
            expected(&headers)?,
            GlobalVariablesInput {
                variables: input.variables,
            },
            input.confirm_discard,
        )
        .await
        .map(response)
        .map_err(ApiError)
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct GlobalVariablesUpdate {
    variables: Vec<bastion_licitaciones::GlobalVariable>,
    #[serde(default)]
    confirm_discard: bool,
}
async fn lot(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Path((id, numero)): Path<(u64, u32)>,
) -> Result<Json<bastion_licitaciones::Lot>, ApiError> {
    let user = actor(&state, &headers).await?;
    state
        .licitaciones
        .lot(user.id, id, numero)
        .await
        .map(Json)
        .map_err(ApiError)
}
async fn lot_summaries(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Path(id): Path<u64>,
) -> Result<Json<Vec<bastion_licitaciones::LotSummary>>, ApiError> {
    let user = actor(&state, &headers).await?;
    state
        .licitaciones
        .lot_summaries(user.id, id)
        .await
        .map(Json)
        .map_err(ApiError)
}
async fn save_lot(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Path((id, numero)): Path<(u64, u32)>,
    Json(input): Json<LotInput>,
) -> Result<Response, ApiError> {
    let user = actor(&state, &headers).await?;
    state
        .licitaciones
        .save_lot(user.id, id, numero, expected(&headers)?, input)
        .await
        .map(response)
        .map_err(ApiError)
}
async fn generate_pdf(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Path(id): Path<u64>,
    Json(_input): Json<EmptyInput>,
) -> Result<Response, ApiError> {
    if !bastion_licitaciones::TenderService::pdf_generation_enabled() {
        return Err(ApiError(CoreError::NotFound));
    }
    let user = actor(&state, &headers).await?;
    let job = state
        .licitaciones
        .create_pdf_job(user.id, id, expected(&headers)?)
        .await
        .map_err(ApiError)?;
    Ok((StatusCode::ACCEPTED, Json(job)).into_response())
}
async fn latest_pdf(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Path(id): Path<u64>,
) -> Result<Json<Option<bastion_licitaciones::PdfJob>>, ApiError> {
    let user = actor(&state, &headers).await?;
    state
        .licitaciones
        .latest_pdf_job(user.id, id)
        .await
        .map(Json)
        .map_err(ApiError)
}
async fn pdf_job(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Path((id, job)): Path<(u64, String)>,
) -> Result<Json<bastion_licitaciones::PdfJob>, ApiError> {
    let user = actor(&state, &headers).await?;
    state
        .licitaciones
        .pdf_job(user.id, id, &job)
        .await
        .map(Json)
        .map_err(ApiError)
}

async fn pdf_order(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Path(id): Path<u64>,
) -> Result<Json<bastion_licitaciones::PdfOrder>, ApiError> {
    let user = actor(&state, &headers).await?;
    state
        .licitaciones
        .pdf_order(user.id, id)
        .await
        .map(Json)
        .map_err(ApiError)
}
async fn save_pdf_order(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Path(id): Path<u64>,
    Json(input): Json<PdfOrderInput>,
) -> Result<Response, ApiError> {
    let user = actor(&state, &headers).await?;
    state
        .licitaciones
        .save_pdf_order(user.id, id, expected(&headers)?, input)
        .await
        .map(response)
        .map_err(ApiError)
}
async fn remove(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Path(id): Path<u64>,
    Json(input): Json<DeleteInput>,
) -> Result<StatusCode, ApiError> {
    let user = actor(&state, &headers).await?;
    state
        .licitaciones
        .delete(user.id, id, expected(&headers)?, input.confirm_id)
        .await
        .map_err(ApiError)?;
    Ok(StatusCode::NO_CONTENT)
}
pub(super) fn single_header<'a>(
    headers: &'a axum::http::HeaderMap,
    name: &str,
) -> Result<&'a str, ApiError> {
    if headers.get_all(name).iter().count() != 1 {
        return Err(ApiError(CoreError::Invalid));
    }
    headers
        .get(name)
        .and_then(|v| v.to_str().ok())
        .ok_or(ApiError(CoreError::Invalid))
}
fn expected(headers: &axum::http::HeaderMap) -> Result<u64, ApiError> {
    let value = single_header(headers, "if-match")?;
    let value = value
        .strip_prefix('"')
        .and_then(|v| v.strip_suffix('"'))
        .ok_or(ApiError(CoreError::Invalid))?;
    if value.starts_with('0') || !value.bytes().all(|b| b.is_ascii_digit()) {
        return Err(ApiError(CoreError::Invalid));
    }
    value
        .parse()
        .ok()
        .filter(|v| *v > 0)
        .ok_or(ApiError(CoreError::Invalid))
}
fn response(expedient: Expedient) -> Response {
    let version = expedient.version;
    let mut response = Json(expedient).into_response();
    response.headers_mut().insert(
        "etag",
        HeaderValue::from_str(&format!("\"{version}\"")).expect("integer ETag"),
    );
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn mysql57_tender_http_session_csrf_versions_and_mass_assignment() {
        use axum::body::{Body, to_bytes};
        use bastion_core::Database;
        use tower::ServiceExt;
        let Ok(url) = std::env::var("BASTION_LICITACIONES_HTTP_TEST_URL") else {
            eprintln!("Requires isolated BASTION_LICITACIONES_HTTP_TEST_URL; CI supplies it.");
            return;
        };
        let pool = sqlx::mysql::MySqlPoolOptions::new()
            .max_connections(5)
            .connect(&url)
            .await
            .unwrap();
        let db = Database::from_test_pool(pool.clone());
        db.migrate().await.unwrap();
        bastion_licitaciones::TenderService::migrate(&db)
            .await
            .unwrap();
        bastion_colaboracion::CollaborationService::migrate(&db)
            .await
            .unwrap();
        sqlx::query("INSERT INTO persons (id,full_name) VALUES (1,'HTTP Admin'),(2,'HTTP Reader')")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO roles (id,name) VALUES (2,'HTTP Reader')")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO users (id,person_id,role_id,email,status) VALUES (1,1,1,'admin@example.test','active'),(2,2,2,'reader@example.test','active')").execute(&pool).await.unwrap();
        for (id, token) in [
            (1, "synthetic-tender-admin"),
            (2, "synthetic-tender-reader"),
        ] {
            sqlx::query("INSERT INTO sessions (user_id,token_hash,csrf_hash,ip_address,started_at,last_activity_at,last_totp_at,expires_at) VALUES (?,UNHEX(SHA2(?,256)),UNHEX(SHA2('synthetic-csrf',256)),'127.0.0.1',UTC_TIMESTAMP(6),UTC_TIMESTAMP(6),UTC_TIMESTAMP(6),UTC_TIMESTAMP(6)+INTERVAL 1 HOUR)")
                .bind(id).bind(token).execute(&pool).await.unwrap();
        }
        let core = CoreService::new(db.clone(), "x".repeat(32), [1; 32]).unwrap();
        let tenders = bastion_licitaciones::TenderService::new(db.clone(), core.admin());
        tenders.synchronize_release(true).await.unwrap();
        let library = bastion_colaboracion::CollaborationService::new(db, core.admin());
        library.synchronize_release(true).await.unwrap();
        core.admin()
            .set_module_enabled(1, "licitaciones", true)
            .await
            .unwrap();
        core.admin()
            .set_module_enabled(1, "colaboracion", true)
            .await
            .unwrap();
        let app = super::super::router(
            core.clone(),
            "https://example.test".into(),
            Vec::new(),
            "/unused".into(),
        )
        .unwrap();
        let request = |method: &str,
                       path: &str,
                       token: Option<&str>,
                       csrf: bool,
                       origin: &str,
                       body: String,
                       version: &str| {
            let mut builder = axum::http::Request::builder()
                .method(method)
                .uri(path)
                .header("content-type", "application/json")
                .header("origin", origin)
                .header("idempotency-key", "synthetic-http-key-01")
                .header("if-match", version);
            if let Some(token) = token {
                builder = builder.header(
                    "cookie",
                    format!("__Host-bastion_session={token}; __Host-bastion_csrf=synthetic-csrf"),
                );
            }
            if csrf {
                builder = builder.header("x-csrf-token", "synthetic-csrf");
            }
            let mut request = builder.body(Body::from(body)).unwrap();
            request
                .extensions_mut()
                .insert(ConnectInfo("127.0.0.1:5010".parse::<SocketAddr>().unwrap()));
            request
        };
        for (token, status) in [
            (None, StatusCode::UNAUTHORIZED),
            (Some("synthetic-tender-reader"), StatusCode::FORBIDDEN),
        ] {
            assert_eq!(
                app.clone()
                    .oneshot(request(
                        "GET",
                        "/api/v1/licitaciones",
                        token,
                        false,
                        "https://example.test",
                        "".into(),
                        "\"1\""
                    ))
                    .await
                    .unwrap()
                    .status(),
                status
            );
        }
        for (csrf, origin) in [
            (false, "https://example.test"),
            (true, "https://evil.example"),
        ] {
            assert_eq!(
                app.clone()
                    .oneshot(request(
                        "POST",
                        "/api/v1/licitaciones",
                        Some("synthetic-tender-admin"),
                        csrf,
                        origin,
                        "{}".into(),
                        "\"1\""
                    ))
                    .await
                    .unwrap()
                    .status(),
                StatusCode::FORBIDDEN
            );
        }
        let response = app
            .clone()
            .oneshot(request(
                "POST",
                "/api/v1/licitaciones",
                Some("synthetic-tender-admin"),
                true,
                "https://example.test",
                "{}".into(),
                "\"1\"",
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
        assert_eq!(response.headers()["etag"], "\"1\"");
        let value: serde_json::Value =
            serde_json::from_slice(&to_bytes(response.into_body(), 256 * 1024).await.unwrap())
                .unwrap();
        let id = value["id"].as_u64().unwrap();
        assert_eq!(value["forms"], serde_json::json!([]));
        let response = app
            .clone()
            .oneshot(request(
                "POST",
                "/api/v1/licitaciones",
                Some("synthetic-tender-admin"),
                true,
                "https://example.test",
                r#"{"state":"enviada"}"#.into(),
                "\"1\"",
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        let huge = format!("{{\"unexpected\":\"{}\"}}", "x".repeat(270 * 1024));
        assert_eq!(
            app.clone()
                .oneshot(request(
                    "POST",
                    "/api/v1/licitaciones",
                    Some("synthetic-tender-admin"),
                    true,
                    "https://example.test",
                    huge,
                    "\"1\""
                ))
                .await
                .unwrap()
                .status(),
            StatusCode::PAYLOAD_TOO_LARGE
        );
        let path = format!("/api/v1/licitaciones/{id}");
        assert_eq!(
            app.clone()
                .oneshot(request(
                    "DELETE",
                    &path,
                    Some("synthetic-tender-admin"),
                    true,
                    "https://example.test",
                    format!("{{\"confirm_id\":{id}}}"),
                    "\"2\""
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
                    &path,
                    Some("synthetic-tender-admin"),
                    true,
                    "https://example.test",
                    format!("{{\"confirm_id\":{id}}}"),
                    "W/\"1\""
                ))
                .await
                .unwrap()
                .status(),
            StatusCode::BAD_REQUEST
        );
        let schema=serde_json::json!({"definition":{"title":"HTTP sintético","section":"legal","variables":[]}}).to_string();
        sqlx::query("INSERT INTO role_permissions (role_id,module_id,scope_hash,scope_key,capability) VALUES (2,'colaboracion',UNHEX(SHA2('*',256)),'*','editar')").execute(&pool).await.unwrap();
        assert_eq!(
            app.clone()
                .oneshot(request(
                    "POST",
                    "/api/v1/colaboracion/formularios",
                    Some("synthetic-tender-reader"),
                    true,
                    "https://example.test",
                    schema.clone(),
                    "\"1\""
                ))
                .await
                .unwrap()
                .status(),
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            app.clone()
                .oneshot(request(
                    "POST",
                    "/api/v1/colaboracion/formularios",
                    Some("synthetic-tender-admin"),
                    true,
                    "https://example.test",
                    schema,
                    "\"1\""
                ))
                .await
                .unwrap()
                .status(),
            StatusCode::CREATED
        );
        let renamed = app
            .clone()
            .oneshot(request(
                "PATCH",
                &path,
                Some("synthetic-tender-admin"),
                true,
                "https://example.test",
                r#"{"name":"  Oferta HTTP  "}"#.into(),
                "\"1\"",
            ))
            .await
            .unwrap();
        assert_eq!(renamed.status(), StatusCode::OK);
        assert_eq!(renamed.headers()["etag"], "\"2\"");
        let renamed: serde_json::Value =
            serde_json::from_slice(&to_bytes(renamed.into_body(), 256 * 1024).await.unwrap())
                .unwrap();
        assert_eq!(renamed["name"], "Oferta HTTP");
        assert_eq!(renamed["id"], id);
        for (token, body, version, status) in [
            (
                "synthetic-tender-reader",
                r#"{"name":"No autorizado"}"#,
                "\"2\"",
                StatusCode::FORBIDDEN,
            ),
            (
                "synthetic-tender-admin",
                r#"{"name":"Obsoleto"}"#,
                "\"1\"",
                StatusCode::PRECONDITION_FAILED,
            ),
            (
                "synthetic-tender-admin",
                r#"{"name":"   "}"#,
                "\"2\"",
                StatusCode::BAD_REQUEST,
            ),
            (
                "synthetic-tender-admin",
                r#"{"name":"Otro","state":"enviada"}"#,
                "\"2\"",
                StatusCode::UNPROCESSABLE_ENTITY,
            ),
        ] {
            assert_eq!(
                app.clone()
                    .oneshot(request(
                        "PATCH",
                        &path,
                        Some(token),
                        true,
                        "https://example.test",
                        body.into(),
                        version
                    ))
                    .await
                    .unwrap()
                    .status(),
                status
            );
        }
        let variables_path = format!("{path}/variables");
        let global_body = r#"{"variables":[{"key":"nombre","label":"Nombre","kind":"text","value":"Valor privado"}]}"#;
        for (actor, csrf, version, status) in [
            (
                "synthetic-tender-reader",
                true,
                "\"2\"",
                StatusCode::FORBIDDEN,
            ),
            (
                "synthetic-tender-admin",
                false,
                "\"2\"",
                StatusCode::FORBIDDEN,
            ),
            (
                "synthetic-tender-admin",
                true,
                "\"1\"",
                StatusCode::PRECONDITION_FAILED,
            ),
        ] {
            assert_eq!(
                app.clone()
                    .oneshot(request(
                        "PUT",
                        &variables_path,
                        Some(actor),
                        csrf,
                        "https://example.test",
                        global_body.into(),
                        version
                    ))
                    .await
                    .unwrap()
                    .status(),
                status
            );
        }
        let saved = app
            .clone()
            .oneshot(request(
                "PUT",
                &variables_path,
                Some("synthetic-tender-admin"),
                true,
                "https://example.test",
                global_body.into(),
                "\"2\"",
            ))
            .await
            .unwrap();
        assert_eq!(saved.status(), StatusCode::OK);
        assert_eq!(saved.headers()["etag"], "\"3\"");
        assert_eq!(
            app.clone()
                .oneshot(request(
                    "DELETE",
                    &path,
                    Some("synthetic-tender-admin"),
                    true,
                    "https://example.test",
                    format!("{{\"confirm_id\":{id}}}"),
                    "\"3\""
                ))
                .await
                .unwrap()
                .status(),
            StatusCode::NO_CONTENT
        );
        core.admin()
            .set_module_enabled(1, "licitaciones", false)
            .await
            .unwrap();
        assert_eq!(
            app.oneshot(request(
                "GET",
                "/api/v1/licitaciones",
                Some("synthetic-tender-admin"),
                false,
                "https://example.test",
                "".into(),
                "\"1\""
            ))
            .await
            .unwrap()
            .status(),
            StatusCode::NOT_FOUND
        );
    }
    #[test]
    fn edits_require_single_strong_etag() {
        for value in [
            "*",
            "1",
            "W/\"1\"",
            "\"0\"",
            "\"01\"",
            "\"1\",\"2\"",
            "\"18446744073709551616\"",
        ] {
            let mut headers = axum::http::HeaderMap::new();
            headers.insert("if-match", value.parse().unwrap());
            assert!(expected(&headers).is_err());
        }
        let mut headers = axum::http::HeaderMap::new();
        headers.insert("if-match", HeaderValue::from_static("\"2\""));
        assert_eq!(expected(&headers).ok(), Some(2));
        headers.append("if-match", HeaderValue::from_static("\"3\""));
        assert!(expected(&headers).is_err());
    }
}
