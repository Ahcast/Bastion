use super::*;
use bastion_clientes::{Client, ClientFilter, ClientInput, ClientPage};

pub(super) fn routes(state: ApiState) -> Router<ApiState> {
    Router::new()
        .route("/api/v1/clientes", get(list).post(create))
        .route("/api/v1/clientes/catalogo", get(catalog))
        .route("/api/v1/clientes/{id}", get(detail).put(replace))
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
            .clientes
            .check_access(user.id, capability)
            .await
            .map_err(ApiError)?;
        state
            .clientes
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
    Query(filter): Query<ClientFilter>,
) -> Result<Json<ClientPage>, ApiError> {
    let user = actor(&state, &headers).await?;
    state
        .clientes
        .list(user.id, &filter)
        .await
        .map(Json)
        .map_err(ApiError)
}
async fn catalog(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Query(filter): Query<ClientFilter>,
) -> Result<Json<bastion_clientes::ClientPage<bastion_clientes::CatalogClient>>, ApiError> {
    let user = actor(&state, &headers).await?;
    state
        .clientes
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
        .clientes
        .detail(user.id, id)
        .await
        .map(client_response)
        .map_err(ApiError)
}
async fn create(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Json(input): Json<ClientInput>,
) -> Result<Response, ApiError> {
    let user = actor(&state, &headers).await?;
    let key = single_header(&headers, "idempotency-key")?;
    let result = state
        .clientes
        .create(user.id, key, input)
        .await
        .map_err(ApiError)?;
    let id = result.client.id;
    let version = result.client.version;
    let created = !result.merged;
    let mut response = Json(result).into_response();
    etag(&mut response, version);
    if created {
        *response.status_mut() = StatusCode::CREATED;
    }
    response.headers_mut().insert(
        "location",
        HeaderValue::from_str(&format!("/api/v1/clientes/{id}"))
            .map_err(|_| ApiError(CoreError::Invalid))?,
    );
    Ok(response)
}
async fn replace(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Path(id): Path<u64>,
    Json(input): Json<ClientInput>,
) -> Result<Response, ApiError> {
    let user = actor(&state, &headers).await?;
    let expected =
        version(single_header(&headers, "if-match")?).ok_or(ApiError(CoreError::Invalid))?;
    match state.clientes.replace(user.id, id, expected, input).await {
        Ok(client) => Ok(client_response(client)),
        Err(CoreError::PreconditionFailed) => Ok(problem(
            StatusCode::PRECONDITION_FAILED,
            "Precondition Failed",
            "El cliente cambió. Tus cambios no se guardaron; vuelve a abrir la versión actual.",
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
fn client_response(client: Client) -> Response {
    let version = client.version;
    let mut response = Json(client).into_response();
    etag(&mut response, version);
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn edits_require_a_single_strong_version() {
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
    async fn mysql57_http_session_csrf_permissions_etag_and_limits() {
        use axum::body::{Body, to_bytes};
        use bastion_core::Database;
        use sqlx::mysql::MySqlPoolOptions;
        use tower::ServiceExt;

        let Ok(url) = std::env::var("BASTION_CLIENTES_HTTP_TEST_URL") else {
            eprintln!("Requires isolated BASTION_CLIENTES_HTTP_TEST_URL; CI supplies it.");
            return;
        };
        let pool = MySqlPoolOptions::new()
            .max_connections(5)
            .connect(&url)
            .await
            .unwrap();
        let database = Database::from_test_pool(pool.clone());
        database.migrate().await.unwrap();
        bastion_clientes::ClientService::migrate(&database)
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
        // Synthetic sessions exist only in the disposable CI database.
        for (user, token) in [
            (1_u64, "synthetic-admin-session"),
            (2, "synthetic-viewer-session"),
        ] {
            sqlx::query("INSERT INTO sessions (user_id,token_hash,csrf_hash,ip_address,started_at,last_activity_at,last_totp_at,expires_at) VALUES (?,UNHEX(SHA2(?,256)),UNHEX(SHA2('synthetic-csrf',256)),'127.0.0.1',UTC_TIMESTAMP(6),UTC_TIMESTAMP(6),UTC_TIMESTAMP(6),UTC_TIMESTAMP(6)+INTERVAL 1 HOUR)")
                .bind(user).bind(token).execute(&pool).await.unwrap();
        }
        let core = CoreService::new(database.clone(), "x".repeat(32), [1; 32]).unwrap();
        let clients = bastion_clientes::ClientService::new(database, core.admin());
        clients.synchronize_release(true).await.unwrap();
        core.admin()
            .set_module_enabled(1, "clientes", true)
            .await
            .unwrap();
        let app = super::super::router(
            core.clone(),
            "https://example.test".into(),
            Vec::new(),
            "/unused".into(),
        )
        .unwrap();
        let request = |method: &str,
                       path: &str,
                       session: Option<&str>,
                       csrf: bool,
                       origin: &str,
                       body: &str| {
            let mut builder = axum::http::Request::builder()
                .method(method)
                .uri(path)
                .header("origin", origin)
                .header("content-type", "application/json");
            if let Some(token) = session {
                builder = builder.header(
                    "cookie",
                    format!("__Host-bastion_session={token}; __Host-bastion_csrf=synthetic-csrf"),
                );
            }
            if csrf {
                builder = builder.header("x-csrf-token", "synthetic-csrf");
            }
            let mut request = builder.body(Body::from(body.to_owned())).unwrap();
            request
                .extensions_mut()
                .insert(ConnectInfo("127.0.0.1:5010".parse::<SocketAddr>().unwrap()));
            request
        };
        let payload = r#"{"cliente_name":"Cliente HTTP","email":"test@example.test"}"#;
        let anonymous = app
            .clone()
            .oneshot(request(
                "GET",
                "/api/v1/clientes",
                None,
                false,
                "https://example.test",
                "",
            ))
            .await
            .unwrap();
        assert_eq!(anonymous.status(), StatusCode::UNAUTHORIZED);
        for (csrf, origin) in [
            (false, "https://example.test"),
            (true, "https://evil.example"),
        ] {
            let response = app
                .clone()
                .oneshot(request(
                    "POST",
                    "/api/v1/clientes",
                    Some("synthetic-admin-session"),
                    csrf,
                    origin,
                    payload,
                ))
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::FORBIDDEN);
        }
        let viewer = app
            .clone()
            .oneshot(request(
                "GET",
                "/api/v1/clientes",
                Some("synthetic-viewer-session"),
                false,
                "https://example.test",
                "",
            ))
            .await
            .unwrap();
        assert_eq!(viewer.status(), StatusCode::FORBIDDEN);
        sqlx::query("INSERT INTO role_permissions (role_id,module_id,scope_hash,scope_key,capability) VALUES (2,'clientes',UNHEX(SHA2('catalogo',256)),'catalogo','ver')").execute(&pool).await.unwrap();
        let mut create = request(
            "POST",
            "/api/v1/clientes",
            Some("synthetic-admin-session"),
            true,
            "https://example.test",
            payload,
        );
        create.headers_mut().insert(
            "idempotency-key",
            HeaderValue::from_static("http-client-create-key"),
        );
        let response = app.clone().oneshot(create).await.unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
        assert_eq!(response.headers()["etag"], "\"1\"");
        assert_eq!(response.headers()["location"], "/api/v1/clientes/1");
        let result: serde_json::Value =
            serde_json::from_slice(&to_bytes(response.into_body(), 16384).await.unwrap()).unwrap();
        assert_eq!(result["client"]["cliente_name"], "Cliente HTTP");
        for status in [StatusCode::OK, StatusCode::PRECONDITION_FAILED] {
            let mut update = request(
                "PUT",
                "/api/v1/clientes/1",
                Some("synthetic-admin-session"),
                true,
                "https://example.test",
                payload,
            );
            update
                .headers_mut()
                .insert("if-match", HeaderValue::from_static("\"1\""));
            let response = app.clone().oneshot(update).await.unwrap();
            assert_eq!(response.status(), status);
            let current: u64 =
                sqlx::query_scalar("SELECT version FROM clientes_records WHERE id=1")
                    .fetch_one(&pool)
                    .await
                    .unwrap();
            assert_eq!(current, 2);
        }
        let catalog = app
            .clone()
            .oneshot(request(
                "GET",
                "/api/v1/clientes/catalogo",
                Some("synthetic-viewer-session"),
                false,
                "https://example.test",
                "",
            ))
            .await
            .unwrap();
        assert_eq!(catalog.status(), StatusCode::OK);
        let catalog: serde_json::Value =
            serde_json::from_slice(&to_bytes(catalog.into_body(), 16384).await.unwrap()).unwrap();
        assert!(catalog["items"][0].get("direction").is_none());
        let oversized = app
            .clone()
            .oneshot(request(
                "POST",
                "/api/v1/clientes",
                Some("synthetic-admin-session"),
                true,
                "https://example.test",
                &"x".repeat(17 * 1024),
            ))
            .await
            .unwrap();
        assert_eq!(oversized.status(), StatusCode::PAYLOAD_TOO_LARGE);
        let forbidden = app
            .clone()
            .oneshot(request(
                "PUT",
                "/api/v1/clientes/1",
                Some("synthetic-viewer-session"),
                true,
                "https://example.test",
                payload,
            ))
            .await
            .unwrap();
        assert_eq!(forbidden.status(), StatusCode::FORBIDDEN);
        core.admin()
            .set_module_enabled(1, "clientes", false)
            .await
            .unwrap();
        let unavailable = app
            .oneshot(request(
                "GET",
                "/api/v1/clientes/1",
                Some("synthetic-admin-session"),
                false,
                "https://example.test",
                "",
            ))
            .await
            .unwrap();
        assert_eq!(unavailable.status(), StatusCode::NOT_FOUND);
    }
}
