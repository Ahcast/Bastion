#![forbid(unsafe_code)]

mod forms;
mod policy;
pub use forms::{
    FormDefinition, FormSection, FormValues, FormVariable, PublishFormInput, PublishedForm,
    VariableKind,
};
mod service;
pub use policy::FilePolicy;
pub use service::CollaborationService;

use base64::{Engine, engine::general_purpose::STANDARD};
use bastion_core::CoreError;
use serde::{Deserialize, Serialize};

pub const MAX_FILE_BYTES: usize = bastion_core::MAX_PRIVATE_FILE_BYTES;

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UploadInput {
    pub scope: String,
    pub id_persona: Option<u64>,
    pub title: String,
    #[serde(default)]
    pub notes: String,
    pub filename: String,
    pub content_base64: String,
}
impl UploadInput {
    pub fn validate(&mut self, max_bytes: usize) -> Result<Vec<u8>, CoreError> {
        folder(&self.scope, self.id_persona)?;
        self.title = self.title.trim().to_owned();
        self.notes = self.notes.trim().to_owned();
        if self.title.is_empty()
            || self.filename.is_empty()
            || self.title.chars().count() > 255
            || self.filename.chars().count() > 255
            || self.notes.chars().count() > 4000
            || self.title.chars().any(char::is_control)
            || self
                .filename
                .chars()
                .any(|c| c.is_control() || matches!(c, '/' | '\\' | ':'))
            || matches!(self.filename.as_str(), "." | "..")
            || self
                .notes
                .chars()
                .any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t'))
            || max_bytes > MAX_FILE_BYTES
            || self.content_base64.len() > max_bytes.div_ceil(3) * 4
        {
            return Err(CoreError::Invalid);
        }
        let bytes = STANDARD
            .decode(&self.content_base64)
            .map_err(|_| CoreError::Invalid)?;
        if bytes.is_empty() || bytes.len() > max_bytes {
            return Err(CoreError::Invalid);
        }
        Ok(bytes)
    }
}
pub fn folder(scope: &str, person: Option<u64>) -> Result<String, CoreError> {
    match (scope, person) {
        ("empresa", None) => Ok("empresa".into()),
        ("persona", Some(id)) if id > 0 => Ok(format!("persona:{id}")),
        _ => Err(CoreError::Invalid),
    }
}
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileFilter {
    #[serde(default)]
    pub trash: bool,
    pub scope: Option<String>,
    pub id_persona: Option<u64>,
    pub search: Option<String>,
    pub page: Option<u32>,
    pub page_size: Option<u32>,
}
impl FileFilter {
    pub fn bounds(&self) -> Result<(u32, u32), CoreError> {
        let page = self.page.unwrap_or(1);
        let size = self.page_size.unwrap_or(25);
        if !(1..=10000).contains(&page)
            || !(1..=100).contains(&size)
            || self
                .search
                .as_ref()
                .is_some_and(|s| s.chars().count() > 200)
            || self
                .scope
                .as_ref()
                .is_some_and(|s| !matches!(s.as_str(), "empresa" | "persona"))
            || self.id_persona == Some(0)
            || (self.id_persona.is_some() && self.scope.as_deref() != Some("persona"))
        {
            return Err(CoreError::Invalid);
        }
        Ok((page, size))
    }
}
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct FileRecord {
    pub id: u64,
    pub scope: String,
    pub id_persona: Option<u64>,
    pub title: String,
    pub notes: String,
    pub filename: String,
    pub size_bytes: u64,
    pub mime: String,
    pub trashed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub purge_after: Option<chrono::DateTime<chrono::Utc>>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}
#[derive(Serialize)]
pub struct FilePage<T = FileRecord> {
    pub items: Vec<T>,
    pub page: u32,
    pub page_size: u32,
    pub has_more: bool,
}
#[derive(Serialize)]
pub struct FileContent {
    pub filename: String,
    pub mime: &'static str,
    pub content_base64: String,
}
// Only inert text is rendered by the frontend; HTML and SVG never get an active origin.
pub fn preview_mime(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"%PDF-") {
        Some("application/pdf")
    } else if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if bytes.starts_with(b"\xff\xd8\xff") {
        Some("image/jpeg")
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some("image/gif")
    } else if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
        Some("image/webp")
    } else if bytes.starts_with(b"BM") {
        Some("image/bmp")
    } else if std::str::from_utf8(bytes).is_ok()
        && !bytes.iter().any(|b| *b < 32 && !matches!(*b, 9 | 10 | 13))
    {
        Some("text/plain")
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn folder_and_files_reject_cross_scope_mass_assignment_and_paths() {
        assert_eq!(folder("persona", Some(9)).unwrap(), "persona:9");
        for (scope, id) in [
            ("empresa", Some(1)),
            ("persona", None),
            ("persona", Some(0)),
            ("*", None),
        ] {
            assert!(folder(scope, id).is_err());
        }
        for name in ["../a", "a\\b", "a\r\nb", "C:foo", ".."] {
            let mut input = UploadInput {
                scope: "empresa".into(),
                id_persona: None,
                title: "Contrato".into(),
                notes: String::new(),
                filename: name.into(),
                content_base64: STANDARD.encode(b"a"),
            };
            assert!(input.validate(MAX_FILE_BYTES).is_err());
        }
        assert!(serde_json::from_value::<UploadInput>(serde_json::json!({"scope":"empresa","title":"a","filename":"a","content_base64":"YQ==","object_key":"secret"})).is_err());
        assert!(
            FileFilter {
                scope: Some("empresa".into()),
                id_persona: Some(1),
                ..Default::default()
            }
            .bounds()
            .is_err()
        );
    }
    #[test]
    fn preview_does_not_interpret_user_supplied_mime_or_markup() {
        assert_eq!(preview_mime(b"<svg onload='alert(1)'>"), Some("text/plain"));
        assert_eq!(preview_mime(b"%PDF-1.7"), Some("application/pdf"));
        assert_eq!(preview_mime(b"\0binary"), None);
    }
}
