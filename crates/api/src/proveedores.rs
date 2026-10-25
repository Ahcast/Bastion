use super::*;
use bastion_proveedores::{Supplier, SupplierFilter, SupplierInput, SupplierPage};

pub(super) fn routes(state: ApiState) -> Router<ApiState> {
    Router::new()
        .route("/api/v1/proveedores", get(list).post(create))
        .route("/api/v1/proveedores/catalogo", get(catalog))
        .route("/api/v1/proveedores/{id}", get(detail).put(replace))
        .layer(RequestBodyLimitLayer::new(16 * 1024))
        .route_layer(middleware::from_fn_with_state(state, guard))
}

async fn guard(State(state): State<ApiState>, request: Request, next: Next) -> Response {
    match tokio::time::timeout(Duration::from_secs(30), async {
        let user = actor(&state, request.headers()).await?;
        let capability = if request.method() == Method::GET || request.method() == Method::HEAD {
            "ver"
        } else {
            "editar"
        };
        state
            .proveedores
            .check_access(user.id, capability)
            .await
            .map_err(ApiError)?;
        state
            .proveedores
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
    Query(filter): Query<SupplierFilter>,
) -> Result<Json<SupplierPage>, ApiError> {
    let user = actor(&state, &headers).await?;
    state
        .proveedores
        .list(user.id, &filter)
        .await
        .map(Json)
        .map_err(ApiError)
}

async fn catalog(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Query(filter): Query<SupplierFilter>,
) -> Result<Json<bastion_proveedores::SupplierPage<bastion_proveedores::CatalogSupplier>>, ApiError>
{
    let user = actor(&state, &headers).await?;
    state
        .proveedores
        .catalog(user.id, &filter)
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
        .proveedores
        .detail(user.id, id)
        .await
        .map(supplier_response)
        .map_err(ApiError)
}

async fn create(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Json(input): Json<SupplierInput>,
) -> Result<Response, ApiError> {
    let user = actor(&state, &headers).await?;
    let key = single_header(&headers, "idempotency-key")?;
    let supplier = state
        .proveedores
        .create(user.id, key, input)
        .await
        .map_err(ApiError)?;
    let id = supplier.id;
    let version = supplier.version;
    let mut response = Json(supplier).into_response();
    etag(&mut response, version);
    *response.status_mut() = StatusCode::CREATED;
    response.headers_mut().insert(
        "location",
        HeaderValue::from_str(&format!("/api/v1/proveedores/{id}"))
            .map_err(|_| ApiError(CoreError::Invalid))?,
    );
    Ok(response)
}

async fn replace(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Path(id): Path<u64>,
    Json(input): Json<SupplierInput>,
) -> Result<Response, ApiError> {
    let user = actor(&state, &headers).await?;
    let expected =
        version(single_header(&headers, "if-match")?).ok_or(ApiError(CoreError::Invalid))?;
    match state
        .proveedores
        .replace(user.id, id, expected, input)
        .await
    {
        Ok(supplier) => Ok(supplier_response(supplier)),
        Err(CoreError::PreconditionFailed) => Ok(problem(
            StatusCode::PRECONDITION_FAILED,
            "Precondition Failed",
            "El proveedor cambió. Tus cambios no se guardaron; vuelve a abrir la versión actual.",
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

fn supplier_response(supplier: Supplier) -> Response {
    let version = supplier.version;
    let mut response = Json(supplier).into_response();
    etag(&mut response, version);
    response
}
