use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

/// A validated OAuth PKCE S256 challenge. Plain challenges are not supported.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct S256Challenge([u8; 32]);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PkceError {
    InvalidChallenge,
    InvalidVerifier,
    VerificationFailed,
}

impl S256Challenge {
    /// Accept only canonical, unpadded base64url encoding of a SHA-256 digest.
    pub fn parse(encoded: &str) -> Result<Self, PkceError> {
        if encoded.len() != 43 {
            return Err(PkceError::InvalidChallenge);
        }
        let decoded = URL_SAFE_NO_PAD
            .decode(encoded)
            .map_err(|_| PkceError::InvalidChallenge)?;
        let bytes: [u8; 32] = decoded
            .try_into()
            .map_err(|_| PkceError::InvalidChallenge)?;
        if URL_SAFE_NO_PAD.encode(bytes) != encoded {
            return Err(PkceError::InvalidChallenge);
        }
        Ok(Self(bytes))
    }

    /// Verify an RFC 7636 code verifier without retaining it.
    pub fn verify(&self, verifier: &str) -> Result<(), PkceError> {
        let bytes = verifier.as_bytes();
        if !valid_verifier(verifier) {
            return Err(PkceError::InvalidVerifier);
        }
        let digest = Sha256::digest(bytes);
        if bool::from(self.0.as_slice().ct_eq(digest.as_slice())) {
            Ok(())
        } else {
            Err(PkceError::VerificationFailed)
        }
    }
}

pub(crate) fn valid_verifier(verifier: &str) -> bool {
    let bytes = verifier.as_bytes();
    (43..=128).contains(&bytes.len())
        && bytes
            .iter()
            .all(|b| b.is_ascii_alphanumeric() || b"-._~".contains(b))
}

#[cfg(test)]
mod tests {
    use super::*;

    const CHALLENGE: &str = "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM";
    const VERIFIER: &str = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";

    #[test]
    fn verifies_rfc_7636_s256_example_and_rejects_mismatch() {
        let challenge = S256Challenge::parse(CHALLENGE).unwrap();
        assert_eq!(challenge.verify(VERIFIER), Ok(()));
        assert_eq!(
            challenge.verify(&"a".repeat(43)),
            Err(PkceError::VerificationFailed)
        );
    }

    #[test]
    fn rejects_invalid_verifiers() {
        let challenge = S256Challenge::parse(CHALLENGE).unwrap();
        for value in [
            "a".repeat(42),
            "a".repeat(129),
            format!("{}+", "a".repeat(42)),
            format!("{}é", "a".repeat(42)),
        ] {
            assert_eq!(challenge.verify(&value), Err(PkceError::InvalidVerifier));
        }
    }

    #[test]
    fn rejects_invalid_or_noncanonical_challenges() {
        for value in [
            "a".repeat(42),
            format!("{}=", CHALLENGE),
            format!("{}+", "a".repeat(42)),
            format!("{}N", &CHALLENGE[..42]),
        ] {
            assert_eq!(
                S256Challenge::parse(&value),
                Err(PkceError::InvalidChallenge)
            );
        }
    }
}
