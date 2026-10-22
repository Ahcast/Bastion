use super::*;
use bastion_procesos::ProcessFilter;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ImportRequest {
    csv_text: String,
}

pub(super) fn routes(state: ApiState) -> Router<ApiState> {
    Router::new()
        .route("/api/v1/procesos/status", get(status))
        .route("/api/v1/procesos/rows", get(rows))
        .route("/api/v1/procesos/rows/{id}", get(detail))
        .route("/api/v1/procesos/import", post(import))
        .route("/api/v1/procesos/export", get(export))
        .layer(axum::extract::DefaultBodyLimit::max(16 * 1024 * 1024))
        .layer(RequestBodyLimitLayer::new(16 * 1024 * 1024))
        .route_layer(middleware::from_fn_with_state(state, guard))
}

// Identity, module status and permission are checked before buffering uploaded CSV.
async fn guard(State(state): State<ApiState>, request: Request, next: Next) -> Response {
    let actor = match actor(&state, request.headers()).await {
        Ok(actor) => actor,
        Err(error) => return error.into_response(),
    };
    let path = request.uri().path();
    let (scope, capability, expensive) = match path {
        "/api/v1/procesos/import" => ("importacion", "editar", true),
        "/api/v1/procesos/export" => ("exportacion", "ver", true),
        _ => ("consulta", "ver", false),
    };
    if let Err(error) = state
        .procesos
        .check_access(actor.id, scope, capability)
        .await
    {
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
    match tokio::time::timeout(Duration::from_secs(60), next.run(request)).await {
        Ok(response) => response,
        Err(_) => ApiError(CoreError::Overloaded).into_response(),
    }
}

async fn status(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
    let actor = actor(&state, &headers).await?;
    state
        .procesos
        .status(actor.id)
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
) -> Result<Json<bastion_procesos::ProcessRecord>, ApiError> {
    let actor = actor(&state, &headers).await?;
    state
        .procesos
        .detail(actor.id, &id)
        .await
        .map(Json)
        .map_err(ApiError)
}

async fn import(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Json(body): Json<ImportRequest>,
) -> Result<Json<bastion_procesos::ImportResult>, ApiError> {
    let actor = actor(&state, &headers).await?;
    state
        .procesos
        .import(actor.id, &body.csv_text)
        .await
        .map(Json)
        .map_err(ApiError)
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
