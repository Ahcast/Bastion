use std::collections::{BTreeMap, BTreeSet};

use crate::{Credential, S256Challenge, pkce::valid_verifier};

const MAX_QUERY_BYTES: usize = 8192;
const MAX_PARAMETER_BYTES: usize = 2048;
const MAX_SCOPE_COUNT: usize = 64;

/// Parsed protocol input, not an authorization decision. In particular, the
/// redirect URI is untrusted until checked against the current client approval.
pub struct AuthorizationParameters {
    pub client_id: String,
    pub redirect_uri: String,
    pub resource: String,
    pub scopes: BTreeSet<String>,
    pub state: Option<String>,
    pub challenge: S256Challenge,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtocolError {
    InvalidRequest,
    UnsupportedResponseType,
    InvalidScope,
    InvalidTarget,
    InvalidPkce,
    UnsupportedGrantType,
    InvalidGrant,
}

/// Parse one authorization request. `expected_resource` is the configured
/// canonical MCP resource URI, not a URI supplied by the OAuth client.
/// Errors must be rendered locally, never redirected to an unverified URI.
pub fn parse_authorization_parameters(
    query: &str,
    expected_resource: &str,
) -> Result<AuthorizationParameters, ProtocolError> {
    if expected_resource.is_empty() {
        return Err(ProtocolError::InvalidRequest);
    }
    let parameters = parse_form(query)?;
    let required = |key: &str| -> Result<&str, ProtocolError> {
        parameters
            .get(key)
            .map(String::as_str)
            .filter(|value| !value.is_empty())
            .ok_or(ProtocolError::InvalidRequest)
    };
    if required("response_type")? != "code" {
        return Err(ProtocolError::UnsupportedResponseType);
    }
    let client_id = required("client_id")?.to_owned();
    let redirect_uri = required("redirect_uri")?.to_owned();
    let resource = required("resource")?.to_owned();
    if resource != expected_resource {
        return Err(ProtocolError::InvalidTarget);
    }
    if required("code_challenge_method")? != "S256" {
        return Err(ProtocolError::InvalidPkce);
    }
    let challenge = S256Challenge::parse(required("code_challenge")?)
        .map_err(|_| ProtocolError::InvalidPkce)?;
    let scope_string = required("scope")?;
    let scopes = parse_scopes(scope_string)?;
    Ok(AuthorizationParameters {
        client_id,
        redirect_uri,
        resource,
        scopes,
        state: parameters.get("state").cloned(),
        challenge,
    })
}

/// Sensitive fields deliberately have no Debug implementation. Parsing does not
/// authenticate a client, consume a code, rotate a token, or create a grant.
pub enum TokenParameters {
    AuthorizationCode {
        client_id: String,
        redirect_uri: String,
        resource: String,
        code: Credential,
        verifier: String,
    },
    RefreshToken {
        client_id: String,
        resource: String,
        refresh_token: Credential,
        scopes: Option<BTreeSet<String>>,
    },
}

/// Parse an application/x-www-form-urlencoded token request for public clients.
/// Confidential-client authentication must be handled by the future HTTP layer.
pub fn parse_token_parameters(
    body: &str,
    expected_resource: &str,
) -> Result<TokenParameters, ProtocolError> {
    if expected_resource.is_empty() {
        return Err(ProtocolError::InvalidRequest);
    }
    let parameters = parse_form(body)?;
    let required = |key: &str| -> Result<&str, ProtocolError> {
        parameters
            .get(key)
            .map(String::as_str)
            .filter(|value| !value.is_empty())
            .ok_or(ProtocolError::InvalidRequest)
    };
    let client_id = required("client_id")?.to_owned();
    let resource = required("resource")?.to_owned();
    if resource != expected_resource {
        return Err(ProtocolError::InvalidTarget);
    }
    match required("grant_type")? {
        "authorization_code" => {
            if parameters.contains_key("refresh_token") || parameters.contains_key("scope") {
                return Err(ProtocolError::InvalidRequest);
            }
            let code =
                Credential::parse(required("code")?).map_err(|_| ProtocolError::InvalidGrant)?;
            let verifier = required("code_verifier")?.to_owned();
            if !valid_verifier(&verifier) {
                return Err(ProtocolError::InvalidPkce);
            }
            Ok(TokenParameters::AuthorizationCode {
                client_id,
                redirect_uri: required("redirect_uri")?.to_owned(),
                resource,
                code,
                verifier,
            })
        }
        "refresh_token" => {
            if parameters.contains_key("code")
                || parameters.contains_key("code_verifier")
                || parameters.contains_key("redirect_uri")
            {
                return Err(ProtocolError::InvalidRequest);
            }
            let refresh_token = Credential::parse(required("refresh_token")?)
                .map_err(|_| ProtocolError::InvalidGrant)?;
            let scopes = parameters
                .get("scope")
                .map(|scope| parse_scopes(scope))
                .transpose()?;
            Ok(TokenParameters::RefreshToken {
                client_id,
                resource,
                refresh_token,
                scopes,
            })
        }
        _ => Err(ProtocolError::UnsupportedGrantType),
    }
}

fn parse_scopes(value: &str) -> Result<BTreeSet<String>, ProtocolError> {
    let mut scopes = BTreeSet::new();
    for scope in value.split(' ') {
        if scope.is_empty()
            || !scope
                .bytes()
                .all(|b| b == 0x21 || (0x23..=0x5b).contains(&b) || (0x5d..=0x7e).contains(&b))
            || !scopes.insert(scope.to_owned())
            || scopes.len() > MAX_SCOPE_COUNT
        {
            return Err(ProtocolError::InvalidScope);
        }
    }
    Ok(scopes)
}

fn parse_form(input: &str) -> Result<BTreeMap<String, String>, ProtocolError> {
    if input.is_empty() || input.len() > MAX_QUERY_BYTES {
        return Err(ProtocolError::InvalidRequest);
    }
    // Reject malformed escapes and invalid UTF-8 before the permissive parser.
    validate_form_encoding(input)?;
    let mut parameters = BTreeMap::new();
    for (key, value) in url::form_urlencoded::parse(input.as_bytes()) {
        if key.is_empty()
            || key.len() > MAX_PARAMETER_BYTES
            || value.len() > MAX_PARAMETER_BYTES
            || parameters
                .insert(key.into_owned(), value.into_owned())
                .is_some()
        {
            return Err(ProtocolError::InvalidRequest);
        }
    }
    Ok(parameters)
}

fn validate_form_encoding(input: &str) -> Result<(), ProtocolError> {
    let bytes = input.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            if i + 2 >= bytes.len() {
                return Err(ProtocolError::InvalidRequest);
            }
            let high = (bytes[i + 1] as char)
                .to_digit(16)
                .ok_or(ProtocolError::InvalidRequest)?;
            let low = (bytes[i + 2] as char)
                .to_digit(16)
                .ok_or(ProtocolError::InvalidRequest)?;
            decoded.push(((high << 4) | low) as u8);
            i += 3;
        } else {
            decoded.push(bytes[i]);
            i += 1;
        }
    }
    if std::str::from_utf8(&decoded).is_err() || decoded.iter().any(|b| *b < 0x20 || *b == 0x7f) {
        return Err(ProtocolError::InvalidRequest);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    const RESOURCE: &str = "https://api.example/mcp";
    const VALID: &str = "response_type=code&client_id=client&redirect_uri=https%3A%2F%2Fclient.example%2Fcb&resource=https%3A%2F%2Fapi.example%2Fmcp&scope=read+write&code_challenge_method=S256&code_challenge=E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM&state=opaque";

    #[test]
    fn parses_bound_s256_request_without_deciding_consent() {
        let result = parse_authorization_parameters(VALID, RESOURCE).unwrap();
        assert_eq!(result.client_id, "client");
        assert_eq!(result.redirect_uri, "https://client.example/cb");
        assert_eq!(result.resource, RESOURCE);
        assert_eq!(result.state.as_deref(), Some("opaque"));
        assert_eq!(
            result.scopes,
            BTreeSet::from(["read".to_owned(), "write".to_owned()])
        );
    }

    #[test]
    fn rejects_ambiguous_or_malformed_parameters() {
        for suffix in [
            "&client_id=other",
            "&%63lient_id=other",
            "&x=%",
            "&x=%ff",
            "&x=%00",
            "&x=1&x=2",
        ] {
            assert_eq!(
                parse_authorization_parameters(&(VALID.to_owned() + suffix), RESOURCE).err(),
                Some(ProtocolError::InvalidRequest)
            );
        }
        assert_eq!(
            parse_authorization_parameters(&"x".repeat(MAX_QUERY_BYTES + 1), RESOURCE).err(),
            Some(ProtocolError::InvalidRequest)
        );
    }

    #[test]
    fn rejects_wrong_resource_method_and_scopes() {
        assert_eq!(
            parse_authorization_parameters(VALID, "https://api.example/other").err(),
            Some(ProtocolError::InvalidTarget)
        );
        assert_eq!(
            parse_authorization_parameters(&VALID.replace("method=S256", "method=plain"), RESOURCE)
                .err(),
            Some(ProtocolError::InvalidPkce)
        );
        assert_eq!(
            parse_authorization_parameters(
                &VALID.replace("response_type=code", "response_type=token"),
                RESOURCE
            )
            .err(),
            Some(ProtocolError::UnsupportedResponseType)
        );
        for scope in ["read++write", "read+read", "read%22write", "read%5Cwrite"] {
            assert_eq!(
                parse_authorization_parameters(&VALID.replace("read+write", scope), RESOURCE).err(),
                Some(ProtocolError::InvalidScope)
            );
        }
    }

    #[test]
    fn parses_code_exchange_and_refresh_without_exposing_credentials() {
        let code = Credential::generate().unwrap().expose();
        let body = format!(
            "grant_type=authorization_code&client_id=client&redirect_uri=https%3A%2F%2Fclient.example%2Fcb&resource=https%3A%2F%2Fapi.example%2Fmcp&code={code}&code_verifier={}",
            "a".repeat(43)
        );
        assert!(matches!(
            parse_token_parameters(&body, RESOURCE),
            Ok(TokenParameters::AuthorizationCode { .. })
        ));
        let refresh = format!(
            "grant_type=refresh_token&client_id=client&resource=https%3A%2F%2Fapi.example%2Fmcp&refresh_token={code}&scope=read"
        );
        match parse_token_parameters(&refresh, RESOURCE).unwrap() {
            TokenParameters::RefreshToken { scopes, .. } => {
                assert_eq!(scopes, Some(BTreeSet::from(["read".to_owned()])))
            }
            _ => panic!("unexpected grant type"),
        }
        assert_eq!(
            parse_token_parameters(&(body.clone() + "&code=other"), RESOURCE).err(),
            Some(ProtocolError::InvalidRequest)
        );
        assert_eq!(
            parse_token_parameters(&(body.clone() + "&refresh_token=other"), RESOURCE).err(),
            Some(ProtocolError::InvalidRequest)
        );
        assert_eq!(
            parse_token_parameters(
                &body.replace("code_verifier=", "code_verifier=%2B"),
                RESOURCE
            )
            .err(),
            Some(ProtocolError::InvalidPkce)
        );
        assert_eq!(
            parse_token_parameters(&refresh, "https://api.example/other").err(),
            Some(ProtocolError::InvalidTarget)
        );
        assert_eq!(
            parse_token_parameters(
                &refresh.replace("grant_type=refresh_token", "grant_type=password"),
                RESOURCE
            )
            .err(),
            Some(ProtocolError::UnsupportedGrantType)
        );
    }
}
