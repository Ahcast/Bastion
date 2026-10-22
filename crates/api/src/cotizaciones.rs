use super::*;
use bastion_cotizaciones::{QuoteDraft, QuoteFilter, QuoteInput, QuotePage};

pub(super) fn routes(state: ApiState) -> Router<ApiState> {
    Router::new()
        .route("/api/v1/cotizaciones", get(list).post(create))
        .route("/api/v1/cotizaciones/calculate", post(preview))
        .route("/api/v1/cotizaciones/{id}", get(detail).put(replace))
        .layer(RequestBodyLimitLayer::new(GENERAL_REQUEST_BODY_LIMIT))
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

async fn guard(State(state): State<ApiState>, request: Request, next: Next) -> Response {
    let result = tokio::time::timeout(Duration::from_secs(30), async {
        let user = actor(&state, request.headers()).await?;
        let capability = if request.method() == Method::GET || request.method() == Method::HEAD {
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
    })
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
}
