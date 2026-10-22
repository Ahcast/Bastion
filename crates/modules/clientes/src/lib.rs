#![forbid(unsafe_code)]

mod service;
pub use service::ClientService;

use bastion_core::CoreError;
use serde::{Deserialize, Deserializer, Serialize};
use sha2::{Digest, Sha256};
use unicode_normalization::{UnicodeNormalization, char::is_combining_mark};

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ClientInput {
    pub cliente_name: String,
    #[serde(default, deserialize_with = "optional_text")]
    pub user_contact: String,
    #[serde(default, deserialize_with = "optional_text")]
    pub email: String,
    #[serde(default, deserialize_with = "optional_text")]
    pub rtn: String,
    #[serde(default, deserialize_with = "optional_text")]
    pub phone_number: String,
    #[serde(default, deserialize_with = "optional_text")]
    pub mobile_number: String,
    #[serde(default, deserialize_with = "optional_text")]
    pub direction: String,
}

fn optional_text<'de, D: Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    Ok(Option::<String>::deserialize(d)?.unwrap_or_default())
}

impl ClientInput {
    pub fn normalize(mut self) -> Result<Self, CoreError> {
        for (value, max) in [
            (&mut self.cliente_name, 255),
            (&mut self.user_contact, 255),
            (&mut self.email, 50),
            (&mut self.rtn, 30),
            (&mut self.phone_number, 20),
            (&mut self.mobile_number, 20),
            (&mut self.direction, 1500),
        ] {
            *value = value.trim().to_owned();
            if value.chars().count() > max
                || value
                    .chars()
                    .any(|c| c.is_control() && c != '\n' && c != '\r' && c != '\t')
            {
                return Err(CoreError::Invalid);
            }
        }
        if self.cliente_name.is_empty()
            || canonical_name(&self.cliente_name).chars().count() > 1020
            || (!self.email.is_empty() && self.email.parse::<lettre::Address>().is_err())
        {
            return Err(CoreError::Invalid);
        }
        Ok(self)
    }

    pub fn merge_into(self, mut current: Self) -> Self {
        current.cliente_name = self.cliente_name;
        for (existing, incoming) in [
            (&mut current.user_contact, self.user_contact),
            (&mut current.email, self.email),
            (&mut current.rtn, self.rtn),
            (&mut current.phone_number, self.phone_number),
            (&mut current.mobile_number, self.mobile_number),
            (&mut current.direction, self.direction),
        ] {
            if existing.is_empty() {
                *existing = incoming;
            }
        }
        current
    }

    pub(crate) fn score(&self) -> u8 {
        u8::from(!self.email.is_empty()) * 20 + u8::from(!self.rtn.is_empty()) * 10
    }
}

// Deterministic Spanish/Latin normalization; wider iconv transliteration parity
// remains pending and is explicitly documented (no locale-dependent behavior).
pub fn canonical_name(value: &str) -> String {
    value
        .nfkd()
        .filter(|c| !is_combining_mark(*c))
        .flat_map(char::to_lowercase)
        .collect::<String>()
        .replace('ß', "ss")
        .replace('æ', "ae")
        .replace('œ', "oe")
        .replace('ø', "o")
        .replace('ł', "l")
        .replace('đ', "d")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

pub(crate) fn name_hash(value: &str) -> Vec<u8> {
    Sha256::digest(canonical_name(value).as_bytes()).to_vec()
}
pub(crate) fn rtn_digits(value: &str) -> String {
    value.chars().filter(char::is_ascii_digit).collect()
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Client {
    pub id: u64,
    pub version: u64,
    #[serde(flatten)]
    pub data: ClientInput,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ClientWrite {
    pub client: Client,
    pub merged: bool,
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClientFilter {
    pub search: Option<String>,
    pub page: Option<u32>,
    pub page_size: Option<u32>,
}
impl ClientFilter {
    pub fn bounds(&self) -> Result<(u32, u32), CoreError> {
        let page = self.page.unwrap_or(1);
        let size = self.page_size.unwrap_or(25);
        if !(1..=10000).contains(&page)
            || !(1..=100).contains(&size)
            || self
                .search
                .as_ref()
                .is_some_and(|s| s.chars().count() > 200)
        {
            return Err(CoreError::Invalid);
        }
        Ok((page, size))
    }
    pub(crate) fn search(&self) -> Option<&str> {
        self.search.as_deref().map(str::trim).filter(|s| {
            !s.is_empty() && !s.eq_ignore_ascii_case("null") && !s.eq_ignore_ascii_case("undefined")
        })
    }
}

#[derive(Debug, Serialize)]
pub struct ClientPage<T = Client> {
    pub items: Vec<T>,
    pub page: u32,
    pub page_size: u32,
    pub has_more: bool,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct CatalogClient {
    pub id: u64,
    pub cliente_name: String,
    pub rtn: String,
    pub email: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_names_rtn_and_optional_payload_without_changing_tax_identity() {
        assert_eq!(canonical_name("  COMPAÑÍA   José  "), "compania jose");
        assert_eq!(canonical_name("JOSE\u{301}"), "jose");
        assert_eq!(rtn_digits("0801-1234 56"), "0801123456");
        let input: ClientInput = serde_json::from_value(
            serde_json::json!({"cliente_name":"  José  ","email":null,"rtn":" 0801-1234 "}),
        )
        .unwrap();
        let input = input.normalize().unwrap();
        assert_eq!(input.cliente_name, "José");
        assert_eq!(input.rtn, "0801-1234");
        assert_eq!(input.email, "");
        let current = ClientInput {
            cliente_name: "Uno".into(),
            rtn: "111".into(),
            email: "old@example.test".into(),
            ..Default::default()
        };
        let incoming = ClientInput {
            cliente_name: "UNO".into(),
            rtn: "222".into(),
            email: "new@example.test".into(),
            user_contact: "Ana".into(),
            ..Default::default()
        };
        let result = incoming.merge_into(current);
        assert_eq!(result.rtn, "111");
        assert_eq!(result.email, "old@example.test");
        assert_eq!(result.user_contact, "Ana");
        assert_eq!(result.cliente_name, "UNO");
    }

    #[test]
    fn validation_limits_and_mass_assignment() {
        for payload in [
            serde_json::json!({"cliente_name":" "}),
            serde_json::json!({"cliente_name":"Ana","email":"bad"}),
            serde_json::json!({"cliente_name":"Ana","phone_number":"1".repeat(21)}),
            serde_json::json!({"cliente_name":"A".repeat(256)}),
        ] {
            assert!(
                serde_json::from_value::<ClientInput>(payload)
                    .unwrap()
                    .normalize()
                    .is_err()
            );
        }
        for field in ["id", "version", "created_by", "name_hash", "tenant_id"] {
            let mut payload = serde_json::json!({"cliente_name":"Ana"});
            payload[field] = serde_json::json!(1);
            assert!(serde_json::from_value::<ClientInput>(payload).is_err());
        }
        assert!(
            ClientFilter {
                page_size: Some(101),
                ..Default::default()
            }
            .bounds()
            .is_err()
        );
        assert!(
            ClientFilter {
                page: Some(0),
                ..Default::default()
            }
            .bounds()
            .is_err()
        );
    }

    #[test]
    fn receipt_response_round_trips_with_explicit_version_and_flattened_fields() {
        let response = ClientWrite {
            client: Client {
                id: 1,
                version: 3,
                data: ClientInput {
                    cliente_name: "José".into(),
                    ..Default::default()
                },
            },
            merged: true,
        };
        let value = serde_json::to_value(&response).unwrap();
        let restored: ClientWrite = serde_json::from_value(value).unwrap();
        assert_eq!(restored.client.id, 1);
        assert_eq!(restored.client.version, 3);
        assert_eq!(restored.client.data, response.client.data);
    }
}
