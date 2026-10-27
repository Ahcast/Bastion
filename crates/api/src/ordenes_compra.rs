use super::*;
use bastion_ordenes_compra::{OrderFilter, OrderInput, OrderPage, PurchaseOrder};

pub(super) fn routes(state: ApiState) -> Router<ApiState> {
    let documents = Router::new()
        .route("/api/v1/ordenes-compra/{id}/documents", get(documents))
        .route(
            "/api/v1/ordenes-compra/{id}/documents/{item}",
            post(upload_document).delete(remove_document),
        )
        .route(
            "/api/v1/ordenes-compra/{id}/documents/{item}/content",
            get(document_content),
        )
        .layer(axum::extract::DefaultBodyLimit::max(14 * 1024 * 1024))
        .layer(RequestBodyLimitLayer::new(14 * 1024 * 1024));
    Router::new()
        .route(
            "/api/v1/ordenes-compra/document-readiness",
            get(document_readiness),
        )
        .route("/api/v1/ordenes-compra/{id}/clone", post(clone_order))
        .route(
            "/api/v1/ordenes-compra/{id}/convert/invoice",
            post(convert_invoice),
        )
        .route("/api/v1/ordenes-compra/dashboard", get(dashboard))
        .route("/api/v1/ordenes-compra", get(list).post(create))
        .route("/api/v1/ordenes-compra/calculate", post(preview))
        .route(
            "/api/v1/ordenes-compra/destinations",
            get(destinations).post(create_destination),
        )
        .route(
            "/api/v1/ordenes-compra/destinations/{id}",
            axum::routing::put(update_destination),
        )
        .route("/api/v1/ordenes-compra/{id}", get(detail).put(replace))
        .layer(RequestBodyLimitLayer::new(1024 * 1024))
        .merge(documents)
        .route_layer(middleware::from_fn_with_state(state, guard))
}

static DOCUMENT_REQUESTS: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(4);
async fn guard(State(state): State<ApiState>, request: Request, next: Next) -> Response {
    let document_operation = request.uri().path().contains("/documents");
    match tokio::time::timeout(
        Duration::from_secs(if document_operation { 60 } else { 30 }),
        async {
            let _permit = if document_operation {
                Some(
                    DOCUMENT_REQUESTS
                        .try_acquire()
                        .map_err(|_| ApiError(CoreError::Overloaded))?,
                )
            } else {
                None
            };
            let user = actor(&state, request.headers()).await?;
            let capability = if request.method() == Method::DELETE {
                "eliminar"
            } else if request.method() == Method::GET || request.method() == Method::HEAD {
                "ver"
            } else {
                "editar"
            };
            let catalog = request.uri().path().contains("/destinations");
            if catalog && capability == "ver" {
                state
                    .ordenes_compra
                    .check_destinations(user.id)
                    .await
                    .map_err(ApiError)?;
            } else {
                state
                    .ordenes_compra
                    .check_access(
                        user.id,
                        if catalog { "destinos" } else { "documentos" },
                        capability,
                    )
                    .await
                    .map_err(ApiError)?;
            }
            state
                .ordenes_compra
                .consume_request(user.id)
                .await
                .map_err(ApiError)?;
            Ok::<_, ApiError>(next.run(request).await)
        },
    )
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
    Query(filter): Query<OrderFilter>,
) -> Result<Json<OrderPage>, ApiError> {
    let user = actor(&state, &headers).await?;
    state
        .ordenes_compra
        .list(user.id, &filter)
        .await
        .map(Json)
        .map_err(ApiError)
}
async fn dashboard(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Query(query): Query<DashboardQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let user = actor(&state, &headers).await?;
    state
        .ordenes_compra
        .dashboard(user.id, query.period)
        .await
        .map(Json)
        .map_err(ApiError)
}
async fn preview(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Json(input): Json<OrderInput>,
) -> Result<Json<bastion_ordenes_compra::OrderCalculation>, ApiError> {
    let user = actor(&state, &headers).await?;
    state
        .ordenes_compra
        .preview(user.id, &input)
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
        .ordenes_compra
        .detail(user.id, id)
        .await
        .map(order_response)
        .map_err(ApiError)
}
async fn create(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Json(input): Json<OrderInput>,
) -> Result<Response, ApiError> {
    let user = actor(&state, &headers).await?;
    let key = single_header(&headers, "idempotency-key")?;
    let result = state
        .ordenes_compra
        .create(user.id, key, input)
        .await
        .map_err(ApiError)?;
    let id = result.id;
    let version = result.version;
    let mut response = Json(result).into_response();
    etag(&mut response, version);
    *response.status_mut() = StatusCode::CREATED;
    response.headers_mut().insert(
        "location",
        HeaderValue::from_str(&format!("/api/v1/ordenes-compra/{id}"))
            .map_err(|_| ApiError(CoreError::Invalid))?,
    );
    Ok(response)
}
async fn replace(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Path(id): Path<u64>,
    Json(input): Json<OrderInput>,
) -> Result<Response, ApiError> {
    let user = actor(&state, &headers).await?;
    let expected =
        version(single_header(&headers, "if-match")?).ok_or(ApiError(CoreError::Invalid))?;
    match state
        .ordenes_compra
        .replace(user.id, id, expected, input)
        .await
    {
        Ok(order) => Ok(order_response(order)),
        Err(CoreError::PreconditionFailed) => Ok(problem(
            StatusCode::PRECONDITION_FAILED,
            "Precondition Failed",
            "El orden cambió. Tus cambios no se guardaron; vuelve a abrir la versión actual.",
        )),
        Err(error) => Err(ApiError(error)),
    }
}
async fn destinations(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
) -> Result<Json<Vec<bastion_ordenes_compra::Destination>>, ApiError> {
    let user = actor(&state, &headers).await?;
    state
        .ordenes_compra
        .destinations(user.id)
        .await
        .map(Json)
        .map_err(ApiError)
}
async fn create_destination(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Json(input): Json<bastion_ordenes_compra::DestinationInput>,
) -> Result<Response, ApiError> {
    let user = actor(&state, &headers).await?;
    let row = state
        .ordenes_compra
        .save_destination(user.id, None, None, input)
        .await
        .map_err(ApiError)?;
    let version = row.version;
    let mut response = Json(row).into_response();
    etag(&mut response, version);
    Ok(response)
}
async fn update_destination(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Path(id): Path<u64>,
    Json(input): Json<bastion_ordenes_compra::DestinationInput>,
) -> Result<Response, ApiError> {
    let user = actor(&state, &headers).await?;
    let expected =
        version(single_header(&headers, "if-match")?).ok_or(ApiError(CoreError::Invalid))?;
    let row = state
        .ordenes_compra
        .save_destination(user.id, Some(id), Some(expected), input)
        .await
        .map_err(ApiError)?;
    let version = row.version;
    let mut response = Json(row).into_response();
    etag(&mut response, version);
    Ok(response)
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
    let value = value.strip_prefix('"')?.strip_suffix('"')?;
    if value.starts_with('0') || !value.bytes().all(|c| c.is_ascii_digit()) {
        return None;
    }
    value.parse::<u64>().ok().filter(|v| *v > 0)
}
fn etag(response: &mut Response, version: u64) {
    response.headers_mut().insert(
        "etag",
        HeaderValue::from_str(&format!("\"{version}\"")).expect("integer ETag"),
    );
}
fn order_response(order: PurchaseOrder) -> Response {
    let version = order.version;
    let mut response = Json(order).into_response();
    etag(&mut response, version);
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_headers_must_be_single_and_strong() {
        assert_eq!(version("\"12\""), Some(12));
        for invalid in [
            "12",
            "*",
            "W/\"12\"",
            "\"0\"",
            "\"01\"",
            "\"-1\"",
            "\"1\",\"2\"",
        ] {
            assert!(version(invalid).is_none());
        }
        let mut headers = axum::http::HeaderMap::new();
        headers.append("if-match", HeaderValue::from_static("\"1\""));
        headers.append("if-match", HeaderValue::from_static("\"2\""));
        assert!(single_header(&headers, "if-match").is_err());
    }

    #[tokio::test]
    async fn mysql57_http_session_csrf_permissions_and_etag() {
        use axum::body::{Body, to_bytes};
        use bastion_core::Database;
        use sqlx::mysql::MySqlPoolOptions;
        use tower::ServiceExt;

        let Ok(url) = std::env::var("BASTION_ORDENES_HTTP_TEST_URL") else {
            eprintln!("Requires isolated BASTION_ORDENES_HTTP_TEST_URL; CI supplies it.");
            return;
        };
        let pool = MySqlPoolOptions::new()
            .max_connections(5)
            .connect(&url)
            .await
            .unwrap();
        let database = Database::from_test_pool(pool.clone());
        database.migrate().await.unwrap();
        bastion_ordenes_compra::OrderService::migrate(&database)
            .await
            .unwrap();
        bastion_cotizaciones::QuoteService::migrate(&database)
            .await
            .unwrap();
        bastion_facturas::InvoiceService::migrate(&database)
            .await
            .unwrap();
        bastion_document_flows::ConversionService::migrate(&database)
            .await
            .unwrap();
        sqlx::query("INSERT INTO persons (id,full_name) VALUES (1,'HTTP Admin'),(2,'HTTP Viewer')")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO roles (id,name) VALUES (2,'HTTP Viewer')")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO users (id,person_id,role_id,email,status) VALUES (1,1,1,'admin@example.test','active'),(2,2,2,'viewer@example.test','active')").execute(&pool).await.unwrap();
        for (id, token) in [(1_u64, "synthetic-admin"), (2, "synthetic-viewer")] {
            sqlx::query("INSERT INTO sessions (user_id,token_hash,csrf_hash,ip_address,started_at,last_activity_at,last_totp_at,expires_at) VALUES (?,UNHEX(SHA2(?,256)),UNHEX(SHA2('synthetic-csrf',256)),'127.0.0.1',UTC_TIMESTAMP(6),UTC_TIMESTAMP(6),UTC_TIMESTAMP(6),UTC_TIMESTAMP(6)+INTERVAL 1 HOUR)")
                .bind(id).bind(token).execute(&pool).await.unwrap();
        }
        let core = CoreService::new(database.clone(), "x".repeat(32), [1; 32]).unwrap();
        let orders = bastion_ordenes_compra::OrderService::new(database, core.admin());
        orders.synchronize_release(true).await.unwrap();
        core.admin()
            .set_module_enabled(1, "ordenes_compra", true)
            .await
            .unwrap();
        let app = router(
            core.clone(),
            "https://example.test".into(),
            vec![],
            std::path::PathBuf::from("unused.csv"),
        )
        .unwrap();
        sqlx::query("INSERT INTO ordenes_destinations (id,destination,name_key) VALUES (1,'Destino sintético','destino sintético')").execute(&pool).await.unwrap();
        let payload = r#"{"n_orders":"OC-HTTP","n_bill":"","order_date":"2026-10-23","received_date":null,"client_name":"Cliente HTTP","client_rtn":"","id_destination":1,"comments":"","products":[],"expenses":[]}"#;
        let request = |method: &str,
                       path: &str,
                       token: Option<&str>,
                       csrf: bool,
                       origin: &str,
                       body: &str| {
            let mut builder = axum::http::Request::builder()
                .method(method)
                .uri(path)
                .header("origin", origin)
                .header("content-type", "application/json");
            if let Some(token) = token {
                builder = builder.header(
                    "cookie",
                    format!("__Host-bastion_session={token}; __Host-bastion_csrf=synthetic-csrf"),
                );
            }
            if csrf {
                builder = builder.header("x-csrf-token", "synthetic-csrf");
            }
            let mut req = builder.body(Body::from(body.to_owned())).unwrap();
            req.extensions_mut()
                .insert(ConnectInfo("127.0.0.1:5010".parse::<SocketAddr>().unwrap()));
            req
        };
        let response = app
            .clone()
            .oneshot(request(
                "GET",
                "/api/v1/ordenes-compra",
                None,
                false,
                "https://example.test",
                "",
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        for (csrf, origin) in [
            (false, "https://example.test"),
            (true, "https://evil.example"),
        ] {
            let response = app
                .clone()
                .oneshot(request(
                    "POST",
                    "/api/v1/ordenes-compra",
                    Some("synthetic-admin"),
                    csrf,
                    origin,
                    payload,
                ))
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::FORBIDDEN);
        }
        let response = app
            .clone()
            .oneshot(request(
                "GET",
                "/api/v1/ordenes-compra",
                Some("synthetic-viewer"),
                false,
                "https://example.test",
                "",
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        sqlx::query("INSERT INTO role_permissions (role_id,module_id,scope_hash,scope_key,capability) VALUES (2,'ordenes_compra',UNHEX(SHA2('documentos',256)),'documentos','ver')").execute(&pool).await.unwrap();
        let mut create = request(
            "POST",
            "/api/v1/ordenes-compra",
            Some("synthetic-admin"),
            true,
            "https://example.test",
            payload,
        );
        create.headers_mut().insert(
            "idempotency-key",
            HeaderValue::from_static("http-order-key-0001"),
        );
        let response = app.clone().oneshot(create).await.unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
        assert_eq!(response.headers()["etag"], "\"1\"");
        assert_eq!(response.headers()["location"], "/api/v1/ordenes-compra/1");
        let value: serde_json::Value =
            serde_json::from_slice(&to_bytes(response.into_body(), 16384).await.unwrap()).unwrap();
        assert_eq!(value["status"], "Pendiente");
        assert_eq!(value["data"]["n_orders"], "OC-HTTP");
        for expected in [StatusCode::OK, StatusCode::PRECONDITION_FAILED] {
            let mut update = request(
                "PUT",
                "/api/v1/ordenes-compra/1",
                Some("synthetic-admin"),
                true,
                "https://example.test",
                payload,
            );
            update
                .headers_mut()
                .insert("if-match", HeaderValue::from_static("\"1\""));
            assert_eq!(
                app.clone().oneshot(update).await.unwrap().status(),
                expected
            );
        }
        let response = app
            .clone()
            .oneshot(request(
                "GET",
                "/api/v1/ordenes-compra/1",
                Some("synthetic-viewer"),
                false,
                "https://example.test",
                "",
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let response = app
            .clone()
            .oneshot(request(
                "POST",
                "/api/v1/ordenes-compra/calculate",
                Some("synthetic-viewer"),
                true,
                "https://example.test",
                payload,
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        let response = app
            .clone()
            .oneshot(request(
                "POST",
                "/api/v1/ordenes-compra",
                Some("synthetic-admin"),
                true,
                "https://example.test",
                &"x".repeat(1024 * 1024 + 1),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
        for (path, method, token, csrf, expected_status) in [
            (
                "/api/v1/ordenes-compra/1/clone",
                "POST",
                "synthetic-admin",
                false,
                StatusCode::FORBIDDEN,
            ),
            (
                "/api/v1/ordenes-compra/1/clone",
                "POST",
                "synthetic-viewer",
                true,
                StatusCode::FORBIDDEN,
            ),
            (
                "/api/v1/ordenes-compra/1/documents",
                "GET",
                "synthetic-viewer",
                false,
                StatusCode::OK,
            ),
            (
                "/api/v1/ordenes-compra/1/convert/invoice",
                "POST",
                "synthetic-admin",
                true,
                StatusCode::NOT_FOUND,
            ),
            (
                "/api/v1/ordenes-compra/1/clone",
                "POST",
                "synthetic-admin",
                true,
                StatusCode::CREATED,
            ),
        ] {
            let mut req = request(method, path, Some(token), csrf, "https://example.test", "");
            req.headers_mut()
                .insert("if-match", HeaderValue::from_static("\"2\""));
            req.headers_mut().insert(
                "idempotency-key",
                HeaderValue::from_static("http-order-actions-0001"),
            );
            assert_eq!(
                app.clone().oneshot(req).await.unwrap().status(),
                expected_status,
                "{method} {path}"
            );
        }
        core.admin()
            .set_module_enabled(1, "ordenes_compra", false)
            .await
            .unwrap();
        let response = app
            .oneshot(request(
                "GET",
                "/api/v1/ordenes-compra/1",
                Some("synthetic-admin"),
                false,
                "https://example.test",
                "",
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
}

async fn clone_order(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Path(id): Path<u64>,
) -> Result<Response, ApiError> {
    let user = actor(&state, &headers).await?;
    let expected =
        version(single_header(&headers, "if-match")?).ok_or(ApiError(CoreError::Invalid))?;
    let order = state
        .ordenes_compra
        .clone_order(
            user.id,
            id,
            expected,
            single_header(&headers, "idempotency-key")?,
        )
        .await
        .map_err(ApiError)?;
    let id = order.id;
    let mut response = order_response(order);
    *response.status_mut() = StatusCode::CREATED;
    response.headers_mut().insert(
        "location",
        HeaderValue::from_str(&format!("/api/v1/ordenes-compra/{id}"))
            .map_err(|_| ApiError(CoreError::Invalid))?,
    );
    Ok(response)
}
async fn convert_invoice(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Path(id): Path<u64>,
) -> Result<Json<bastion_document_flows::OrderInvoiceResult>, ApiError> {
    let user = actor(&state, &headers).await?;
    let expected =
        version(single_header(&headers, "if-match")?).ok_or(ApiError(CoreError::Invalid))?;
    bastion_document_flows::ConversionService::new(&state.core)
        .order_invoice(
            user.id,
            id,
            expected,
            single_header(&headers, "idempotency-key")?,
        )
        .await
        .map(Json)
        .map_err(ApiError)
}
async fn document_readiness() -> Json<serde_json::Value> {
    Json(
        serde_json::json!({"storage":bastion_core::DocumentService::from_environment().storage_configured()}),
    )
}
async fn documents(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Path(id): Path<u64>,
) -> Result<Json<bastion_ordenes_compra::OrderDocuments>, ApiError> {
    let user = actor(&state, &headers).await?;
    state
        .ordenes_compra
        .documents(user.id, id)
        .await
        .map(Json)
        .map_err(ApiError)
}
async fn upload_document(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Path((id, item)): Path<(u64, String)>,
    Json(input): Json<bastion_ordenes_compra::OrderDocumentInput>,
) -> Result<Json<bastion_ordenes_compra::OrderDocumentWrite>, ApiError> {
    let user = actor(&state, &headers).await?;
    let expected =
        version(single_header(&headers, "if-match")?).ok_or(ApiError(CoreError::Invalid))?;
    state
        .ordenes_compra
        .upload_document(
            user.id,
            id,
            &item,
            expected,
            single_header(&headers, "idempotency-key")?,
            input,
        )
        .await
        .map(Json)
        .map_err(ApiError)
}
async fn document_content(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Path((id, item)): Path<(u64, String)>,
) -> Result<Response, ApiError> {
    let user = actor(&state, &headers).await?;
    let content = state
        .ordenes_compra
        .document_content(user.id, id, &item)
        .await
        .map_err(ApiError)?;
    let mut response = Json(content).into_response();
    response
        .headers_mut()
        .insert("cache-control", HeaderValue::from_static("no-store"));
    Ok(response)
}
async fn remove_document(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Path((id, item)): Path<(u64, String)>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let user = actor(&state, &headers).await?;
    let expected =
        version(single_header(&headers, "if-match")?).ok_or(ApiError(CoreError::Invalid))?;
    let version = state
        .ordenes_compra
        .remove_document(user.id, id, &item, expected)
        .await
        .map_err(ApiError)?;
    Ok(Json(serde_json::json!({"order_version":version})))
}
