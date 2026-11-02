use std::collections::BTreeSet;

use serde::Deserialize;
use sha2::{Digest, Sha256};
use url::{Host, Url};

const MAX_DOCUMENT_BYTES: usize = 64 * 1024;
const MAX_URI_BYTES: usize = 2048;
const MAX_REDIRECTS: usize = 32;

/// Validated data for administrative review. This does not approve a client.
pub struct ClientMetadata {
    client_id: String,
    name: Option<String>,
    redirect_uris: BTreeSet<String>,
    /// Hash of the exact document bytes; any change requires a new review.
    document_hash: [u8; 32],
}

impl ClientMetadata {
    pub fn client_id(&self) -> &str {
        &self.client_id
    }
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }
    pub fn redirect_uris(&self) -> &BTreeSet<String> {
        &self.redirect_uris
    }
    pub fn document_hash(&self) -> &[u8; 32] {
        &self.document_hash
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetadataError {
    InvalidIdentifier,
    InvalidResponse,
    InvalidDocument,
    IdentityMismatch,
    InvalidRedirect,
    UnsupportedAuthentication,
}

// Serde rejects duplicate fields that this implementation consumes. Unknown
// extensions are never used as authorization inputs.
#[derive(Deserialize)]
struct Document {
    client_id: String,
    client_name: Option<String>,
    redirect_uris: Vec<String>,
    token_endpoint_auth_method: Option<String>,
    token_endpoint_auth_methods_supported: Option<Vec<String>>,
}

/// Validate bytes already fetched by a hardened HTTPS transport. That
/// transport must enforce public DNS/IP destinations, pin the resolved address,
/// verify TLS, reject redirects and limit response size/time before this call.
/// This function does not fetch URLs or protect against SSRF by itself.
pub fn validate_client_metadata(
    client_id: &str,
    fetched_url: &str,
    status: u16,
    body: &[u8],
) -> Result<ClientMetadata, MetadataError> {
    validate_client_identifier(client_id)?;
    if client_id != fetched_url {
        return Err(MetadataError::IdentityMismatch);
    }
    if status != 200 || body.is_empty() || body.len() > MAX_DOCUMENT_BYTES {
        return Err(MetadataError::InvalidResponse);
    }
    let document: Document =
        serde_json::from_slice(body).map_err(|_| MetadataError::InvalidDocument)?;
    if document.client_id != client_id {
        return Err(MetadataError::IdentityMismatch);
    }
    // This server accepts only public clients. When a client publishes its
    // supported methods, select "none" from the intersection; its singular
    // field may express a preference that this server does not support.
    let public_supported = match &document.token_endpoint_auth_methods_supported {
        Some(methods) => {
            !methods.is_empty()
                && methods.len() <= 8
                && methods.iter().all(|method| !method.is_empty() && method.len() <= 64)
                && methods.iter().collect::<BTreeSet<_>>().len() == methods.len()
                && methods.iter().any(|method| method == "none")
        }
        None => document.token_endpoint_auth_method.as_deref() == Some("none"),
    };
    if !public_supported {
        return Err(MetadataError::UnsupportedAuthentication);
    }
    let value: serde_json::Value =
        serde_json::from_slice(body).map_err(|_| MetadataError::InvalidDocument)?;
    let object = value.as_object().ok_or(MetadataError::InvalidDocument)?;
    if [
        "client_secret",
        "client_secret_expires_at",
        "jwks",
    ]
    .iter()
    .any(|key| object.contains_key(*key))
    {
        return Err(MetadataError::UnsupportedAuthentication);
    }
    // A public client may publish a JWKS URI for other servers; Bastion does
    // not fetch or use it, and still authenticates this client as "none".
    if object.contains_key("jwks_uri") && document.token_endpoint_auth_methods_supported.is_none() {
        return Err(MetadataError::UnsupportedAuthentication);
    }
    if document.client_name.as_ref().is_none_or(|name| {
        name.is_empty() || name.len() > 255 || name.chars().any(char::is_control)
    }) {
        return Err(MetadataError::InvalidDocument);
    }
    if document.redirect_uris.is_empty() || document.redirect_uris.len() > MAX_REDIRECTS {
        return Err(MetadataError::InvalidRedirect);
    }
    let mut redirects = BTreeSet::new();
    for uri in document.redirect_uris {
        validate_redirect_uri(&uri)?;
        if !redirects.insert(uri) {
            return Err(MetadataError::InvalidRedirect);
        }
    }
    Ok(ClientMetadata {
        client_id: client_id.to_owned(),
        name: document.client_name,
        redirect_uris: redirects,
        document_hash: Sha256::digest(body).into(),
    })
}

pub fn validate_client_identifier(value: &str) -> Result<(), MetadataError> {
    if value.len() > MAX_URI_BYTES || !value.starts_with("https://") {
        return Err(MetadataError::InvalidIdentifier);
    }
    let url = Url::parse(value).map_err(|_| MetadataError::InvalidIdentifier)?;
    let after_scheme = &value["https://".len()..];
    let authority_and_path = after_scheme.split(['?', '#']).next().unwrap_or("");
    let path_start = authority_and_path
        .find('/')
        .ok_or(MetadataError::InvalidIdentifier)?;
    let authority = &authority_and_path[..path_start];
    let raw_path = &authority_and_path[path_start..];
    let has_dot_segment = raw_path.split('/').any(|segment| {
        let decoded = segment.to_ascii_lowercase().replace("%2e", ".");
        decoded == "." || decoded == ".."
    });
    if authority.contains('@')
        || url.host().is_none()
        || url.fragment().is_some()
        || has_dot_segment
    {
        return Err(MetadataError::InvalidIdentifier);
    }
    Ok(())
}

/// Validate registered redirect destinations and reject query parameters that
/// would make an authorization response ambiguous after appending its fields.
pub fn validate_redirect_uri(value: &str) -> Result<(), MetadataError> {
    if value.len() > MAX_URI_BYTES || value.chars().any(char::is_control) {
        return Err(MetadataError::InvalidRedirect);
    }
    let parsed = Url::parse(value).map_err(|_| MetadataError::InvalidRedirect)?;
    let authority = value
        .split_once("://")
        .map(|(_, rest)| rest.split(['/', '?', '#']).next().unwrap_or(""))
        .ok_or(MetadataError::InvalidRedirect)?;
    let loopback = match parsed.host() {
        Some(Host::Ipv4(ip)) => ip.is_loopback(),
        Some(Host::Ipv6(ip)) => ip.is_loopback(),
        _ => false,
    };
    if authority.contains('@')
        || parsed.host().is_none()
        || parsed.fragment().is_some()
        || parsed.query_pairs().any(|(key, _)| {
            matches!(
                key.as_ref(),
                "code" | "state" | "iss" | "error" | "error_description" | "error_uri"
            )
        })
        || !(parsed.scheme() == "https" || (parsed.scheme() == "http" && loopback))
    {
        return Err(MetadataError::InvalidRedirect);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    const ID: &str = "https://client.example/metadata.json";
    const VALID: &[u8] = br#"{"client_id":"https://client.example/metadata.json","client_name":"Example","redirect_uris":["https://client.example/callback","http://127.0.0.1:41234/cb"],"token_endpoint_auth_method":"none"}"#;

    #[test]
    fn accepts_public_method_from_supported_list_without_enabling_confidential_auth() {
        let mut document: serde_json::Value = serde_json::from_slice(VALID).unwrap();
        document["token_endpoint_auth_method"] = "private_key_jwt".into();
        document["token_endpoint_auth_methods_supported"] = serde_json::json!(["none", "private_key_jwt"]);
        document["jwks_uri"] = "https://client.example/jwks.json".into();
        let check = |value: &serde_json::Value| {
            validate_client_metadata(ID, ID, 200, value.to_string().as_bytes()).map(|_| ())
        };
        assert!(check(&document).is_ok());
        let mut confidential = document.clone();
        confidential["token_endpoint_auth_methods_supported"] = serde_json::json!(["private_key_jwt"]);
        assert_eq!(check(&confidential), Err(MetadataError::UnsupportedAuthentication));
        let mut duplicate = document.clone();
        duplicate["token_endpoint_auth_methods_supported"] = serde_json::json!(["none", "none"]);
        assert_eq!(check(&duplicate), Err(MetadataError::UnsupportedAuthentication));
        let mut secret = document.clone();
        secret["client_secret"] = "not-allowed".into();
        assert_eq!(check(&secret), Err(MetadataError::UnsupportedAuthentication));
        let mut wrong_type = document;
        wrong_type["token_endpoint_auth_methods_supported"] = "none".into();
        assert_eq!(check(&wrong_type), Err(MetadataError::InvalidDocument));
    }

    #[test]
    fn validates_client_identity_and_preserves_exact_redirects() {
        let doc = validate_client_metadata(ID, ID, 200, VALID).unwrap();
        assert!(doc.redirect_uris.contains("http://127.0.0.1:41234/cb"));
        assert_eq!(doc.name.as_deref(), Some("Example"));
        assert_eq!(
            validate_client_metadata(ID, "https://client.example:443/metadata.json", 200, VALID)
                .err(),
            Some(MetadataError::IdentityMismatch)
        );
        assert_eq!(
            validate_client_metadata(ID, ID, 302, VALID).err(),
            Some(MetadataError::InvalidResponse)
        );
        let mut changed = VALID.to_vec();
        changed.push(b' ');
        assert_ne!(
            doc.document_hash,
            validate_client_metadata(ID, ID, 200, &changed)
                .unwrap()
                .document_hash
        );
    }

    #[test]
    fn rejects_unsafe_or_ambiguous_metadata() {
        let missing_name = br#"{"client_id":"https://client.example/metadata.json","redirect_uris":["https://client.example/callback"],"token_endpoint_auth_method":"none"}"#;
        assert!(matches!(
            validate_client_metadata(ID, ID, 200, missing_name),
            Err(MetadataError::InvalidDocument)
        ));
        assert!(validate_redirect_uri("https://client.example/callback?next=home").is_ok());
        for key in [
            "code",
            "%63ode",
            "state",
            "iss",
            "error",
            "error_uri",
            "error_description",
        ] {
            assert_eq!(
                validate_redirect_uri(&format!("https://client.example/callback?{key}=value")),
                Err(MetadataError::InvalidRedirect)
            );
        }
        for value in [
            "http://client.example/doc",
            "https://client.example",
            "https://client.example?q=/doc",
            "https://user@client.example/doc",
            "https://client.example/a/%2e%2e/doc",
            "https://client.example/doc#fragment",
        ] {
            assert_eq!(
                validate_client_metadata(value, value, 200, VALID).err(),
                Some(MetadataError::InvalidIdentifier)
            );
        }
        let input = String::from_utf8(VALID.to_vec()).unwrap();
        for (from, to, error) in [
            (
                "https://client.example/callback",
                "http://client.example/callback",
                MetadataError::InvalidRedirect,
            ),
            (
                "https://client.example/callback",
                "https://client.example/callback#x",
                MetadataError::InvalidRedirect,
            ),
            (
                "\"none\"",
                "\"client_secret_basic\"",
                MetadataError::UnsupportedAuthentication,
            ),
            (
                "\"client_name\":\"Example\"",
                "\"client_name\":\"Example\",\"client_name\":\"Other\"",
                MetadataError::InvalidDocument,
            ),
            (
                "\"client_id\":\"https://client.example/metadata.json\"",
                "\"client_id\":\"https://client.example/metadata.json\",\"client_id\":\"https://other.example/metadata.json\"",
                MetadataError::InvalidDocument,
            ),
            (
                "\"token_endpoint_auth_method\":\"none\"",
                "\"token_endpoint_auth_method\":\"none\",\"client_secret\":null",
                MetadataError::UnsupportedAuthentication,
            ),
        ] {
            assert_eq!(
                validate_client_metadata(ID, ID, 200, input.replace(from, to).as_bytes()).err(),
                Some(error)
            );
        }
    }
}
