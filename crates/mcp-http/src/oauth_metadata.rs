use std::sync::Arc;

use axum::{Json, Router, extract::State, http::header::CACHE_CONTROL, routing::get};
use bastion_mcp::{ConsentResult, DeniedConsent, validate_redirect_uri};
use serde_json::{Value, json};
use url::Url;

/// Server-owned OAuth discovery values. These documents are not exposed by
/// the application until their endpoints and public-client flow are complete.
#[derive(Clone)]
pub struct DraftOauthMetadata {
    resource: String,
    issuer: String,
    authorization_endpoint: String,
    token_endpoint: String,
    protected_resource_metadata_url: String,
    authorization_server_metadata_url: String,
}

#[derive(Debug, PartialEq, Eq)]
pub struct OauthMetadataError;

impl DraftOauthMetadata {
    pub fn new(
        resource: String,
        issuer: String,
        authorization_endpoint: String,
        token_endpoint: String,
    ) -> Result<Self, OauthMetadataError> {
        fn checked(value: &str) -> Result<Url, OauthMetadataError> {
            if value.len() > 2048
                || !value
                    .bytes()
                    .all(|b| (0x21..=0x7e).contains(&b) && b != b'\\' && b != b'"')
            {
                return Err(OauthMetadataError);
            }
            let url = Url::parse(value).map_err(|_| OauthMetadataError)?;
            if url.scheme() != "https"
                || url.host().is_none()
                || !url.username().is_empty()
                || url.password().is_some()
                || url.query().is_some()
                || url.fragment().is_some()
                || url.as_str() != value
            {
                return Err(OauthMetadataError);
            }
            Ok(url)
        }
        let resource_url = checked(&resource)?;
        let issuer_url = checked(&issuer)?;
        let auth_url = checked(&authorization_endpoint)?;
        let token_url = checked(&token_endpoint)?;
        // Axum route patterns must be literal and cannot contain capture
        // syntax, percent escapes, or ambiguous path separators.
        for url in [&resource_url, &issuer_url] {
            if !url
                .path()
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"/-._~".contains(&b))
                || url.path().contains("//")
            {
                return Err(OauthMetadataError);
            }
        }
        if auth_url.origin() != issuer_url.origin()
            || token_url.origin() != issuer_url.origin()
            || auth_url == token_url
        {
            return Err(OauthMetadataError);
        }
        let metadata_url = |url: &Url, suffix: &str| {
            let path = if url.path() == "/" { "" } else { url.path() };
            format!(
                "{}/.well-known/{}{}",
                url.origin().ascii_serialization(),
                suffix,
                path
            )
        };
        let protected_resource_metadata_url =
            metadata_url(&resource_url, "oauth-protected-resource");
        let authorization_server_metadata_url =
            metadata_url(&issuer_url, "oauth-authorization-server");
        Ok(Self {
            resource,
            issuer,
            authorization_endpoint,
            token_endpoint,
            protected_resource_metadata_url,
            authorization_server_metadata_url,
        })
    }

    pub fn protected_resource_metadata_url(&self) -> &str {
        &self.protected_resource_metadata_url
    }
    pub fn resource(&self) -> &str {
        &self.resource
    }
    pub fn authorization_server_metadata_url(&self) -> &str {
        &self.authorization_server_metadata_url
    }

    pub fn protected_resource(&self) -> Value {
        json!({"resource": self.resource, "authorization_servers": [self.issuer], "bearer_methods_supported": ["header"], "scopes_supported": bastion_mcp_core::approved_read_requirements().into_keys().collect::<Vec<_>>()})
    }

    pub fn authorization_server(&self) -> Value {
        json!({"issuer": self.issuer, "authorization_endpoint": self.authorization_endpoint, "token_endpoint": self.token_endpoint, "response_types_supported": ["code"], "grant_types_supported": ["authorization_code", "refresh_token"], "token_endpoint_auth_methods_supported": ["none"], "code_challenge_methods_supported": ["S256"], "client_id_metadata_document_supported": true, "scopes_supported": bastion_mcp_core::approved_read_requirements().into_keys().collect::<Vec<_>>()})
    }

    /// Only call after the transactional consent flow has returned the
    /// registered, exactly matched redirect URI. Do not log the returned URL:
    /// it contains a short-lived authorization code and optional state.
    pub fn success_redirect(&self, consent: ConsentResult) -> Result<String, OauthMetadataError> {
        validate_redirect_uri(&consent.redirect_uri).map_err(|_| OauthMetadataError)?;
        let mut target = Url::parse(&consent.redirect_uri).map_err(|_| OauthMetadataError)?;
        {
            let mut query = target.query_pairs_mut();
            query.append_pair("code", &consent.code.expose());
            query.append_pair("iss", &self.issuer);
            if let Some(state) = consent.state {
                query.append_pair("state", &state);
            }
        }
        Ok(target.into())
    }

    /// Use only the validated result of an explicit user denial. The issuer
    /// binds the error to this authorization server (RFC 9207).
    pub fn denied_redirect(&self, denial: DeniedConsent) -> Result<String, OauthMetadataError> {
        validate_redirect_uri(&denial.redirect_uri).map_err(|_| OauthMetadataError)?;
        let mut target = Url::parse(&denial.redirect_uri).map_err(|_| OauthMetadataError)?;
        {
            let mut query = target.query_pairs_mut();
            query.append_pair("error", "access_denied");
            query.append_pair("iss", &self.issuer);
            if let Some(state) = denial.state {
                query.append_pair("state", &state);
            }
        }
        Ok(target.into())
    }
}

/// Draft router; do not mount or advertise until the authorization endpoint,
/// metadata retrieval and browser consent are implemented and verified.
pub fn draft_oauth_metadata_router(metadata: DraftOauthMetadata) -> Router {
    let resource_path = Url::parse(metadata.protected_resource_metadata_url())
        .expect("validated metadata URL")
        .path()
        .to_owned();
    let server_path = Url::parse(metadata.authorization_server_metadata_url())
        .expect("validated metadata URL")
        .path()
        .to_owned();
    Router::new()
        .route(&resource_path, get(protected_resource))
        .route(&server_path, get(authorization_server))
        .with_state(Arc::new(metadata))
}

async fn protected_resource(
    State(metadata): State<Arc<DraftOauthMetadata>>,
) -> ([(axum::http::HeaderName, &'static str); 1], Json<Value>) {
    (
        [(CACHE_CONTROL, "no-store")],
        Json(metadata.protected_resource()),
    )
}

async fn authorization_server(
    State(metadata): State<Arc<DraftOauthMetadata>>,
) -> ([(axum::http::HeaderName, &'static str); 1], Json<Value>) {
    (
        [(CACHE_CONTROL, "no-store")],
        Json(metadata.authorization_server()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::{Body, to_bytes},
        http::{Request, StatusCode},
    };
    use bastion_mcp::Credential;
    use tower::ServiceExt;

    fn metadata(
        resource: &str,
        issuer: &str,
        auth: &str,
        token: &str,
    ) -> Result<DraftOauthMetadata, OauthMetadataError> {
        DraftOauthMetadata::new(resource.into(), issuer.into(), auth.into(), token.into())
    }

    #[test]
    fn builds_well_known_urls_and_accurate_documents() {
        let doc = metadata(
            "https://example.invalid/mcp",
            "https://example.invalid/oauth",
            "https://example.invalid/oauth/authorize",
            "https://example.invalid/oauth/token",
        )
        .unwrap();
        assert_eq!(
            doc.protected_resource_metadata_url(),
            "https://example.invalid/.well-known/oauth-protected-resource/mcp"
        );
        assert_eq!(
            doc.authorization_server_metadata_url(),
            "https://example.invalid/.well-known/oauth-authorization-server/oauth"
        );
        assert_eq!(
            doc.protected_resource()["resource"],
            "https://example.invalid/mcp"
        );
        assert_eq!(
            doc.authorization_server()["issuer"],
            "https://example.invalid/oauth"
        );
        assert_eq!(
            doc.authorization_server()["token_endpoint_auth_methods_supported"][0],
            "none"
        );
        assert_eq!(
            doc.authorization_server()["client_id_metadata_document_supported"],
            true
        );
        assert_eq!(
            doc.protected_resource()["scopes_supported"]
                .as_array()
                .unwrap()
                .len(),
            3
        );
    }

    #[test]
    fn rejects_noncanonical_or_untrusted_urls() {
        let good = (
            "https://example.invalid/mcp",
            "https://example.invalid/oauth",
            "https://example.invalid/oauth/authorize",
            "https://example.invalid/oauth/token",
        );
        for resource in [
            "http://example.invalid/mcp",
            "https://user@example.invalid/mcp",
            "https://example.invalid/mcp#fragment",
            "https://example.invalid/mcp?x=1",
            "https://EXAMPLE.invalid/mcp",
        ] {
            assert!(metadata(resource, good.1, good.2, good.3).is_err());
        }
        assert!(metadata(good.0, good.1, "https://other.invalid/authorize", good.3).is_err());
        assert!(metadata(good.0, good.1, good.3, good.3).is_err());
        assert!(metadata("https://example.invalid/{capture}", good.1, good.2, good.3).is_err());
        assert!(metadata("https://example.invalid/a%2Fb", good.1, good.2, good.3).is_err());
    }

    #[tokio::test]
    async fn serves_both_draft_documents_only_at_configured_paths() {
        let doc = metadata(
            "https://example.invalid/mcp",
            "https://example.invalid/oauth",
            "https://example.invalid/oauth/authorize",
            "https://example.invalid/oauth/token",
        )
        .unwrap();
        let router = draft_oauth_metadata_router(doc);
        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/.well-known/oauth-protected-resource/mcp")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers().get(CACHE_CONTROL).unwrap(), "no-store");
        let body: Value =
            serde_json::from_slice(&to_bytes(response.into_body(), 8192).await.unwrap()).unwrap();
        assert_eq!(body["resource"], "https://example.invalid/mcp");
        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/.well-known/oauth-authorization-server/oauth")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body: Value =
            serde_json::from_slice(&to_bytes(response.into_body(), 8192).await.unwrap()).unwrap();
        assert_eq!(body["issuer"], "https://example.invalid/oauth");
        let missing = router
            .oneshot(
                Request::builder()
                    .uri("/.well-known/oauth-protected-resource/other")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(missing.status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn successful_redirect_encodes_state_and_issuer_without_ambiguity() {
        let doc = metadata(
            "https://example.invalid/mcp",
            "https://example.invalid/oauth",
            "https://example.invalid/oauth/authorize",
            "https://example.invalid/oauth/token",
        )
        .unwrap();
        let code = Credential::generate().unwrap();
        let expected = code.expose();
        let response = doc
            .success_redirect(ConsentResult {
                code,
                redirect_uri: "https://client.invalid/callback?original=yes".into(),
                state: Some("a&code=fake".into()),
            })
            .unwrap();
        let parsed = Url::parse(&response).unwrap();
        let pairs: std::collections::BTreeMap<_, _> = parsed.query_pairs().into_owned().collect();
        assert_eq!(pairs.get("original").unwrap(), "yes");
        assert_eq!(pairs.get("code").unwrap(), &expected);
        assert_eq!(pairs.get("iss").unwrap(), "https://example.invalid/oauth");
        assert_eq!(pairs.get("state").unwrap(), "a&code=fake");
        for redirect in [
            "https://client.invalid/cb?%63ode=fake",
            "https://client.invalid/cb?iss=fake",
            "https://client.invalid/cb#fragment",
            "http://client.invalid/cb",
        ] {
            assert!(
                doc.success_redirect(ConsentResult {
                    code: Credential::generate().unwrap(),
                    redirect_uri: redirect.into(),
                    state: None
                })
                .is_err()
            );
        }
        let error = doc
            .denied_redirect(DeniedConsent {
                redirect_uri: "https://client.invalid/callback?original=yes".into(),
                state: Some("a&iss=fake".into()),
            })
            .unwrap();
        let parsed = Url::parse(&error).unwrap();
        let pairs: std::collections::BTreeMap<_, _> = parsed.query_pairs().into_owned().collect();
        assert_eq!(pairs.get("error").unwrap(), "access_denied");
        assert_eq!(pairs.get("iss").unwrap(), "https://example.invalid/oauth");
        assert_eq!(pairs.get("state").unwrap(), "a&iss=fake");
    }
}
