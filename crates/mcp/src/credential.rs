use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use rand::{RngCore, rngs::OsRng};
use sha2::{Digest, Sha256};

/// Random, opaque credential. Deliberately does not implement Debug or Clone.
pub struct Credential([u8; 32]);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CredentialKind {
    AuthorizationCode,
    AccessToken,
    RefreshToken,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CredentialError {
    RandomnessUnavailable,
    InvalidEncoding,
}

impl Credential {
    pub fn generate() -> Result<Self, CredentialError> {
        let mut bytes = [0; 32];
        OsRng
            .try_fill_bytes(&mut bytes)
            .map_err(|_| CredentialError::RandomnessUnavailable)?;
        Ok(Self(bytes))
    }

    pub fn parse(value: &str) -> Result<Self, CredentialError> {
        if value.len() != 43 {
            return Err(CredentialError::InvalidEncoding);
        }
        let decoded = URL_SAFE_NO_PAD
            .decode(value)
            .map_err(|_| CredentialError::InvalidEncoding)?;
        let bytes: [u8; 32] = decoded
            .try_into()
            .map_err(|_| CredentialError::InvalidEncoding)?;
        if URL_SAFE_NO_PAD.encode(bytes) != value {
            return Err(CredentialError::InvalidEncoding);
        }
        Ok(Self(bytes))
    }

    /// Only use at the protocol boundary; do not log or persist the result.
    pub fn expose(&self) -> String {
        URL_SAFE_NO_PAD.encode(self.0)
    }

    /// A purpose-separated lookup digest. Store this value, never the secret.
    pub fn digest(&self, kind: CredentialKind) -> [u8; 32] {
        let prefix: &[u8] = match kind {
            CredentialKind::AuthorizationCode => b"bastion/oauth/code/v1\0",
            CredentialKind::AccessToken => b"bastion/oauth/access/v1\0",
            CredentialKind::RefreshToken => b"bastion/oauth/refresh/v1\0",
        };
        let mut hash = Sha256::new();
        hash.update(prefix);
        hash.update(self.0);
        hash.finalize().into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_credentials_round_trip_and_are_purpose_separated() {
        let value = Credential::generate().unwrap();
        let encoded = value.expose();
        let parsed = Credential::parse(&encoded).unwrap();
        assert_eq!(
            value.digest(CredentialKind::AuthorizationCode),
            parsed.digest(CredentialKind::AuthorizationCode)
        );
        assert_ne!(
            value.digest(CredentialKind::AuthorizationCode),
            value.digest(CredentialKind::AccessToken)
        );
        assert_ne!(
            value.digest(CredentialKind::RefreshToken),
            value.digest(CredentialKind::AccessToken)
        );
    }

    #[test]
    fn rejects_noncanonical_and_malformed_values() {
        for value in [
            "a".repeat(42),
            "a".repeat(44),
            format!("{}+", "a".repeat(42)),
            format!("{}N", "a".repeat(42)),
        ] {
            assert!(matches!(
                Credential::parse(&value),
                Err(CredentialError::InvalidEncoding)
            ));
        }
    }
}
