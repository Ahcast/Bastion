use super::*;
#[cfg(test)]
#[path = "quote_order_tests.rs"]
mod tests;

pub(super) async fn prepare_order(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Path(id): Path<u64>,
) -> Result<Json<bastion_workflows::QuoteOrderSeed>, ApiError> {
    let user = actor(&state, &headers).await?;
    state
        .ordenes_compra
        .check_access(user.id, "documentos", "editar")
        .await
        .map_err(ApiError)?;
    state
        .ordenes_compra
        .consume_request(user.id)
        .await
        .map_err(ApiError)?;
    let date = (chrono::Utc::now() - chrono::Duration::hours(6)).date_naive();
    bastion_workflows::prepare_quote_order(
        &state.cotizaciones,
        &state.ordenes_compra,
        user.id,
        id,
        expected_version(&headers)?,
        date,
    )
    .await
    .map(Json)
    .map_err(ApiError)
}
pub(super) async fn convert_order(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Path(id): Path<u64>,
    Json(input): Json<bastion_ordenes_compra::OrderInput>,
) -> Result<Json<bastion_ordenes_compra::PurchaseOrder>, ApiError> {
    let user = actor(&state, &headers).await?;
    state
        .ordenes_compra
        .check_access(user.id, "documentos", "editar")
        .await
        .map_err(ApiError)?;
    state
        .ordenes_compra
        .consume_request(user.id)
        .await
        .map_err(ApiError)?;
    match bastion_workflows::convert_quote_order(
        &state.cotizaciones,
        &state.ordenes_compra,
        user.id,
        id,
        expected_version(&headers)?,
        single_header(&headers, "idempotency-key")?,
        input,
    )
    .await
    {
        Ok(order) => Ok(Json(order)),
        Err(error) => Err(ApiError(error)),
    }
}
