use base64::{Engine, engine::general_purpose::STANDARD};
use bastion_core::CoreService;
use bastion_mcp::{ActorQuota, ToolQuota};
use bastion_mcp_http::{
    DraftCompositionConfig, DraftMetadataFetcher, DraftPerimeter, compose_draft_router,
};
use ipnet::IpNet;
use std::{collections::BTreeSet, io, time::Duration};
use url::Url;

/// Explicit opt-in; all addresses and the quota key are operator-controlled.
/// Enabling the router still requires the documented infrastructure checks.
pub struct McpDeployment {
    api_origin: String,
    client_origins: BTreeSet<String>,
    quota_key: [u8; 32],
}
fn invalid(name: &'static str) -> io::Error {
    io::Error::other(format!("missing or invalid {name}"))
}
fn origin(raw: &str) -> Option<String> {
    let url = Url::parse(raw).ok()?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.path() != "/"
        || url.query().is_some()
        || url.fragment().is_some()
        || url.origin().ascii_serialization() != raw
    {
        return None;
    }
    Some(raw.to_owned())
}
fn parse(
    enabled: Option<&str>,
    api: Option<&str>,
    origins: Option<&str>,
    key: Option<&str>,
) -> io::Result<Option<McpDeployment>> {
    match enabled {
        None | Some("false") | Some("0") => return Ok(None),
        Some("true") | Some("1") => {}
        _ => return Err(invalid("BASTION_MCP_ENABLED")),
    }
    let api_origin = api
        .and_then(origin)
        .ok_or_else(|| invalid("BASTION_MCP_PUBLIC_API_ORIGIN"))?;
    let mut client_origins = BTreeSet::new();
    if let Some(raw) = origins {
        if !raw.is_empty() {
            for value in raw.split(',') {
                let value =
                    origin(value.trim()).ok_or_else(|| invalid("BASTION_MCP_CLIENT_ORIGINS"))?;
                if !client_origins.insert(value) {
                    return Err(invalid("BASTION_MCP_CLIENT_ORIGINS"));
                }
            }
        }
    }
    let decoded = STANDARD
        .decode(key.ok_or_else(|| invalid("BASTION_MCP_IP_QUOTA_KEY"))?)
        .map_err(|_| invalid("BASTION_MCP_IP_QUOTA_KEY"))?;
    let quota_key: [u8; 32] = decoded
        .try_into()
        .map_err(|_| invalid("BASTION_MCP_IP_QUOTA_KEY"))?;
    if quota_key.iter().all(|byte| *byte == 0) {
        return Err(invalid("BASTION_MCP_IP_QUOTA_KEY"));
    }
    Ok(Some(McpDeployment {
        api_origin,
        client_origins,
        quota_key,
    }))
}
fn read(name: &'static str) -> io::Result<Option<String>> {
    match std::env::var(name) {
        Ok(value) => Ok(Some(value)),
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(_) => Err(invalid(name)),
    }
}
impl McpDeployment {
    pub fn from_env() -> io::Result<Option<Self>> {
        let enabled = read("BASTION_MCP_ENABLED")?;
        if matches!(enabled.as_deref(), None | Some("false") | Some("0")) {
            return Ok(None);
        }
        parse(
            enabled.as_deref(),
            read("BASTION_MCP_PUBLIC_API_ORIGIN")?.as_deref(),
            read("BASTION_MCP_CLIENT_ORIGINS")?.as_deref(),
            read("BASTION_MCP_IP_QUOTA_KEY")?.as_deref(),
        )
    }
    pub fn router(
        self,
        core: CoreService,
        frontend_origin: String,
        proxies: Vec<IpNet>,
    ) -> io::Result<axum::Router> {
        let pool = core.database().pool().clone();
        let perimeter = DraftPerimeter::new(proxies, Duration::from_secs(60), 120, 100_000)
            .and_then(|p| p.with_shared(pool, self.quota_key, 60, 120))
            .ok_or_else(|| invalid("MCP perimeter"))?;
        let config = DraftCompositionConfig {
            resource: format!("{}/mcp", self.api_origin),
            issuer: format!("{}/oauth", self.api_origin),
            authorization_endpoint: format!("{}/oauth/authorize", self.api_origin),
            token_endpoint: format!("{}/oauth/token", self.api_origin),
            frontend_origin,
            client_origins: self.client_origins,
            actor_quota: ActorQuota {
                window_seconds: 60,
                user_total: 120,
                client_total: 300,
            },
            tool_quota: ToolQuota {
                user_tool: 60,
                client_tool: 100,
            },
            max_concurrent: 16,
            max_metadata_fetches: 8,
            request_timeout: Duration::from_secs(20),
            perimeter,
        };
        let fetcher = DraftMetadataFetcher::new(Duration::from_secs(5))
            .map_err(|_| invalid("MCP metadata fetcher"))?;
        compose_draft_router(core, config, fetcher).map_err(|_| invalid("MCP configuration"))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn mysql57_application_router_keeps_api_and_mcp_routes_separate() {
        use axum::{
            body::Body,
            extract::ConnectInfo,
            http::{Request, StatusCode},
        };
        use bastion_core::Database;
        use bastion_mcp::Credential;
        use sqlx::mysql::MySqlPoolOptions;
        use tower::ServiceExt;
        let Ok(url) = std::env::var("BASTION_MCP_TEST_URL") else {
            return;
        };
        let pool = MySqlPoolOptions::new()
            .max_connections(5)
            .connect(&url)
            .await
            .unwrap();
        let db = Database::from_test_pool(pool);
        db.migrate().await.unwrap();
        let core =
            CoreService::new(db, Credential::generate().unwrap().expose(), [57; 32]).unwrap();
        let key = STANDARD.encode([58u8; 32]);
        let config = parse(
            Some("true"),
            Some("https://api.example.invalid"),
            None,
            Some(&key),
        )
        .unwrap()
        .unwrap();
        let mcp = config
            .router(core.clone(), "https://app.example.invalid".into(), vec![])
            .unwrap();
        let api = bastion_api::router(
            core,
            "https://app.example.invalid".into(),
            vec![],
            "/tmp/no-scout-fixture".into(),
        )
        .unwrap();
        let app = api.merge(mcp).layer(axum::Extension(ConnectInfo(
            "192.0.2.96:1234".parse::<std::net::SocketAddr>().unwrap(),
        )));
        let get = |path: &str| Request::builder().uri(path).body(Body::empty()).unwrap();
        assert_eq!(
            app.clone()
                .oneshot(get("/health/live"))
                .await
                .unwrap()
                .status(),
            StatusCode::OK
        );
        assert_eq!(
            app.clone()
                .oneshot(get("/.well-known/oauth-protected-resource/mcp"))
                .await
                .unwrap()
                .status(),
            StatusCode::OK
        );
        let request = Request::builder()
            .method("POST")
            .uri("/mcp")
            .body(Body::empty())
            .unwrap();
        assert_eq!(
            app.oneshot(request).await.unwrap().status(),
            StatusCode::UNAUTHORIZED
        );
    }
    #[test]
    fn config_is_disabled_by_default_and_requires_canonical_values() {
        assert!(parse(None, None, None, None).unwrap().is_none());
        assert!(
            parse(Some("false"), Some("not-url"), None, None)
                .unwrap()
                .is_none()
        );
        let key = STANDARD.encode([1u8; 32]);
        assert!(
            parse(
                Some("true"),
                Some("https://api.example.invalid"),
                Some("https://client.example.invalid"),
                Some(&key)
            )
            .unwrap()
            .is_some()
        );
        for bad in [
            "http://api.example.invalid",
            "https://api.example.invalid/path",
            "https://api.example.invalid/",
            "https://user@api.example.invalid",
        ] {
            assert!(parse(Some("true"), Some(bad), None, Some(&key)).is_err());
        }
        assert!(
            parse(
                Some("true"),
                Some("https://api.example.invalid"),
                None,
                None
            )
            .is_err()
        );
        assert!(
            parse(
                Some("true"),
                Some("https://api.example.invalid"),
                Some("*"),
                Some(&key)
            )
            .is_err()
        );
        assert!(parse(Some("yes"), None, None, None).is_err());
    }
}
