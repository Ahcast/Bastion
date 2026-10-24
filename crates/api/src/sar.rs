use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use bastion_core::SarSettings;
use serde::Deserialize;

use super::{ApiError, ApiState, actor};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SarUpdate {
    settings: SarSettings,
    version: String,
}

pub(super) async fn get(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let user = actor(&state, &headers).await?;
    state
        .core
        .admin()
        .get_sar_settings(user.id)
        .await
        .map(|value| Json(value).into_response())
        .map_err(ApiError)
}

pub(super) async fn put(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(body): Json<SarUpdate>,
) -> Result<Response, ApiError> {
    let user = actor(&state, &headers).await?;
    match state
        .core
        .admin()
        .set_sar_settings(user.id, body.settings, &body.version)
        .await
    {
        Ok(value) => Ok(Json(value).into_response()),
        Err(bastion_core::CoreError::PreconditionFailed) => Ok(super::problem(
            StatusCode::PRECONDITION_FAILED,
            "Precondition Failed",
            "La configuración SAR cambió. Recarga la versión actual antes de guardar.",
        )),
        Err(error) => Err(ApiError(error)),
    }
}

