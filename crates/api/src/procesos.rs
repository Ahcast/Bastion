use super::*;
use bastion_procesos::ProcessFilter;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RefreshRequest {}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ScoutStatusQuery {}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReviewRequest {
    marked: Option<bool>,
    note: Option<String>,
}

pub(super) fn routes(state: ApiState) -> Router<ApiState> {
    Router::new()
        .route("/api/v1/procesos/dashboard", get(dashboard))
        .route("/api/v1/procesos/status", get(status))
        .route("/api/v1/procesos/scout-status", get(scout_status))
        .route("/api/v1/procesos/entities", get(entities))
        .route("/api/v1/procesos/rows", get(rows))
        .route("/api/v1/procesos/rows/{id}", get(detail))
        .route(
            "/api/v1/procesos/rows/{id}/review",
            axum::routing::patch(review),
        )
        .route("/api/v1/procesos/export", get(export))
        .route("/api/v1/procesos/refresh", post(refresh))
        .layer(RequestBodyLimitLayer::new(GENERAL_REQUEST_BODY_LIMIT))
        .route_layer(middleware::from_fn_with_state(state, guard))
}

// Check identity, module status and import permission before opening local files.
async fn guard(State(state): State<ApiState>, request: Request, next: Next) -> Response {
    match tokio::time::timeout(
        Duration::from_secs(60),
        guarded_request(state, request, next),
    )
    .await
    {
        Ok(response) => response,
        Err(_) => ApiError(CoreError::Overloaded).into_response(),
    }
}

async fn guarded_request(state: ApiState, request: Request, next: Next) -> Response {
    let actor = match actor(&state, request.headers()).await {
        Ok(actor) => actor,
        Err(error) => return error.into_response(),
    };
    let path = request.uri().path();
    let (scope, capability, expensive) = match path {
        "/api/v1/procesos/refresh" => ("importacion", "editar", true),
        "/api/v1/procesos/export" => ("exportacion", "ver", true),
        "/api/v1/procesos/scout-status" => ("consulta", "ver", true),
        _ => ("consulta", "ver", false),
    };
    let access = if path.ends_with("/review") {
        state.procesos.check_enabled().await
    } else {
        state
            .procesos
            .check_access(actor.id, scope, capability)
            .await
    };
    if let Err(error) = access {
        return ApiError(error).into_response();
    }
    if let Err(error) = state.procesos.consume_request(actor.id).await {
        return ApiError(error).into_response();
    }
    let _permit = if expensive {
        match state.procesos_jobs.clone().try_acquire_owned() {
            Ok(permit) => Some(permit),
            Err(_) => return ApiError(CoreError::Overloaded).into_response(),
        }
    } else {
        None
    };
    next.run(request).await
}

async fn status(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Query(filter): Query<ProcessFilter>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let actor = actor(&state, &headers).await?;
    state
        .procesos
        .overview(actor.id, &filter)
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
        .procesos
        .dashboard(user.id, query.period)
        .await
        .map(Json)
        .map_err(ApiError)
}

async fn scout_status(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Query(_query): Query<ScoutStatusQuery>,
) -> Result<Json<bastion_procesos::ScoutDiagnostics>, ApiError> {
    let actor = actor(&state, &headers).await?;
    let path = state.scout_csv_path.clone();
    let report =
        tokio::task::spawn_blocking(move || bastion_procesos::ScoutDiagnostics::read(&path))
            .await
            .map_err(|_| ApiError(CoreError::Overloaded))?;
    state
        .procesos
        .check_access(actor.id, "consulta", "ver")
        .await
        .map_err(ApiError)?;
    Ok(Json(report))
}

async fn entities(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
    let actor = actor(&state, &headers).await?;
    state
        .procesos
        .entities(actor.id)
        .await
        .map(Json)
        .map_err(ApiError)
}

async fn rows(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Query(filter): Query<ProcessFilter>,
) -> Result<Json<bastion_procesos::ProcessPage>, ApiError> {
    let actor = actor(&state, &headers).await?;
    state
        .procesos
        .rows(actor.id, &filter)
        .await
        .map(Json)
        .map_err(ApiError)
}

async fn detail(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    let actor = actor(&state, &headers).await?;
    let record = state
        .procesos
        .detail(actor.id, &id)
        .await
        .map_err(ApiError)?;
    let version = record.version;
    Ok(with_version(Json(record).into_response(), version))
}

fn with_version(mut response: Response, version: u64) -> Response {
    if let Ok(value) = HeaderValue::from_str(&format!("\"{version}\"")) {
        response.headers_mut().insert("etag", value);
    }
    response
}

async fn review(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<ReviewRequest>,
) -> Result<Response, ApiError> {
    let actor = actor(&state, &headers).await?;
    if headers.get_all("if-match").iter().count() != 1 {
        return Err(ApiError(CoreError::Invalid));
    }
    let version = headers
        .get("if-match")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix('"'))
        .and_then(|v| v.strip_suffix('"'))
        .and_then(|v| v.parse::<u64>().ok())
        .ok_or(ApiError(CoreError::Invalid))?;
    let review = state
        .procesos
        .review(actor.id, &id, version, body.marked, body.note.as_deref())
        .await
        .map_err(ApiError)?;
    let version = review.version;
    Ok(with_version(Json(review).into_response(), version))
}

async fn refresh(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Json(_body): Json<RefreshRequest>,
) -> Result<Json<bastion_procesos::ImportResult>, Response> {
    let actor = actor(&state, &headers)
        .await
        .map_err(IntoResponse::into_response)?;
    let path = state.scout_csv_path.clone();
    let delivery =
        tokio::task::spawn_blocking(move || bastion_procesos::ReadyDelivery::read(&path))
            .await
            .map_err(|_| local_delivery_problem(bastion_procesos::LocalDeliveryError::Io))?
            .map_err(local_delivery_problem)?;
    state
        .procesos
        .import_scout_for_user(actor.id, &delivery)
        .await
        .map(Json)
        .map_err(|error| ApiError(error).into_response())
}

fn local_delivery_problem(error: bastion_procesos::LocalDeliveryError) -> Response {
    use bastion_procesos::LocalDeliveryError;
    let (status, detail) = match error {
        LocalDeliveryError::Pending | LocalDeliveryError::Changed => (
            StatusCode::CONFLICT,
            "Scout aún no publicó filas listas para incorporar o está actualizando sus resultados. Intenta más tarde.",
        ),
        LocalDeliveryError::Invalid => (
            StatusCode::UNPROCESSABLE_ENTITY,
            "Los resultados de Scout no cumplen el contrato de entrega. No se importaron registros.",
        ),
        LocalDeliveryError::Missing | LocalDeliveryError::Io => (
            StatusCode::SERVICE_UNAVAILABLE,
            "Los resultados de Scout no están disponibles. Revisa la publicación y los permisos del servicio.",
        ),
    };
    problem(status, "Scout Delivery Unavailable", detail)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refresh_cannot_select_a_file_or_upload_csv() {
        assert!(serde_json::from_str::<ScoutStatusQuery>("{}").is_ok());
        assert!(serde_json::from_str::<ScoutStatusQuery>(r#"{"path":"/etc/passwd"}"#).is_err());
        assert!(serde_json::from_str::<ScoutStatusQuery>(r#"{"file":"config.json"}"#).is_err());
        assert!(serde_json::from_str::<RefreshRequest>("{}").is_ok());
        for payload in [
            r#"{"path":"/etc/passwd"}"#,
            r#"{"csv_text":"untrusted"}"#,
            r#"{"url":"https://example.test/maestro.csv"}"#,
        ] {
            assert!(serde_json::from_str::<RefreshRequest>(payload).is_err());
        }
    }

    #[test]
    fn unavailable_or_partial_delivery_is_not_reported_as_success() {
        use bastion_procesos::LocalDeliveryError::*;
        for (error, expected) in [
            (Pending, StatusCode::CONFLICT),
            (Changed, StatusCode::CONFLICT),
            (Invalid, StatusCode::UNPROCESSABLE_ENTITY),
            (Missing, StatusCode::SERVICE_UNAVAILABLE),
            (Io, StatusCode::SERVICE_UNAVAILABLE),
        ] {
            assert_eq!(local_delivery_problem(error).status(), expected);
        }
    }
}

async fn export(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Query(filter): Query<ProcessFilter>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let actor = actor(&state, &headers).await?;
    let csv = state
        .procesos
        .export(actor.id, &filter)
        .await
        .map_err(ApiError)?;
    Ok(Json(serde_json::json!({"csv_text": csv})))
}
