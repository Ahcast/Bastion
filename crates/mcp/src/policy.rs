use std::collections::BTreeSet;

/// Trusted, current administrative approval, loaded by the caller from the core.
/// None of these fields may be populated from the OAuth request itself.
pub struct ClientApproval<'a> {
    pub client_id: &'a str,
    pub enabled: bool,
    /// False when client metadata has changed since administrative approval.
    pub metadata_approved: bool,
    pub redirect_uris: &'a BTreeSet<String>,
    pub allowed_scopes: &'a BTreeSet<String>,
}

pub struct AuthorizationRequest<'a> {
    pub client_id: &'a str,
    pub redirect_uri: &'a str,
    pub scopes: &'a BTreeSet<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyError {
    ClientNotApproved,
    RedirectNotApproved,
    InvalidScopes,
    ScopeNotApproved,
    ScopeNotPermitted,
}

/// Checks only whether a request can proceed to user consent. This does not
/// authenticate a user, record consent, create a grant, or issue any token.
/// `current_user_scopes` must be resolved from current core authorization.
pub fn validate_authorization_request(
    approval: &ClientApproval<'_>,
    request: &AuthorizationRequest<'_>,
    current_user_scopes: &BTreeSet<String>,
) -> Result<(), PolicyError> {
    if !approval.enabled
        || !approval.metadata_approved
        || approval.client_id.is_empty()
        || approval.client_id != request.client_id
    {
        return Err(PolicyError::ClientNotApproved);
    }
    if request.redirect_uri.is_empty() || !approval.redirect_uris.contains(request.redirect_uri) {
        return Err(PolicyError::RedirectNotApproved);
    }
    if request.scopes.is_empty() || request.scopes.iter().any(String::is_empty) {
        return Err(PolicyError::InvalidScopes);
    }
    if !request.scopes.is_subset(approval.allowed_scopes) {
        return Err(PolicyError::ScopeNotApproved);
    }
    if !request.scopes.is_subset(current_user_scopes) {
        return Err(PolicyError::ScopeNotPermitted);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(values: &[&str]) -> BTreeSet<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    #[test]
    fn accepts_only_scopes_allowed_by_both_parties() {
        let redirects = set(&["https://client.example/callback"]);
        let allowed = set(&["documents:read", "documents:write"]);
        let user = set(&["documents:read"]);
        let read = set(&["documents:read"]);
        let write = set(&["documents:write"]);
        let other = set(&["users:read"]);
        let empty = set(&[]);
        let empty_name = set(&[""]);
        let approval = ClientApproval {
            client_id: "client",
            enabled: true,
            metadata_approved: true,
            redirect_uris: &redirects,
            allowed_scopes: &allowed,
        };
        let request = |scopes| AuthorizationRequest {
            client_id: "client",
            redirect_uri: "https://client.example/callback",
            scopes,
        };
        assert_eq!(
            validate_authorization_request(&approval, &request(&read), &user),
            Ok(())
        );
        assert_eq!(
            validate_authorization_request(&approval, &request(&write), &user),
            Err(PolicyError::ScopeNotPermitted)
        );
        assert_eq!(
            validate_authorization_request(&approval, &request(&other), &other),
            Err(PolicyError::ScopeNotApproved)
        );
        assert_eq!(
            validate_authorization_request(&approval, &request(&empty), &user),
            Err(PolicyError::InvalidScopes)
        );
        assert_eq!(
            validate_authorization_request(&approval, &request(&empty_name), &user),
            Err(PolicyError::InvalidScopes)
        );
    }

    #[test]
    fn fails_closed_for_client_identity_and_metadata_changes() {
        let redirects = set(&["https://client.example/callback"]);
        let scopes = set(&["read"]);
        let request = AuthorizationRequest {
            client_id: "client",
            redirect_uri: "https://client.example/callback",
            scopes: &scopes,
        };
        for (id, enabled, metadata_approved) in [
            ("client", false, true),
            ("client", true, false),
            ("other", true, true),
            ("", true, true),
        ] {
            let approval = ClientApproval {
                client_id: id,
                enabled,
                metadata_approved,
                redirect_uris: &redirects,
                allowed_scopes: &scopes,
            };
            assert_eq!(
                validate_authorization_request(&approval, &request, &scopes),
                Err(PolicyError::ClientNotApproved)
            );
        }
    }

    #[test]
    fn requires_exact_redirect_match() {
        let redirects = set(&["https://client.example/callback"]);
        let scopes = set(&["read"]);
        let approval = ClientApproval {
            client_id: "client",
            enabled: true,
            metadata_approved: true,
            redirect_uris: &redirects,
            allowed_scopes: &scopes,
        };
        for redirect_uri in [
            "",
            "https://client.example/callback/",
            "https://client.example/callback?next=x",
            "https://CLIENT.example/callback",
            "https://client.example.evil/callback",
        ] {
            let request = AuthorizationRequest {
                client_id: "client",
                redirect_uri,
                scopes: &scopes,
            };
            assert_eq!(
                validate_authorization_request(&approval, &request, &scopes),
                Err(PolicyError::RedirectNotApproved)
            );
        }
    }
}
