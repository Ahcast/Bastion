use std::collections::BTreeSet;

const ACCESS_TOKEN_MAX_SECONDS: i64 = 15 * 60;
const GRANT_MAX_SECONDS: i64 = 15 * 24 * 60 * 60;

/// Current trusted state retrieved for every request, never from token claims alone.
pub struct GrantState<'a> {
    pub issued_at: i64,
    pub expires_at: i64,
    pub revoked: bool,
    pub user_active: bool,
    pub client_approved: bool,
    pub client_metadata_approved: bool,
    /// The caller must reflect password changes and recovery in this value.
    pub credentials_current: bool,
    pub granted_scopes: &'a BTreeSet<String>,
    pub client_max_scopes: &'a BTreeSet<String>,
    pub current_user_scopes: &'a BTreeSet<String>,
}

pub struct TokenWindow {
    pub issued_at: i64,
    pub expires_at: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValidityError {
    GrantInactive,
    InvalidGrantWindow,
    InvalidTokenWindow,
    ScopesNoLongerAllowed,
}

/// Validate the time window and current authorization of an already identified
/// access token and its grant. Signature/token lookup, audience, actor, tenant,
/// object access, rate limits, and audit are separate mandatory checks.
pub fn validate_token_window(
    grant: &GrantState<'_>,
    token: &TokenWindow,
    now: i64,
) -> Result<(), ValidityError> {
    validate_grant(grant, now)?;
    if token.issued_at < grant.issued_at
        || token.issued_at > now
        || token.expires_at <= now
        || token.expires_at <= token.issued_at
        || token.expires_at > grant.expires_at
        || token
            .expires_at
            .checked_sub(token.issued_at)
            .is_none_or(|duration| duration > ACCESS_TOKEN_MAX_SECONDS)
    {
        return Err(ValidityError::InvalidTokenWindow);
    }
    Ok(())
}

pub(crate) fn validate_grant(grant: &GrantState<'_>, now: i64) -> Result<(), ValidityError> {
    if grant.revoked
        || !grant.user_active
        || !grant.client_approved
        || !grant.client_metadata_approved
        || !grant.credentials_current
    {
        return Err(ValidityError::GrantInactive);
    }
    if grant.issued_at > now
        || grant.expires_at <= now
        || grant.expires_at <= grant.issued_at
        || grant
            .expires_at
            .checked_sub(grant.issued_at)
            .is_none_or(|duration| duration > GRANT_MAX_SECONDS)
    {
        return Err(ValidityError::InvalidGrantWindow);
    }
    if grant.granted_scopes.is_empty()
        || !grant.granted_scopes.is_subset(grant.client_max_scopes)
        || !grant.granted_scopes.is_subset(grant.current_user_scopes)
    {
        return Err(ValidityError::ScopesNoLongerAllowed);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grant(scopes: &BTreeSet<String>) -> GrantState<'_> {
        GrantState {
            issued_at: 1_000,
            expires_at: 1_000 + GRANT_MAX_SECONDS,
            revoked: false,
            user_active: true,
            client_approved: true,
            client_metadata_approved: true,
            credentials_current: true,
            granted_scopes: scopes,
            client_max_scopes: scopes,
            current_user_scopes: scopes,
        }
    }

    #[test]
    fn enforces_expiry_and_maximum_lifetimes() {
        let scopes = BTreeSet::from(["read".to_owned()]);
        let mut g = grant(&scopes);
        let mut token = TokenWindow {
            issued_at: 1_100,
            expires_at: 2_000,
        };
        assert_eq!(validate_token_window(&g, &token, 1_100), Ok(()));
        assert_eq!(
            validate_token_window(&g, &token, 2_000),
            Err(ValidityError::InvalidTokenWindow)
        );
        token.expires_at += 1;
        assert_eq!(
            validate_token_window(&g, &token, 1_100),
            Err(ValidityError::InvalidTokenWindow)
        );
        token.expires_at -= 1;
        g.expires_at += 1;
        assert_eq!(
            validate_token_window(&g, &token, 1_100),
            Err(ValidityError::InvalidGrantWindow)
        );
        g.issued_at = i64::MIN;
        assert_eq!(
            validate_token_window(&g, &token, 1_100),
            Err(ValidityError::InvalidGrantWindow)
        );
    }

    #[test]
    fn rejects_revoked_or_outdated_identity_and_scopes() {
        let scopes = BTreeSet::from(["read".to_owned()]);
        let empty = BTreeSet::new();
        let token = TokenWindow {
            issued_at: 1_100,
            expires_at: 2_000,
        };
        let mut g = grant(&scopes);
        g.revoked = true;
        assert_eq!(
            validate_token_window(&g, &token, 1_100),
            Err(ValidityError::GrantInactive)
        );
        g.revoked = false;
        g.credentials_current = false;
        assert_eq!(
            validate_token_window(&g, &token, 1_100),
            Err(ValidityError::GrantInactive)
        );
        g.credentials_current = true;
        g.client_approved = false;
        assert_eq!(
            validate_token_window(&g, &token, 1_100),
            Err(ValidityError::GrantInactive)
        );
        g.client_approved = true;
        g.client_metadata_approved = false;
        assert_eq!(
            validate_token_window(&g, &token, 1_100),
            Err(ValidityError::GrantInactive)
        );
        g.client_metadata_approved = true;
        g.user_active = false;
        assert_eq!(
            validate_token_window(&g, &token, 1_100),
            Err(ValidityError::GrantInactive)
        );
        g.user_active = true;
        g.client_max_scopes = &empty;
        assert_eq!(
            validate_token_window(&g, &token, 1_100),
            Err(ValidityError::ScopesNoLongerAllowed)
        );
        g.client_max_scopes = &scopes;
        g.current_user_scopes = &empty;
        assert_eq!(
            validate_token_window(&g, &token, 1_100),
            Err(ValidityError::ScopesNoLongerAllowed)
        );
    }
}
