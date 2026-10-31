use super::*;

pub(super) fn routes() -> Router<ApiState> {
    Router::new()
        .route("/api/v1/settings/backups", get(status).post(command))
        .layer(RequestBodyLimitLayer::new(1024 * 1024))
}

async fn status(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
    let user = actor(&state, &headers).await?;
    state
        .core
        .admin()
        .authorize_backup_request(user.id, None)
        .await
        .map_err(ApiError)?;
    call(serde_json::json!({"action":"status","actor":user.id})).await
}

async fn command(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Json(mut body): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let user = actor(&state, &headers).await?;
    state
        .core
        .admin()
        .authorize_backup_request(user.id, None)
        .await
        .map_err(ApiError)?;
    let map = body.as_object_mut().ok_or(ApiError(CoreError::Invalid))?;
    if map.contains_key("actor") {
        return Err(ApiError(CoreError::Invalid));
    }
    let action = map
        .get("action")
        .and_then(|x| x.as_str())
        .ok_or(ApiError(CoreError::Invalid))?;
    if ![
        "backup",
        "download",
        "prepare_s3",
        "upload_begin",
        "upload_chunk",
        "upload_finish",
        "upload_abort",
        "restore",
        "discard",
    ]
    .contains(&action)
    {
        return Err(ApiError(CoreError::Invalid));
    }
    if action != "upload_chunk" {
        state
            .core
            .admin()
            .authorize_backup_request(user.id, Some(action))
            .await
            .map_err(ApiError)?;
    }
    map.insert("actor".into(), serde_json::json!(user.id));
    call(body).await
}

async fn call(body: serde_json::Value) -> Result<Json<serde_json::Value>, ApiError> {
    tokio::task::spawn_blocking(move || {
        #[cfg(unix)]
        {
            use std::io::{BufRead, BufReader, Read, Write};
            use std::os::unix::net::UnixStream;
            let mut stream = UnixStream::connect("/run/bastion-backup/control.sock")
                .map_err(|_| ApiError(CoreError::Overloaded))?;
            let timeout = Some(Duration::from_secs(10));
            stream
                .set_read_timeout(timeout)
                .map_err(|_| ApiError(CoreError::Overloaded))?;
            stream
                .set_write_timeout(timeout)
                .map_err(|_| ApiError(CoreError::Overloaded))?;
            let mut bytes = serde_json::to_vec(&body).map_err(|_| ApiError(CoreError::Invalid))?;
            bytes.push(b'\n');
            stream
                .write_all(&bytes)
                .map_err(|_| ApiError(CoreError::Overloaded))?;
            let mut answer = String::new();
            BufReader::new(stream.take(1024 * 1024))
                .read_line(&mut answer)
                .map_err(|_| ApiError(CoreError::Overloaded))?;
            if !answer.ends_with('\n') {
                return Err(ApiError(CoreError::Overloaded));
            }
            let response: serde_json::Value =
                serde_json::from_str(&answer).map_err(|_| ApiError(CoreError::Overloaded))?;
            if let Some(error) = response.get("error").and_then(|x| x.as_str()) {
                return Err(ApiError(match error {
                    "invalid" => CoreError::Invalid,
                    "busy" => CoreError::Conflict,
                    _ => CoreError::Overloaded,
                }));
            }
            Ok(Json(response))
        }
        #[cfg(not(unix))]
        {
            let _ = body;
            Err(ApiError(CoreError::Overloaded))
        }
    })
    .await
    .map_err(|_| ApiError(CoreError::Overloaded))?
}
