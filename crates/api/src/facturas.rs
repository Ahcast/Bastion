use super::*;
use bastion_facturas::{InvoiceDraft, InvoiceFilter, InvoiceInput, InvoicePage};

pub(super) fn routes(state: ApiState) -> Router<ApiState> {
    Router::new()
        .route("/api/v1/facturas", get(list).post(create))
        .route("/api/v1/facturas/calculate", post(preview))
        .route("/api/v1/facturas/{id}", get(detail).put(replace))
        .layer(RequestBodyLimitLayer::new(1024 * 1024))
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
            .facturas
            .check_access(user.id, capability)
            .await
            .map_err(ApiError)?;
        state
            .facturas
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
    Query(filter): Query<InvoiceFilter>,
) -> Result<Json<InvoicePage>, ApiError> {
    let user = actor(&state, &headers).await?;
    state
        .facturas
        .list(user.id, &filter)
        .await
        .map(Json)
        .map_err(ApiError)
}
async fn preview(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Json(input): Json<InvoiceInput>,
) -> Result<Json<bastion_facturas::InvoiceCalculation>, ApiError> {
    let user = actor(&state, &headers).await?;
    state
        .facturas
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
        .facturas
        .detail(user.id, id)
        .await
        .map(invoice_response)
        .map_err(ApiError)
}
async fn create(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Json(input): Json<InvoiceInput>,
) -> Result<Response, ApiError> {
    let user = actor(&state, &headers).await?;
    let key = single_header(&headers, "idempotency-key")?;
    let result = state
        .facturas
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
        HeaderValue::from_str(&format!("/api/v1/facturas/{id}"))
            .map_err(|_| ApiError(CoreError::Invalid))?,
    );
    Ok(response)
}
async fn replace(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Path(id): Path<u64>,
    Json(input): Json<InvoiceInput>,
) -> Result<Response, ApiError> {
    let user = actor(&state, &headers).await?;
    let expected =
        version(single_header(&headers, "if-match")?).ok_or(ApiError(CoreError::Invalid))?;
    match state.facturas.replace(user.id, id, expected, input).await {
        Ok(invoice) => Ok(invoice_response(invoice)),
        Err(CoreError::PreconditionFailed) => Ok(problem(
            StatusCode::PRECONDITION_FAILED,
            "Precondition Failed",
            "El borrador cambió. Tus cambios no se guardaron; vuelve a abrir la versión actual.",
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
fn invoice_response(invoice: InvoiceDraft) -> Response {
    let version = invoice.version;
    let mut response = Json(invoice).into_response();
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

        let Ok(url) = std::env::var("BASTION_FACTURAS_HTTP_TEST_URL") else {
            eprintln!("Requires isolated BASTION_FACTURAS_HTTP_TEST_URL; CI supplies it.");
            return;
        };
        let pool = MySqlPoolOptions::new()
            .max_connections(5)
            .connect(&url)
            .await
            .unwrap();
        let database = Database::from_test_pool(pool.clone());
        database.migrate().await.unwrap();
        bastion_facturas::InvoiceService::migrate(&database)
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
        let invoices = bastion_facturas::InvoiceService::new(database, core.admin());
        invoices.synchronize_release(true).await.unwrap();
        core.admin()
            .set_module_enabled(1, "facturas", true)
            .await
            .unwrap();
        let app = router(
            core.clone(),
            "https://example.test".into(),
            vec![],
            std::path::PathBuf::from("unused.csv"),
        )
        .unwrap();
        let payload = r#"{"client_name":"Cliente HTTP","order_number":"","invoice_date":"2026-10-22","payment_term_type":"credito","credit_days":30,"discount_type":"amount","discount_value":"0","notes":"","public_notes":"","items":[],"expenses":[]}"#;
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
                "/api/v1/facturas",
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
                    "/api/v1/facturas",
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
                "/api/v1/facturas",
                Some("synthetic-viewer"),
                false,
                "https://example.test",
                "",
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        sqlx::query("INSERT INTO role_permissions (role_id,module_id,scope_hash,scope_key,capability) VALUES (2,'facturas',UNHEX(SHA2('documentos',256)),'documentos','ver')").execute(&pool).await.unwrap();
        let mut create = request(
            "POST",
            "/api/v1/facturas",
            Some("synthetic-admin"),
            true,
            "https://example.test",
            payload,
        );
        create.headers_mut().insert(
            "idempotency-key",
            HeaderValue::from_static("http-invoice-key-0001"),
        );
        let response = app.clone().oneshot(create).await.unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
        assert_eq!(response.headers()["etag"], "\"1\"");
        assert_eq!(response.headers()["location"], "/api/v1/facturas/1");
        let value: serde_json::Value =
            serde_json::from_slice(&to_bytes(response.into_body(), 16384).await.unwrap()).unwrap();
        assert_eq!(value["status"], "Borrador");
        assert!(value["invoice_number"].is_null());
        for expected in [StatusCode::OK, StatusCode::PRECONDITION_FAILED] {
            let mut update = request(
                "PUT",
                "/api/v1/facturas/1",
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
                "/api/v1/facturas/1",
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
                "/api/v1/facturas/calculate",
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
                "/api/v1/facturas",
                Some("synthetic-admin"),
                true,
                "https://example.test",
                &"x".repeat(1024 * 1024 + 1),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
        core.admin()
            .set_module_enabled(1, "facturas", false)
            .await
            .unwrap();
        let response = app
            .oneshot(request(
                "GET",
                "/api/v1/facturas/1",
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
