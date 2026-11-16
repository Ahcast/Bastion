use super::*;
use bastion_contabilidad::*;

pub(super) fn routes(state: ApiState) -> Router<ApiState> {
    Router::new()
        .route(
            "/api/v1/contabilidad/cuentas",
            get(accounts).post(create_account),
        )
        .route(
            "/api/v1/contabilidad/cuentas/{code}",
            axum::routing::put(update_account),
        )
        .route(
            "/api/v1/contabilidad/periodos",
            get(periods).post(open_period),
        )
        .route(
            "/api/v1/contabilidad/periodos/{month}",
            axum::routing::put(update_period),
        )
        .route(
            "/api/v1/contabilidad/asientos",
            get(entries).post(create_entry),
        )
        .route(
            "/api/v1/contabilidad/asientos/{id}",
            get(entry).put(update_entry),
        )
        .route("/api/v1/contabilidad/asientos/{id}/{action}", post(act))
        .route("/api/v1/contabilidad/origen/{module}/{id}", get(proposal))
        .route("/api/v1/contabilidad/balance", get(balance))
        .layer(RequestBodyLimitLayer::new(64 * 1024))
        .route_layer(middleware::from_fn_with_state(state, guard))
}
async fn guard(State(state): State<ApiState>, request: Request, next: Next) -> Response {
    match tokio::time::timeout(Duration::from_secs(60), async {
        let user = actor(&state, request.headers()).await?;
        let capability = if request.method() == Method::GET || request.method() == Method::HEAD {
            "ver"
        } else if request.uri().path().ends_with("/reversar")
            || request.uri().path().ends_with("/cancelar")
        {
            "eliminar"
        } else {
            "editar"
        };
        state
            .contabilidad
            .check_access(user.id, capability)
            .await
            .map_err(ApiError)?;
        state
            .contabilidad
            .consume_request(user.id)
            .await
            .map_err(ApiError)?;
        Ok::<_, ApiError>(next.run(request).await)
    })
    .await
    {
        Ok(Ok(r)) => r,
        Ok(Err(e)) => e.into_response(),
        Err(_) => ApiError(CoreError::Overloaded).into_response(),
    }
}
fn version(headers: &axum::http::HeaderMap) -> Result<u64, ApiError> {
    let values = headers.get_all("if-match");
    if values.iter().count() != 1 {
        return Err(ApiError(CoreError::PreconditionFailed));
    }
    let value = values
        .iter()
        .next()
        .and_then(|v| v.to_str().ok())
        .ok_or(ApiError(CoreError::PreconditionFailed))?;
    value
        .strip_prefix('"')
        .and_then(|v| v.strip_suffix('"'))
        .filter(|v| !v.is_empty() && v.bytes().all(|b| b.is_ascii_digit()))
        .and_then(|v| v.parse().ok())
        .filter(|v| *v > 0)
        .ok_or(ApiError(CoreError::PreconditionFailed))
}
async fn accounts(
    State(s): State<ApiState>,
    h: axum::http::HeaderMap,
) -> Result<Json<Vec<Account>>, ApiError> {
    Ok(Json(
        s.contabilidad
            .accounts(actor(&s, &h).await?.id)
            .await
            .map_err(ApiError)?,
    ))
}
async fn create_account(
    State(s): State<ApiState>,
    h: axum::http::HeaderMap,
    Json(body): Json<AccountInput>,
) -> Result<Json<Account>, ApiError> {
    Ok(Json(
        s.contabilidad
            .create_account(actor(&s, &h).await?.id, body)
            .await
            .map_err(ApiError)?,
    ))
}
async fn update_account(
    State(s): State<ApiState>,
    h: axum::http::HeaderMap,
    Path(code): Path<String>,
    Json(body): Json<AccountUpdate>,
) -> Result<Json<Account>, ApiError> {
    Ok(Json(
        s.contabilidad
            .update_account(actor(&s, &h).await?.id, &code, version(&h)?, body)
            .await
            .map_err(ApiError)?,
    ))
}
async fn periods(
    State(s): State<ApiState>,
    h: axum::http::HeaderMap,
) -> Result<Json<Vec<Period>>, ApiError> {
    Ok(Json(
        s.contabilidad
            .periods(actor(&s, &h).await?.id)
            .await
            .map_err(ApiError)?,
    ))
}
async fn open_period(
    State(s): State<ApiState>,
    h: axum::http::HeaderMap,
    Json(body): Json<PeriodInput>,
) -> Result<Json<Period>, ApiError> {
    Ok(Json(
        s.contabilidad
            .open_period(actor(&s, &h).await?.id, &body.month)
            .await
            .map_err(ApiError)?,
    ))
}
async fn update_period(
    State(s): State<ApiState>,
    h: axum::http::HeaderMap,
    Path(month): Path<String>,
    Json(body): Json<PeriodUpdate>,
) -> Result<Json<Period>, ApiError> {
    Ok(Json(
        s.contabilidad
            .update_period(actor(&s, &h).await?.id, &month, version(&h)?, body)
            .await
            .map_err(ApiError)?,
    ))
}
async fn entries(
    State(s): State<ApiState>,
    h: axum::http::HeaderMap,
    Query(filter): Query<EntryFilter>,
) -> Result<Json<EntryPage>, ApiError> {
    Ok(Json(
        s.contabilidad
            .list(actor(&s, &h).await?.id, &filter)
            .await
            .map_err(ApiError)?,
    ))
}
async fn entry(
    State(s): State<ApiState>,
    h: axum::http::HeaderMap,
    Path(id): Path<u64>,
) -> Result<Json<Entry>, ApiError> {
    Ok(Json(
        s.contabilidad
            .detail(actor(&s, &h).await?.id, id)
            .await
            .map_err(ApiError)?,
    ))
}
async fn create_entry(
    State(s): State<ApiState>,
    h: axum::http::HeaderMap,
    Json(body): Json<EntryInput>,
) -> Result<Json<Entry>, ApiError> {
    let key = h
        .get("idempotency-key")
        .and_then(|v| v.to_str().ok())
        .ok_or(ApiError(CoreError::Invalid))?;
    Ok(Json(
        s.contabilidad
            .create(actor(&s, &h).await?.id, key, body)
            .await
            .map_err(ApiError)?,
    ))
}
async fn update_entry(
    State(s): State<ApiState>,
    h: axum::http::HeaderMap,
    Path(id): Path<u64>,
    Json(body): Json<EntryInput>,
) -> Result<Json<Entry>, ApiError> {
    Ok(Json(
        s.contabilidad
            .replace(actor(&s, &h).await?.id, id, version(&h)?, body)
            .await
            .map_err(ApiError)?,
    ))
}
async fn act(
    State(s): State<ApiState>,
    h: axum::http::HeaderMap,
    Path((id, action)): Path<(u64, String)>,
    Json(body): Json<EntryAction>,
) -> Result<Json<Entry>, ApiError> {
    Ok(Json(
        s.contabilidad
            .act(actor(&s, &h).await?.id, id, version(&h)?, &action, body)
            .await
            .map_err(ApiError)?,
    ))
}
async fn proposal(
    State(s): State<ApiState>,
    h: axum::http::HeaderMap,
    Path((module, id)): Path<(String, u64)>,
) -> Result<Json<Proposal>, ApiError> {
    Ok(Json(
        s.contabilidad
            .proposal(actor(&s, &h).await?.id, &module, id)
            .await
            .map_err(ApiError)?,
    ))
}
async fn balance(
    State(s): State<ApiState>,
    h: axum::http::HeaderMap,
    Query(filter): Query<BalanceFilter>,
) -> Result<Json<Balance>, ApiError> {
    Ok(Json(
        s.contabilidad
            .balance(actor(&s, &h).await?.id, &filter)
            .await
            .map_err(ApiError)?,
    ))
}
