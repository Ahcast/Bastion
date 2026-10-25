use super::*;
use bastion_gastos::{Expense, ExpenseFilter, ExpenseInput, ExpensePage};

pub(super) fn routes(state: ApiState) -> Router<ApiState> {
    Router::new()
        .route("/api/v1/gastos", get(list).post(create))
        .route(
            "/api/v1/gastos/{id}",
            get(detail).put(replace).delete(remove),
        )
        .layer(RequestBodyLimitLayer::new(16 * 1024))
        .route_layer(middleware::from_fn_with_state(state, guard))
}

async fn guard(State(state): State<ApiState>, request: Request, next: Next) -> Response {
    match tokio::time::timeout(Duration::from_secs(30), async {
        let user = actor(&state, request.headers()).await?;
        let capability = if request.method() == Method::GET || request.method() == Method::HEAD {
            "ver"
        } else if request.method() == Method::DELETE {
            "eliminar"
        } else {
            "editar"
        };
        state
            .gastos
            .check_access(user.id, capability)
            .await
            .map_err(ApiError)?;
        state
            .gastos
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
    Query(filter): Query<ExpenseFilter>,
) -> Result<Json<ExpensePage>, ApiError> {
    let user = actor(&state, &headers).await?;
    state
        .gastos
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
        .gastos
        .detail(user.id, id)
        .await
        .map(expense_response)
        .map_err(ApiError)
}

async fn create(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Json(input): Json<ExpenseInput>,
) -> Result<Response, ApiError> {
    let user = actor(&state, &headers).await?;
    let key = single_header(&headers, "idempotency-key")?;
    let expense = state
        .gastos
        .create(user.id, key, input)
        .await
        .map_err(ApiError)?;
    let id = expense.id;
    let version = expense.version;
    let mut response = Json(expense).into_response();
    etag(&mut response, version);
    *response.status_mut() = StatusCode::CREATED;
    response.headers_mut().insert(
        "location",
        HeaderValue::from_str(&format!("/api/v1/gastos/{id}"))
            .map_err(|_| ApiError(CoreError::Invalid))?,
    );
    Ok(response)
}

async fn replace(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Path(id): Path<u64>,
    Json(input): Json<ExpenseInput>,
) -> Result<Response, ApiError> {
    let user = actor(&state, &headers).await?;
    let expected =
        version(single_header(&headers, "if-match")?).ok_or(ApiError(CoreError::Invalid))?;
    match state.gastos.replace(user.id, id, expected, input).await {
        Ok(expense) => Ok(expense_response(expense)),
        Err(CoreError::PreconditionFailed) => Ok(problem(
            StatusCode::PRECONDITION_FAILED,
            "Precondition Failed",
            "El gasto cambió. Tus cambios no se guardaron; vuelve a abrir la versión actual.",
        )),
        Err(error) => Err(ApiError(error)),
    }
}

async fn remove(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Path(id): Path<u64>,
) -> Result<StatusCode, ApiError> {
    let user = actor(&state, &headers).await?;
    state.gastos.delete(user.id, id).await.map_err(ApiError)?;
    Ok(StatusCode::NO_CONTENT)
}

fn single_header<'a>(headers: &'a axum::http::HeaderMap, name: &str) -> Result<&'a str, ApiError> {
    if headers.get_all(name).iter().count() != 1 {
        return Err(ApiError(CoreError::Invalid));
    }
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .ok_or(ApiError(CoreError::Invalid))
}

fn version(value: &str) -> Option<u64> {
    let value = value.strip_prefix('"')?.strip_suffix('"')?;
    if value.starts_with('0') || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    value.parse::<u64>().ok().filter(|version| *version > 0)
}

fn etag(response: &mut Response, version: u64) {
    response.headers_mut().insert(
        "etag",
        HeaderValue::from_str(&format!("\"{version}\"")).expect("integer ETag"),
    );
}

fn expense_response(expense: Expense) -> Response {
    let version = expense.version;
    let mut response = Json(expense).into_response();
    etag(&mut response, version);
    response
}
