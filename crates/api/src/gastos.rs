use super::*;
use bastion_gastos::{Expense, ExpenseFilter, ExpenseInput, ExpensePage};

pub(super) fn routes(state: ApiState) -> Router<ApiState> {
    Router::new()
        .route("/api/v1/gastos", get(list).post(create))
        .route("/api/v1/gastos/calculo", post(preview))
        .route(
            "/api/v1/gastos/{id}",
            get(detail).put(replace).delete(remove),
        )
        .layer(RequestBodyLimitLayer::new(16 * 1024))
        .route_layer(middleware::from_fn_with_state(state, guard))
}

async fn guard(State(state): State<ApiState>, request: Request, next: Next) -> Response {
    match tokio::time::timeout(Duration::from_secs(30), async {
        let user = actor(&state, request.headers()).await?;
        let capability = if request.method() == Method::GET || request.method() == Method::HEAD {
            "ver"
        } else if request.method() == Method::DELETE {
            "eliminar"
        } else {
            "editar"
        };
        state
            .gastos
            .check_access(user.id, capability)
            .await
            .map_err(ApiError)?;
        state
            .gastos
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
    Query(filter): Query<ExpenseFilter>,
) -> Result<Json<ExpensePage>, ApiError> {
    let user = actor(&state, &headers).await?;
    state
        .gastos
        .list(user.id, &filter)
        .await
        .map(Json)
        .map_err(ApiError)
}

async fn preview(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Json(input): Json<ExpenseInput>,
) -> Result<Json<bastion_gastos::ExpenseCalculation>, ApiError> {
    let user = actor(&state, &headers).await?;
    state
        .gastos
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
        .gastos
        .detail(user.id, id)
        .await
        .map(expense_response)
        .map_err(ApiError)
}

async fn create(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Json(input): Json<ExpenseInput>,
) -> Result<Response, ApiError> {
    let user = actor(&state, &headers).await?;
    let key = single_header(&headers, "idempotency-key")?;
    let expense = state
        .gastos
        .create(user.id, key, input)
        .await
        .map_err(ApiError)?;
    let id = expense.id;
    let version = expense.version;
    let mut response = Json(expense).into_response();
    etag(&mut response, version);
    *response.status_mut() = StatusCode::CREATED;
    response.headers_mut().insert(
        "location",
        HeaderValue::from_str(&format!("/api/v1/gastos/{id}"))
            .map_err(|_| ApiError(CoreError::Invalid))?,
    );
    Ok(response)
}

async fn replace(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Path(id): Path<u64>,
    Json(input): Json<ExpenseInput>,
) -> Result<Response, ApiError> {
    let user = actor(&state, &headers).await?;
    let expected =
        version(single_header(&headers, "if-match")?).ok_or(ApiError(CoreError::Invalid))?;
    match state.gastos.replace(user.id, id, expected, input).await {
        Ok(expense) => Ok(expense_response(expense)),
        Err(CoreError::PreconditionFailed) => Ok(problem(
            StatusCode::PRECONDITION_FAILED,
            "Precondition Failed",
            "El gasto cambió. Tus cambios no se guardaron; vuelve a abrir la versión actual.",
        )),
        Err(error) => Err(ApiError(error)),
    }
}

async fn remove(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Path(id): Path<u64>,
) -> Result<StatusCode, ApiError> {
    let user = actor(&state, &headers).await?;
    state.gastos.delete(user.id, id).await.map_err(ApiError)?;
    Ok(StatusCode::NO_CONTENT)
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

fn expense_response(expense: Expense) -> Response {
    let version = expense.version;
    let mut response = Json(expense).into_response();
    etag(&mut response, version);
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn updates_require_one_strong_version() {
        assert_eq!(version("\"12\""), Some(12));
        for value in [
            "12",
            "*",
            "W/\"1\"",
            "\"0\"",
            "\"01\"",
            "\"-1\"",
            "\"1\",\"2\"",
        ] {
            assert!(version(value).is_none());
        }
        let mut headers = axum::http::HeaderMap::new();
        headers.append("if-match", HeaderValue::from_static("\"1\""));
        headers.append("if-match", HeaderValue::from_static("\"2\""));
        assert!(single_header(&headers, "if-match").is_err());
    }
    #[tokio::test]
    async fn mysql57_expense_http_auth_csrf_preview_crud_and_limits() {
        use axum::body::{Body, to_bytes};
        use bastion_core::Database;
        use sqlx::mysql::MySqlPoolOptions;
        use tower::ServiceExt;
        let Ok(url) = std::env::var("BASTION_GASTOS_HTTP_TEST_URL") else {
            eprintln!("Requires isolated BASTION_GASTOS_HTTP_TEST_URL; CI supplies it.");
            return;
        };
        let pool = MySqlPoolOptions::new()
            .max_connections(5)
            .connect(&url)
            .await
            .unwrap();
        let name: String = sqlx::query_scalar("SELECT DATABASE()")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(name, "bastion_gastos_http_test");
        let db = Database::from_test_pool(pool.clone());
        db.migrate().await.unwrap();
        bastion_gastos::ExpenseService::migrate(&db).await.unwrap();
        sqlx::query("INSERT INTO persons (id,full_name) VALUES (1,'HTTP Admin'),(2,'HTTP Viewer')")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO roles (id,name) VALUES (2,'HTTP Viewer')")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO users (id,person_id,role_id,email,status) VALUES (1,1,1,'admin@example.test','active'),(2,2,2,'viewer@example.test','active')").execute(&pool).await.unwrap();
        for (user, token) in [
            (1_u64, "synthetic-expense-admin"),
            (2, "synthetic-expense-viewer"),
        ] {
            sqlx::query("INSERT INTO sessions (user_id,token_hash,csrf_hash,ip_address,started_at,last_activity_at,last_totp_at,expires_at) VALUES (?,UNHEX(SHA2(?,256)),UNHEX(SHA2('synthetic-csrf',256)),'127.0.0.1',UTC_TIMESTAMP(6),UTC_TIMESTAMP(6),UTC_TIMESTAMP(6),UTC_TIMESTAMP(6)+INTERVAL 1 HOUR)").bind(user).bind(token).execute(&pool).await.unwrap();
        }
        let core = CoreService::new(db.clone(), "x".repeat(32), [1; 32]).unwrap();
        let svc = bastion_gastos::ExpenseService::new(db, core.admin());
        svc.synchronize_release(true).await.unwrap();
        core.admin()
            .set_module_enabled(1, "gastos", true)
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
                       body: &str,
                       key: Option<&str>,
                       etag_value: Option<&str>| {
            let mut b = axum::http::Request::builder()
                .method(method)
                .uri(path)
                .header("origin", origin)
                .header("content-type", "application/json");
            if let Some(token) = session {
                b = b.header(
                    "cookie",
                    format!("__Host-bastion_session={token}; __Host-bastion_csrf=synthetic-csrf"),
                );
            }
            if csrf {
                b = b.header("x-csrf-token", "synthetic-csrf");
            }
            if let Some(key) = key {
                b = b.header("idempotency-key", key);
            }
            if let Some(value) = etag_value {
                b = b.header("if-match", value);
            }
            let mut request = b.body(Body::from(body.to_owned())).unwrap();
            request
                .extensions_mut()
                .insert(ConnectInfo("127.0.0.1:5010".parse::<SocketAddr>().unwrap()));
            request
        };
        let origin = "https://example.test";
        let admin = Some("synthetic-expense-admin");
        let viewer = Some("synthetic-expense-viewer");
        let key = Some("http-expense-create-1");
        let payload = r#"{"expense_date":"2026-10-24","invoice_number":"F-001","provider":"Proveedor HTTP","currency":"HNL","taxable_amount":"100.005","exempt_amount":"20.005"}"#;
        let response = app
            .clone()
            .oneshot(request(
                "GET",
                "/api/v1/gastos",
                None,
                false,
                origin,
                "",
                None,
                None,
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        for (csrf, site) in [(false, origin), (true, "https://evil.example")] {
            let response = app
                .clone()
                .oneshot(request(
                    "POST",
                    "/api/v1/gastos",
                    admin,
                    csrf,
                    site,
                    payload,
                    key,
                    None,
                ))
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::FORBIDDEN);
        }
        let response = app
            .clone()
            .oneshot(request(
                "GET",
                "/api/v1/gastos",
                viewer,
                false,
                origin,
                "",
                None,
                None,
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        sqlx::query("INSERT INTO role_permissions (role_id,module_id,scope_hash,scope_key,capability) VALUES (2,'gastos',UNHEX(SHA2('registros',256)),'registros','ver')").execute(&pool).await.unwrap();
        let response = app
            .clone()
            .oneshot(request(
                "POST",
                "/api/v1/gastos/calculo",
                admin,
                true,
                origin,
                payload,
                None,
                None,
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let value: serde_json::Value =
            serde_json::from_slice(&to_bytes(response.into_body(), 16384).await.unwrap()).unwrap();
        assert_eq!(value["total"], "135.02");
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM gastos_records")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 0);
        for _ in 0..2 {
            let response = app
                .clone()
                .oneshot(request(
                    "POST",
                    "/api/v1/gastos",
                    admin,
                    true,
                    origin,
                    payload,
                    key,
                    None,
                ))
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::CREATED);
            assert_eq!(response.headers()["etag"], "\"1\"");
            assert_eq!(response.headers()["location"], "/api/v1/gastos/1");
        }
        let response = app
            .clone()
            .oneshot(request(
                "POST",
                "/api/v1/gastos",
                admin,
                true,
                origin,
                &payload.replace("F-001", "F-002"),
                key,
                None,
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CONFLICT);
        for (body, key_value, expected) in [
            (payload, None, StatusCode::BAD_REQUEST),
            ("{}", key, StatusCode::UNPROCESSABLE_ENTITY),
        ] {
            let response = app
                .clone()
                .oneshot(request(
                    "POST",
                    "/api/v1/gastos",
                    admin,
                    true,
                    origin,
                    body,
                    key_value,
                    None,
                ))
                .await
                .unwrap();
            assert_eq!(response.status(), expected);
        }
        for (version, expected) in [
            (None, StatusCode::BAD_REQUEST),
            (Some("W/\"1\""), StatusCode::BAD_REQUEST),
            (Some("\"1\""), StatusCode::OK),
            (Some("\"1\""), StatusCode::PRECONDITION_FAILED),
        ] {
            let response = app
                .clone()
                .oneshot(request(
                    "PUT",
                    "/api/v1/gastos/1",
                    admin,
                    true,
                    origin,
                    payload,
                    None,
                    version,
                ))
                .await
                .unwrap();
            assert_eq!(response.status(), expected);
        }
        let response = app
            .clone()
            .oneshot(request(
                "GET",
                "/api/v1/gastos?page_size=1&date_from=2026-10-24&date_to=2026-10-24",
                viewer,
                false,
                origin,
                "",
                None,
                None,
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let value: serde_json::Value =
            serde_json::from_slice(&to_bytes(response.into_body(), 16384).await.unwrap()).unwrap();
        assert_eq!(value["summary"]["total"], "135.02");
        assert_eq!(value["summary"]["count"], 1);
        let response = app
            .clone()
            .oneshot(request(
                "GET",
                "/api/v1/gastos?date_from=2026-10-25&date_to=2026-10-24",
                admin,
                false,
                origin,
                "",
                None,
                None,
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        for method in ["PUT", "DELETE"] {
            let response = app
                .clone()
                .oneshot(request(
                    method,
                    "/api/v1/gastos/1",
                    viewer,
                    true,
                    origin,
                    payload,
                    None,
                    Some("\"2\""),
                ))
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::FORBIDDEN);
        }
        let response = app
            .clone()
            .oneshot(request(
                "POST",
                "/api/v1/gastos",
                admin,
                true,
                origin,
                &"x".repeat(17 * 1024),
                key,
                None,
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
        let response = app
            .clone()
            .oneshot(request(
                "DELETE",
                "/api/v1/gastos/1",
                admin,
                true,
                origin,
                "",
                None,
                None,
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        let response = app
            .clone()
            .oneshot(request(
                "GET",
                "/api/v1/gastos/1",
                admin,
                false,
                origin,
                "",
                None,
                None,
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let retained: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM gastos_records WHERE deleted_at IS NOT NULL")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(retained, 1);
        let events: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM audit_events WHERE module_id='gastos'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(events, 3);
        sqlx::query("UPDATE gastos_actor_limits SET requests=120,window_start=UTC_TIMESTAMP(6) WHERE actor_id=1").execute(&pool).await.unwrap();
        let response = app
            .clone()
            .oneshot(request(
                "GET",
                "/api/v1/gastos",
                admin,
                false,
                origin,
                "",
                None,
                None,
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
        core.admin()
            .set_module_enabled(1, "gastos", false)
            .await
            .unwrap();
        let response = app
            .oneshot(request(
                "GET",
                "/api/v1/gastos",
                admin,
                false,
                origin,
                "",
                None,
                None,
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
}
