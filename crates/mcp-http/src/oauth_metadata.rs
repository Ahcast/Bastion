use serde_json::{Value, json};
use url::Url;

/// Server-owned OAuth discovery values. These documents are not exposed by
/// the application until their endpoints and public-client flow are complete.
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
    pub fn authorization_server_metadata_url(&self) -> &str {
        &self.authorization_server_metadata_url
    }

    pub fn protected_resource(&self) -> Value {
        json!({"resource": self.resource, "authorization_servers": [self.issuer], "bearer_methods_supported": ["header"]})
    }

    pub fn authorization_server(&self) -> Value {
        json!({"issuer": self.issuer, "authorization_endpoint": self.authorization_endpoint, "token_endpoint": self.token_endpoint, "response_types_supported": ["code"], "grant_types_supported": ["authorization_code", "refresh_token"], "token_endpoint_auth_methods_supported": ["none"], "code_challenge_methods_supported": ["S256"]})
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert!(
            doc.authorization_server()
                .get("client_id_metadata_document_supported")
                .is_none()
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
    }
}
