use super::*;
use bastion_mcp::parse_authorization_parameters;

fn connection_status(error: bastion_mcp::StoreError) -> StatusCode {
    match error {
        bastion_mcp::StoreError::Forbidden => StatusCode::FORBIDDEN,
        bastion_mcp::StoreError::NotFound => StatusCode::NOT_FOUND,
        bastion_mcp::StoreError::InvalidScope => StatusCode::BAD_REQUEST,
        bastion_mcp::StoreError::Database(_) => StatusCode::SERVICE_UNAVAILABLE,
    }
}

pub(super) async fn connections<F: ClientMetadataSource>(
    State(state): State<DraftClientRequestState<F>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let session = match browser_session(&state.core, &state.origin, &headers).await {
        Ok(value) => value,
        Err(status) => return browser_error(status),
    };
    let input: List = match serde_json::from_slice(&body) {
        Ok(value) => value,
        Err(_) => return error(StatusCode::BAD_REQUEST),
    };
    match bastion_mcp::MysqlOauthStore::new(state.pool)
        .connections_for_session(&CoreConsentPolicy, &session, input.before)
        .await
    {
        Ok(value) => response(StatusCode::OK, json!(value)),
        Err(reason) => error(connection_status(reason)),
    }
}

pub(super) async fn disconnect<F: ClientMetadataSource>(
    State(state): State<DraftClientRequestState<F>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let session = match browser_session(&state.core, &state.origin, &headers).await {
        Ok(value) => value,
        Err(status) => return browser_error(status),
    };
    let input: DisableManaged = match serde_json::from_slice(&body) {
        Ok(value) => value,
        Err(_) => return error(StatusCode::BAD_REQUEST),
    };
    match bastion_mcp::MysqlOauthStore::new(state.pool)
        .disconnect_for_session(
            &CoreConsentPolicy,
            &session,
            input.id,
            &input.confirmed_client_id,
        )
        .await
    {
        Ok(()) => response(StatusCode::OK, json!({"status":"disconnected"})),
        Err(reason) => error(connection_status(reason)),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Inspect {
    authorization_query: String,
}

pub(super) async fn inspect<F: ClientMetadataSource>(
    State(state): State<DraftClientRequestState<F>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let session = match browser_session(&state.core, &state.origin, &headers).await {
        Ok(value) => value,
        Err(status) => return browser_error(status),
    };
    let input: Inspect = match serde_json::from_slice(&body) {
        Ok(value) => value,
        Err(_) => return error(StatusCode::BAD_REQUEST),
    };
    let Some(resource) = state.resource.as_deref() else {
        return error(StatusCode::SERVICE_UNAVAILABLE);
    };
    let request = match parse_authorization_parameters(&input.authorization_query, resource) {
        Ok(value) => value,
        Err(_) => {
            return diagnostic(
                StatusCode::BAD_REQUEST,
                "invalid_request",
                "El cliente debe iniciar una solicitud OAuth nueva con PKCE S256, el recurso de Bastion y permisos explícitos.",
            );
        }
    };
    let registered = approved_read_requirements().into_keys().collect();
    if !request.scopes.is_subset(&registered) {
        return diagnostic(
            StatusCode::BAD_REQUEST,
            "invalid_scope",
            "El cliente solicita permisos que Bastion no ofrece. Revisa su configuración y vuelve a iniciar la conexión.",
        );
    }
    let store = ClientRequestStore::new(state.pool.clone());
    if let Err(reason) = store
        .authorize_session(&CoreConsentPolicy, &session, false)
        .await
    {
        return error(status(reason));
    }
    let Ok(_slot) = state.slots.clone().try_acquire_owned() else {
        return error(StatusCode::TOO_MANY_REQUESTS);
    };
    let metadata = match state.fetcher.fetch(&request.client_id).await {
        Ok(value) => value,
        Err(_) => {
            return diagnostic(
                StatusCode::BAD_REQUEST,
                "client_metadata_unavailable",
                "No se pudo verificar el documento público de identidad del cliente. Debe usar HTTPS público y metadatos CIMD válidos. Intenta de nuevo o revisa la configuración del cliente.",
            );
        }
    };
    if metadata.client_id() != request.client_id
        || !metadata.redirect_uris().contains(&request.redirect_uri)
    {
        return diagnostic(
            StatusCode::BAD_REQUEST,
            "redirect_uri_mismatch",
            "La dirección de retorno solicitada no coincide exactamente con las direcciones publicadas por el cliente. Corrige su configuración antes de autorizarlo.",
        );
    }
    if store.observe_approved_metadata(&metadata).await.is_err() {
        return error(StatusCode::SERVICE_UNAVAILABLE);
    }
    let approved =
        match crate::authorization_http::approved(&state.pool, &request, Some(&metadata)).await {
            Ok(value) => value,
            Err(_) => return error(StatusCode::SERVICE_UNAVAILABLE),
        };
    response(
        StatusCode::OK,
        json!({
            "status":if approved {"ready"} else {"approval_required"},
            "client_id":metadata.client_id(),
            "client_name":metadata.name(),
            "scopes":request.scopes,
            "redirect_uri":request.redirect_uri
        }),
    )
}
