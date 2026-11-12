use axum::{
    body::{Body, to_bytes},
    http::{
        Request, StatusCode,
        header::{CACHE_CONTROL, LOCATION, ORIGIN},
    },
};
use bastion_mcp::ClientMetadata;
use bastion_mcp_http::{
    ClientMetadataSource, DraftAuthorizationState, MetadataFetchError, draft_authorization_router,
};
use sqlx::mysql::MySqlPoolOptions;
use std::{
    collections::BTreeSet,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};
use tower::ServiceExt;

#[derive(Clone)]
struct NoFetcher(Arc<AtomicUsize>);
impl ClientMetadataSource for NoFetcher {
    async fn fetch(&self, _: &str) -> Result<ClientMetadata, MetadataFetchError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Err(MetadataFetchError::InvalidClient)
    }
}

#[tokio::test]
async fn anonymous_entry_hands_unknown_clients_to_fixed_frontend_without_database_or_network() {
    // A lazy, unreachable pool proves that this public route cannot create
    // clients, change approval, revoke grants or inspect database identities.
    let pool = MySqlPoolOptions::new()
        .connect_lazy("mysql://unused:unused@127.0.0.1:1/unused")
        .unwrap();
    let hits = Arc::new(AtomicUsize::new(0));
    let router = draft_authorization_router(
        DraftAuthorizationState::new(
            pool,
            "https://api.example.invalid/mcp".into(),
            "https://app.example.invalid".into(),
            BTreeSet::from(["https://client.example.invalid".into()]),
            NoFetcher(hits.clone()),
            2,
        )
        .unwrap(),
    );
    let query = |client: &str, scope: &str, redirect: &str| {
        let mut q = url::form_urlencoded::Serializer::new(String::new());
        q.append_pair("response_type", "code")
            .append_pair("client_id", client)
            .append_pair("redirect_uri", redirect)
            .append_pair("resource", "https://api.example.invalid/mcp")
            .append_pair("scope", scope)
            .append_pair("state", "opaque-private-state")
            .append_pair("code_challenge_method", "S256")
            .append_pair(
                "code_challenge",
                "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM",
            );
        q.finish()
    };
    for client in [
        "https://unknown.example/metadata.json",
        "https://changed.example/client.json",
    ] {
        let query = query(
            client,
            "cotizaciones:read procesos:read",
            "https://client.example/callback",
        );
        let result = router
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/oauth/authorize?{query}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(result.status(), StatusCode::SEE_OTHER);
        assert_eq!(result.headers()[CACHE_CONTROL], "no-store");
        let redirect = url::Url::parse(result.headers()[LOCATION].to_str().unwrap()).unwrap();
        assert_eq!(
            redirect.origin().ascii_serialization(),
            "https://app.example.invalid"
        );
        assert_eq!(redirect.path(), "/oauth/consent");
        assert_eq!(redirect.query(), None);
        assert_eq!(redirect.fragment(), Some(query.as_str()));
    }
    for (client, scope, callback) in [
        (
            "http://evil.example/metadata",
            "cotizaciones:read",
            "https://client.example/callback",
        ),
        (
            "https://client.example/metadata",
            "unknown:write",
            "https://client.example/callback",
        ),
        (
            "https://client.example/metadata",
            "cotizaciones:read",
            "http://evil.example/callback",
        ),
    ] {
        let result = router
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/oauth/authorize?{}",
                        query(client, scope, callback)
                    ))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(result.status(), StatusCode::BAD_REQUEST);
        assert!(!result.headers().contains_key(LOCATION));
        let body = to_bytes(result.into_body(), 8192).await.unwrap();
        assert!(!String::from_utf8_lossy(&body).contains("opaque-private-state"));
    }
    let result = router
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/oauth/authorize?{}",
                    query(
                        "https://client.example/metadata",
                        "cotizaciones:read",
                        "https://client.example/cb"
                    )
                ))
                .header(ORIGIN, "https://evil.example")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(result.status(), StatusCode::FORBIDDEN);
    assert_eq!(hits.load(Ordering::SeqCst), 0);
}
