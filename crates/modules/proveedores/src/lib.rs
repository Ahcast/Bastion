#![forbid(unsafe_code)]

mod service;
pub use service::SupplierService;

use bastion_core::CoreError;
use serde::{Deserialize, Deserializer, Serialize};

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SupplierInput {
    pub supplier_name: String,
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

fn optional_text<'de, D: Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
    Ok(Option::<String>::deserialize(deserializer)?.unwrap_or_default())
}

impl SupplierInput {
    pub fn normalize(mut self) -> Result<Self, CoreError> {
        for (value, max) in [
            (&mut self.supplier_name, 255),
            (&mut self.user_contact, 255),
            (&mut self.email, 254),
            (&mut self.rtn, 100),
            (&mut self.phone_number, 60),
            (&mut self.mobile_number, 60),
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
        if self.supplier_name.is_empty() {
            return Err(CoreError::Invalid);
        }
        Ok(self)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Supplier {
    pub id: u64,
    pub version: u64,
    #[serde(flatten)]
    pub data: SupplierInput,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct CatalogSupplier {
    pub id: u64,
    pub supplier_name: String,
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SupplierFilter {
    pub search: Option<String>,
    pub page: Option<u32>,
    pub page_size: Option<u32>,
}

impl SupplierFilter {
    pub fn bounds(&self) -> Result<(u32, u32), CoreError> {
        let page = self.page.unwrap_or(1);
        let size = self.page_size.unwrap_or(25);
        if !(1..=10000).contains(&page)
            || !(1..=100).contains(&size)
            || self.search.as_ref().is_some_and(|value| value.chars().count() > 200)
        {
            return Err(CoreError::Invalid);
        }
        Ok((page, size))
    }

    pub(crate) fn search(&self) -> Option<&str> {
        self.search.as_deref().map(str::trim).filter(|value| {
            !value.is_empty()
                && !value.eq_ignore_ascii_case("null")
                && !value.eq_ignore_ascii_case("undefined")
        })
    }
}

#[derive(Debug, Serialize)]
pub struct SupplierPage<T = Supplier> {
    pub items: Vec<T>,
    pub page: u32,
    pub page_size: u32,
    pub has_more: bool,
}
