use std::collections::BTreeSet;

use subtle::ConstantTimeEq;

use crate::{CredentialKind, GrantState, S256Challenge, TokenParameters, validity::validate_grant};

const MAX_CODE_SECONDS: i64 = 600;

/// Loaded from trusted storage under a write lock in the same transaction that
/// consumes the code and creates the token records.
pub struct CodeRecord<'a> {
    pub digest: [u8; 32],
    pub client_id: &'a str,
    pub redirect_uri: &'a str,
    pub resource: &'a str,
    pub challenge: &'a S256Challenge,
    pub issued_at: i64,
    pub expires_at: i64,
    pub consumed: bool,
}

/// Retain consumed refresh records until family expiry to detect replay. The
/// caller must lock the record and its grant, then rotate in one transaction.
pub struct RefreshRecord<'a> {
    pub digest: [u8; 32],
    pub client_id: &'a str,
    pub resource: &'a str,
    pub issued_at: i64,
    pub expires_at: i64,
    pub consumed: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExchangeError {
    InvalidGrant,
    /// The caller must revoke the token family and its grant transactionally.
    Replay,
    InvalidScope,
}

/// Checks a code under lock; this function does not consume it or issue tokens.
/// The caller must validate client authentication and execute all changes and
/// audit in one transaction before returning any token to the client.
pub fn validate_code_exchange(
    request: &TokenParameters,
    code: &CodeRecord<'_>,
    grant: &GrantState<'_>,
    now: i64,
) -> Result<(), ExchangeError> {
    let TokenParameters::AuthorizationCode {
        client_id,
        redirect_uri,
        resource,
        code: presented,
        verifier,
    } = request
    else {
        return Err(ExchangeError::InvalidGrant);
    };
    if !bool::from(
        presented
            .digest(CredentialKind::AuthorizationCode)
            .ct_eq(&code.digest),
    ) || client_id != code.client_id
        || redirect_uri != code.redirect_uri
        || resource != code.resource
        || code.challenge.verify(verifier).is_err()
    {
        return Err(ExchangeError::InvalidGrant);
    }
    if code.consumed {
        return Err(ExchangeError::Replay);
    }
    validate_grant(grant, now).map_err(|_| ExchangeError::InvalidGrant)?;
    if code.issued_at < grant.issued_at
        || code.issued_at > now
        || code.expires_at <= now
        || code.expires_at > grant.expires_at
        || code.expires_at <= code.issued_at
        || code
            .expires_at
            .checked_sub(code.issued_at)
            .is_none_or(|duration| duration > MAX_CODE_SECONDS)
    {
        return Err(ExchangeError::InvalidGrant);
    }
    Ok(())
}

/// Checks a refresh token under lock. A successful result is only permission to
/// rotate: the token must be marked consumed and its successor inserted in the
/// same transaction; a replay must revoke the whole grant before committing.
pub fn validate_refresh_exchange<'a>(
    request: &'a TokenParameters,
    token: &RefreshRecord<'_>,
    grant: &'a GrantState<'_>,
    now: i64,
) -> Result<&'a BTreeSet<String>, ExchangeError> {
    let TokenParameters::RefreshToken {
        client_id,
        resource,
        refresh_token,
        scopes,
    } = request
    else {
        return Err(ExchangeError::InvalidGrant);
    };
    if !bool::from(
        refresh_token
            .digest(CredentialKind::RefreshToken)
            .ct_eq(&token.digest),
    ) || client_id != token.client_id
        || resource != token.resource
    {
        return Err(ExchangeError::InvalidGrant);
    }
    if token.consumed {
        return Err(ExchangeError::Replay);
    }
    validate_grant(grant, now).map_err(|_| ExchangeError::InvalidGrant)?;
    if token.issued_at < grant.issued_at
        || token.issued_at > now
        || token.expires_at <= now
        || token.expires_at > grant.expires_at
        || token.expires_at <= token.issued_at
    {
        return Err(ExchangeError::InvalidGrant);
    }
    let effective_scopes = scopes.as_ref().unwrap_or(grant.granted_scopes);
    if effective_scopes.is_empty() || !effective_scopes.is_subset(grant.granted_scopes) {
        return Err(ExchangeError::InvalidScope);
    }
    Ok(effective_scopes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Credential;

    fn grant(scopes: &BTreeSet<String>) -> GrantState<'_> {
        GrantState {
            issued_at: 1000,
            expires_at: 2000,
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
    fn code_binds_client_redirect_resource_pkce_and_detects_replay() {
        let scopes = BTreeSet::from(["read".to_owned()]);
        let grant = grant(&scopes);
        let secret = Credential::generate().unwrap();
        let challenge =
            S256Challenge::parse("E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM").unwrap();
        let mut record = CodeRecord {
            digest: secret.digest(CredentialKind::AuthorizationCode),
            client_id: "client",
            redirect_uri: "https://client/cb",
            resource: "https://api/mcp",
            challenge: &challenge,
            issued_at: 1000,
            expires_at: 1600,
            consumed: false,
        };
        let mut request = TokenParameters::AuthorizationCode {
            client_id: "client".into(),
            redirect_uri: "https://client/cb".into(),
            resource: "https://api/mcp".into(),
            code: secret,
            verifier: "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk".into(),
        };
        assert_eq!(
            validate_code_exchange(&request, &record, &grant, 1100),
            Ok(())
        );
        if let TokenParameters::AuthorizationCode { redirect_uri, .. } = &mut request {
            *redirect_uri = "https://client/cb/".into();
        }
        assert_eq!(
            validate_code_exchange(&request, &record, &grant, 1100),
            Err(ExchangeError::InvalidGrant)
        );
        if let TokenParameters::AuthorizationCode {
            redirect_uri,
            verifier,
            ..
        } = &mut request
        {
            *redirect_uri = "https://client/cb".into();
            *verifier = "a".repeat(43);
        }
        assert_eq!(
            validate_code_exchange(&request, &record, &grant, 1100),
            Err(ExchangeError::InvalidGrant)
        );
        if let TokenParameters::AuthorizationCode { verifier, .. } = &mut request {
            *verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk".into();
        }
        record.consumed = true;
        assert_eq!(
            validate_code_exchange(&request, &record, &grant, 1100),
            Err(ExchangeError::Replay)
        );
        record.consumed = false;
        assert_eq!(
            validate_code_exchange(&request, &record, &grant, 1600),
            Err(ExchangeError::InvalidGrant)
        );
    }

    #[test]
    fn refresh_requires_rotation_and_cannot_expand_scopes() {
        let scopes = BTreeSet::from(["read".to_owned(), "write".to_owned()]);
        let grant = grant(&scopes);
        let secret = Credential::generate().unwrap();
        let mut token = RefreshRecord {
            digest: secret.digest(CredentialKind::RefreshToken),
            client_id: "client",
            resource: "https://api/mcp",
            issued_at: 1000,
            expires_at: 2000,
            consumed: false,
        };
        let mut request = TokenParameters::RefreshToken {
            client_id: "client".into(),
            resource: "https://api/mcp".into(),
            refresh_token: secret,
            scopes: Some(BTreeSet::from(["read".to_owned()])),
        };
        assert_eq!(
            validate_refresh_exchange(&request, &token, &grant, 1100).unwrap(),
            &BTreeSet::from(["read".to_owned()])
        );
        if let TokenParameters::RefreshToken { scopes, .. } = &mut request {
            *scopes = Some(BTreeSet::from(["admin".to_owned()]));
        }
        assert_eq!(
            validate_refresh_exchange(&request, &token, &grant, 1100),
            Err(ExchangeError::InvalidScope)
        );
        token.consumed = true;
        assert_eq!(
            validate_refresh_exchange(&request, &token, &grant, 1100),
            Err(ExchangeError::Replay)
        );
    }
}
