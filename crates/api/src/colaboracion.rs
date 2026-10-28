use super::*;
use bastion_colaboracion::{FileFilter, FilePage, UploadInput};
static FILE_REQUEST_SLOTS: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(2);

pub(super) fn routes(state: ApiState) -> Router<ApiState> {
    Router::new()
        .merge(form_routes(state.clone()))
        .route("/api/v1/colaboracion", get(list).post(upload))
        .route("/api/v1/colaboracion/personas", get(people))
        .route(
            "/api/v1/colaboracion/settings",
            get(settings).put(save_settings),
        )
        .route("/api/v1/colaboracion/{id}/restore", post(restore))
        .route("/api/v1/colaboracion/{id}/purge", post(purge))
        .route("/api/v1/colaboracion/{id}/preview", get(preview))
        .route("/api/v1/colaboracion/{id}/download", get(download))
        .route("/api/v1/colaboracion/{id}", axum::routing::delete(remove))
        .layer(axum::extract::DefaultBodyLimit::max(140 * 1024 * 1024))
        .layer(RequestBodyLimitLayer::new(140 * 1024 * 1024))
        .route_layer(middleware::from_fn_with_state(state, guard))
}

fn form_routes(state: ApiState) -> Router<ApiState> {
    Router::new()
        .route(
            "/api/v1/colaboracion/formularios",
            get(list_forms).post(publish_form),
        )
        .layer(RequestBodyLimitLayer::new(256 * 1024))
        .route_layer(middleware::from_fn_with_state(state, form_guard))
}
async fn form_guard(State(_state): State<ApiState>, request: Request, next: Next) -> Response {
    // Authorization is enforced again by owner methods; bounded metadata timeout.
    match tokio::time::timeout(Duration::from_secs(30), next.run(request)).await {
        Ok(response) => response,
        Err(_) => ApiError(CoreError::Overloaded).into_response(),
    }
}
async fn list_forms(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Query(filter): Query<bastion_licitaciones::PageQuery>,
) -> Result<Json<bastion_colaboracion::FilePage<bastion_colaboracion::PublishedForm>>, ApiError> {
    let user = actor(&state, &headers).await?;
    let (page, size) = filter.bounds().map_err(ApiError)?;
    state
        .colaboracion
        .list_forms(user.id, page, size)
        .await
        .map(Json)
        .map_err(ApiError)
}
async fn publish_form(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Json(input): Json<bastion_colaboracion::PublishFormInput>,
) -> Result<(StatusCode, Json<bastion_colaboracion::PublishedForm>), ApiError> {
    let user = actor(&state, &headers).await?;
    let key = super::licitaciones::single_header(&headers, "idempotency-key")?;
    state
        .colaboracion
        .publish_form(user.id, key, input)
        .await
        .map(|value| (StatusCode::CREATED, Json(value)))
        .map_err(ApiError)
}
async fn guard(State(state): State<ApiState>, request: Request, next: Next) -> Response {
    match tokio::time::timeout(Duration::from_secs(120), async {
        let user = actor(&state, request.headers()).await?;
        let capability = match *request.method() {
            Method::GET | Method::HEAD => "ver",
            Method::DELETE => "eliminar",
            _ if request.uri().path().ends_with("/restore")
                || request.uri().path().ends_with("/purge") =>
            {
                "eliminar"
            }
            _ => "editar",
        };
        state
            .colaboracion
            .check_access(user.id, capability)
            .await
            .map_err(ApiError)?;
        let _permit = tokio::time::timeout(Duration::from_secs(1), FILE_REQUEST_SLOTS.acquire())
            .await
            .map_err(|_| ApiError(CoreError::Overloaded))?
            .map_err(|_| ApiError(CoreError::Overloaded))?;
        let path = request.uri().path();
        let large = (*request.method() == Method::POST && path == "/api/v1/colaboracion")
            || path.ends_with("/preview")
            || path.ends_with("/download");
        let _large_permit = if large {
            Some(
                super::LARGE_FILE_SLOTS
                    .try_acquire()
                    .map_err(|_| ApiError(CoreError::Overloaded))?,
            )
        } else {
            None
        };
        state
            .colaboracion
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
async fn settings(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
) -> Result<Json<bastion_colaboracion::FilePolicy>, ApiError> {
    let user = actor(&state, &headers).await?;
    state
        .colaboracion
        .policy(user.id)
        .await
        .map(Json)
        .map_err(ApiError)
}
async fn save_settings(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Json(policy): Json<bastion_colaboracion::FilePolicy>,
) -> Result<StatusCode, ApiError> {
    let user = actor(&state, &headers).await?;
    state
        .colaboracion
        .save_policy(user.id, policy)
        .await
        .map_err(ApiError)?;
    Ok(StatusCode::NO_CONTENT)
}
async fn restore(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Path(id): Path<u64>,
) -> Result<StatusCode, ApiError> {
    let user = actor(&state, &headers).await?;
    state
        .colaboracion
        .restore(user.id, id)
        .await
        .map_err(ApiError)?;
    Ok(StatusCode::NO_CONTENT)
}
async fn purge(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Path(id): Path<u64>,
    Json(input): Json<DeleteInput>,
) -> Result<StatusCode, ApiError> {
    let user = actor(&state, &headers).await?;
    state
        .colaboracion
        .purge(user.id, id, input.confirm_id)
        .await
        .map_err(ApiError)?;
    Ok(StatusCode::NO_CONTENT)
}
async fn list(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Query(filter): Query<FileFilter>,
) -> Result<Json<FilePage>, ApiError> {
    let user = actor(&state, &headers).await?;
    state
        .colaboracion
        .list(user.id, &filter)
        .await
        .map(Json)
        .map_err(ApiError)
}
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct PeopleQuery {
    q: Option<String>,
    page: Option<u32>,
    page_size: Option<u32>,
}
async fn people(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Query(query): Query<PeopleQuery>,
) -> Result<Json<FilePage<bastion_core::PersonLabel>>, ApiError> {
    let user = actor(&state, &headers).await?;
    let page = query.page.unwrap_or(1);
    let size = query.page_size.unwrap_or(25);
    let mut items = state
        .core
        .admin()
        .module_person_labels(
            user.id,
            "colaboracion",
            query.q.as_deref().unwrap_or(""),
            page,
            size,
        )
        .await
        .map_err(ApiError)?;
    let has_more = items.len() > size as usize;
    items.truncate(size as usize);
    Ok(Json(FilePage {
        items,
        page,
        page_size: size,
        has_more,
    }))
}
async fn upload(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Json(input): Json<UploadInput>,
) -> Result<Response, ApiError> {
    let user = actor(&state, &headers).await?;
    if headers.get_all("idempotency-key").iter().count() != 1 {
        return Err(ApiError(CoreError::Invalid));
    }
    let key = headers
        .get("idempotency-key")
        .and_then(|v| v.to_str().ok())
        .ok_or(ApiError(CoreError::Invalid))?;
    let record = state
        .colaboracion
        .upload(user.id, key, input)
        .await
        .map_err(ApiError)?;
    let location = format!("/api/v1/colaboracion/{}/download", record.id);
    let mut response = (StatusCode::CREATED, Json(record)).into_response();
    response.headers_mut().insert(
        "location",
        HeaderValue::from_str(&location).map_err(|_| ApiError(CoreError::Invalid))?,
    );
    Ok(response)
}
async fn preview(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Path(id): Path<u64>,
) -> Result<Json<bastion_colaboracion::FileContent>, ApiError> {
    let user = actor(&state, &headers).await?;
    state
        .colaboracion
        .read(user.id, id, true)
        .await
        .map(Json)
        .map_err(ApiError)
}
async fn download(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Path(id): Path<u64>,
) -> Result<Json<bastion_colaboracion::FileContent>, ApiError> {
    let user = actor(&state, &headers).await?;
    state
        .colaboracion
        .read(user.id, id, false)
        .await
        .map(Json)
        .map_err(ApiError)
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DeleteInput {
    confirm_id: u64,
}
async fn remove(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Path(id): Path<u64>,
    Json(input): Json<DeleteInput>,
) -> Result<StatusCode, ApiError> {
    let user = actor(&state, &headers).await?;
    state
        .colaboracion
        .delete(user.id, id, input.confirm_id)
        .await
        .map_err(ApiError)?;
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::{Body, to_bytes};
    use bastion_core::Database;
    use sqlx::mysql::MySqlPoolOptions;
    use tower::ServiceExt;
    #[tokio::test]
    async fn mysql57_http_session_csrf_settings_and_disabled_module() {
        let Ok(url) = std::env::var("BASTION_COLABORACION_HTTP_TEST_URL") else {
            eprintln!("Requires isolated BASTION_COLABORACION_HTTP_TEST_URL; CI supplies it.");
            return;
        };
        let pool = MySqlPoolOptions::new()
            .max_connections(5)
            .connect(&url)
            .await
            .unwrap();
        let database = Database::from_test_pool(pool.clone());
        database.migrate().await.unwrap();
        bastion_colaboracion::CollaborationService::migrate(&database)
            .await
            .unwrap();
        sqlx::query("INSERT INTO persons (id,full_name) VALUES (1,'Admin HTTP'),(2,'Viewer HTTP')")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO roles (id,name) VALUES (2,'Viewer HTTP')")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO users (id,person_id,role_id,email,status) VALUES (1,1,1,'admin@example.test','active'),(2,2,2,'viewer@example.test','active')").execute(&pool).await.unwrap();
        for (user, token) in [
            (1_u64, "synthetic-admin-session"),
            (2, "synthetic-viewer-session"),
        ] {
            sqlx::query("INSERT INTO sessions (user_id,token_hash,csrf_hash,ip_address,started_at,last_activity_at,last_totp_at,expires_at) VALUES (?,UNHEX(SHA2(?,256)),UNHEX(SHA2('synthetic-csrf',256)),'127.0.0.1',UTC_TIMESTAMP(6),UTC_TIMESTAMP(6),UTC_TIMESTAMP(6),UTC_TIMESTAMP(6)+INTERVAL 1 HOUR)").bind(user).bind(token).execute(&pool).await.unwrap();
        }
        let core = CoreService::new(database.clone(), "x".repeat(32), [1; 32]).unwrap();
        bastion_colaboracion::CollaborationService::new(database, core.admin())
            .synchronize_release(true)
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
        let request = |method: &str, path: &str, session: Option<&str>, csrf: bool, body: &str| {
            let mut builder = axum::http::Request::builder()
                .method(method)
                .uri(path)
                .header("origin", "https://example.test")
                .header("content-type", "application/json");
            if let Some(token) = session {
                builder = builder.header(
                    "cookie",
                    format!("__Host-bastion_session={token}; __Host-bastion_csrf=synthetic-csrf"),
                );
            }
            if csrf {
                builder = builder.header("x-csrf-token", "synthetic-csrf");
            }
            let mut request = builder.body(Body::from(body.to_owned())).unwrap();
            request
                .extensions_mut()
                .insert(ConnectInfo("127.0.0.1:5010".parse::<SocketAddr>().unwrap()));
            request
        };
        for (session, status) in [
            (None, StatusCode::UNAUTHORIZED),
            (Some("synthetic-viewer-session"), StatusCode::FORBIDDEN),
        ] {
            assert_eq!(
                app.clone()
                    .oneshot(request("GET", "/api/v1/colaboracion", session, false, ""))
                    .await
                    .unwrap()
                    .status(),
                status
            );
        }
        let response = app
            .clone()
            .oneshot(request(
                "GET",
                "/api/v1/colaboracion/settings",
                Some("synthetic-admin-session"),
                false,
                "",
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let value: serde_json::Value =
            serde_json::from_slice(&to_bytes(response.into_body(), 16384).await.unwrap()).unwrap();
        assert_eq!(value["max_file_mb"], 50);
        assert_eq!(value["trash_retention_days"], 30);
        let mut policy = bastion_colaboracion::FilePolicy::default();
        policy.max_file_mb = 20;
        policy.trash_retention_days = 15;
        let body = serde_json::to_string(&policy).unwrap();
        assert_eq!(
            app.clone()
                .oneshot(request(
                    "PUT",
                    "/api/v1/colaboracion/settings",
                    Some("synthetic-admin-session"),
                    false,
                    &body
                ))
                .await
                .unwrap()
                .status(),
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            app.clone()
                .oneshot(request(
                    "PUT",
                    "/api/v1/colaboracion/settings",
                    Some("synthetic-admin-session"),
                    true,
                    &body
                ))
                .await
                .unwrap()
                .status(),
            StatusCode::NO_CONTENT
        );
        sqlx::query("INSERT INTO role_permissions (role_id,module_id,scope_hash,scope_key,capability) VALUES (2,'colaboracion',UNHEX(SHA2('persona:2',256)),'persona:2','ver')").execute(&pool).await.unwrap();
        assert_eq!(
            app.clone()
                .oneshot(request(
                    "GET",
                    "/api/v1/colaboracion?scope=empresa",
                    Some("synthetic-viewer-session"),
                    false,
                    ""
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
                    "/api/v1/colaboracion/1/purge",
                    Some("synthetic-viewer-session"),
                    true,
                    "{\"confirm_id\":1}"
                ))
                .await
                .unwrap()
                .status(),
            StatusCode::FORBIDDEN
        );
        policy.max_file_mb = 101;
        assert_eq!(
            app.clone()
                .oneshot(request(
                    "PUT",
                    "/api/v1/colaboracion/settings",
                    Some("synthetic-admin-session"),
                    true,
                    &serde_json::to_string(&policy).unwrap()
                ))
                .await
                .unwrap()
                .status(),
            StatusCode::BAD_REQUEST
        );
        core.admin()
            .set_module_enabled(1, "colaboracion", false)
            .await
            .unwrap();
        assert_eq!(
            app.oneshot(request(
                "GET",
                "/api/v1/colaboracion/settings",
                Some("synthetic-admin-session"),
                false,
                ""
            ))
            .await
            .unwrap()
            .status(),
            StatusCode::NOT_FOUND
        );
    }
}
