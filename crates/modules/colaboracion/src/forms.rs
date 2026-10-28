//! Versioned capture schemas only. Publishing never authorizes executable templates.
use crate::{CollaborationService, FilePage};
use bastion_core::{CoreAdmin, CoreError};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{MySql, Transaction};
use std::collections::{BTreeMap, BTreeSet};

pub type FormValues = BTreeMap<String, serde_json::Value>;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FormSection {
    Legal,
    Economica,
    Tecnica,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VariableKind {
    Text,
    Integer,
    Decimal,
    Date,
    Boolean,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FormVariable {
    pub key: String,
    pub label: String,
    pub kind: VariableKind,
    pub required: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FormDefinition {
    pub title: String,
    pub section: FormSection,
    pub variables: Vec<FormVariable>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub template_source_id: Option<u64>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublishedForm {
    pub id: u64,
    pub revision: u64,
    pub hash: String,
    pub definition: FormDefinition,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublishFormInput {
    pub form_id: Option<u64>,
    pub expected_revision: Option<u64>,
    pub definition: FormDefinition,
}

fn label_ok(s: &str) -> bool {
    !s.trim().is_empty() && s.chars().count() <= 255 && !s.chars().any(char::is_control)
}
impl FormDefinition {
    pub fn validate(&self) -> Result<(), CoreError> {
        if !label_ok(&self.title)
            || self.variables.len() > 100
            || self.template_source_id == Some(0)
        {
            return Err(CoreError::Invalid);
        }
        let mut keys = BTreeSet::new();
        for v in &self.variables {
            if v.key.is_empty()
                || v.key.len() > 64
                || !v.key.as_bytes()[0].is_ascii_lowercase()
                || !v
                    .key
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
                || !keys.insert(&v.key)
                || !label_ok(&v.label)
            {
                return Err(CoreError::Invalid);
            }
        }
        Ok(())
    }
    /// Values are inert data. Numeric values stay strings, never IEEE floats.
    /// Incomplete drafts are allowed; callers display missing required keys.
    pub fn validate_values(&self, values: &FormValues) -> Result<Vec<String>, CoreError> {
        self.validate()?;
        for (key, value) in values {
            let variable = self
                .variables
                .iter()
                .find(|v| &v.key == key)
                .ok_or(CoreError::Invalid)?;
            if value.as_str() == Some("") {
                continue;
            }
            let valid = match variable.kind {
                VariableKind::Boolean => value.is_boolean(),
                VariableKind::Text => value.as_str().is_some_and(|s| {
                    s.chars().count() <= 4000
                        && !s
                            .chars()
                            .any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t'))
                }),
                VariableKind::Integer => value.as_str().is_some_and(integer),
                VariableKind::Decimal => value.as_str().is_some_and(decimal),
                VariableKind::Date => value.as_str().is_some_and(|s| {
                    s.len() == 10 && chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").is_ok()
                }),
            };
            if !valid {
                return Err(CoreError::Invalid);
            }
        }
        Ok(self
            .variables
            .iter()
            .filter(|v| v.required && values.get(&v.key).is_none_or(|v| v.as_str() == Some("")))
            .map(|v| v.key.clone())
            .collect())
    }
    pub fn hash(&self) -> Result<String, CoreError> {
        Ok(format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(self).map_err(|_| CoreError::Invalid)?)
        ))
    }
}
fn integer(s: &str) -> bool {
    let unsigned = s.strip_prefix('-').unwrap_or(s);
    !unsigned.is_empty() && unsigned.len() <= 30 && unsigned.bytes().all(|b| b.is_ascii_digit())
}
fn decimal(s: &str) -> bool {
    let unsigned = s.strip_prefix('-').unwrap_or(s);
    let mut pieces = unsigned.split('.');
    let whole = pieces.next().unwrap_or("");
    !whole.is_empty()
        && whole.len() <= 30
        && whole.bytes().all(|b| b.is_ascii_digit())
        && pieces
            .next()
            .is_none_or(|f| !f.is_empty() && f.len() <= 12 && f.bytes().all(|b| b.is_ascii_digit()))
        && pieces.next().is_none()
}
fn key_hash(key: &str) -> Result<Vec<u8>, CoreError> {
    if !(16..=128).contains(&key.len())
        || !key
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_'))
    {
        return Err(CoreError::Invalid);
    }
    Ok(Sha256::digest(key.as_bytes()).to_vec())
}

impl CollaborationService {
    pub async fn list_forms(
        &self,
        actor: u64,
        page: u32,
        size: u32,
    ) -> Result<FilePage<PublishedForm>, CoreError> {
        if !(1..=10000).contains(&page) || !(1..=100).contains(&size) {
            return Err(CoreError::Invalid);
        }
        let mut tx = self.begin().await?;
        self.admin
            .authorize_in_transaction(&mut tx, actor, "colaboracion", "empresa", "ver")
            .await?;
        let rows:Vec<(u64,u64,String,serde_json::Value)>=sqlx::query_as("SELECT v.form_id,v.revision,v.content_hash,v.definition_json FROM colaboracion_formularios f JOIN colaboracion_formulario_versiones v ON v.form_id=f.id AND v.revision=f.current_revision ORDER BY f.id DESC LIMIT ? OFFSET ?")
            .bind(size+1).bind((page-1)*size).fetch_all(&mut *tx).await?;
        let has_more = rows.len() > size as usize;
        let items = rows
            .into_iter()
            .take(size as usize)
            .map(|(id, revision, hash, value)| decode(id, revision, hash, value))
            .collect::<Result<Vec<_>, _>>()?;
        tx.commit().await?;
        Ok(FilePage {
            items,
            page,
            page_size: size,
            has_more,
        })
    }
    /// Public owner port. Caller locks Licitaciones before Colaboración; no SQL from the caller.
    pub async fn form_snapshot_in_transaction(
        &self,
        tx: &mut Transaction<'_, MySql>,
        actor: u64,
        id: u64,
        revision: u64,
    ) -> Result<PublishedForm, CoreError> {
        let enabled:Option<bool>=sqlx::query_scalar("SELECT (included_in_release AND environment_allowed AND enabled) FROM module_registry WHERE module_id='colaboracion' FOR UPDATE").fetch_optional(&mut **tx).await?;
        if enabled != Some(true) {
            return Err(CoreError::NotFound);
        }
        self.admin
            .authorize_in_transaction(tx, actor, "colaboracion", "empresa", "ver")
            .await?;
        let (hash,value):(String,serde_json::Value)=sqlx::query_as("SELECT content_hash,definition_json FROM colaboracion_formulario_versiones WHERE form_id=? AND revision=?")
            .bind(id).bind(revision).fetch_optional(&mut **tx).await?.ok_or(CoreError::NotFound)?;
        decode(id, revision, hash, value)
    }
    pub async fn publish_form(
        &self,
        actor: u64,
        key: &str,
        input: PublishFormInput,
    ) -> Result<PublishedForm, CoreError> {
        input.definition.validate()?;
        let key_hash = key_hash(key)?;
        let request_hash =
            Sha256::digest(serde_json::to_vec(&input).map_err(|_| CoreError::Invalid)?).to_vec();
        let mut tx = self.begin().await?;
        self.admin
            .require_system_administrator_in_transaction(&mut tx, actor)
            .await?;
        let receipt:Option<(Vec<u8>,serde_json::Value)>=sqlx::query_as("SELECT request_hash,response_json FROM colaboracion_formulario_recibos WHERE actor_id=? AND key_hash=?")
            .bind(actor).bind(&key_hash).fetch_optional(&mut *tx).await?;
        if let Some((old, value)) = receipt {
            if old != request_hash {
                return Err(CoreError::Conflict);
            }
            let result = serde_json::from_value(value).map_err(|_| CoreError::Invalid)?;
            tx.commit().await?;
            return Ok(result);
        }
        if let Some(source_id) = input.definition.template_source_id {
            let (source, _) = self
                .file_metadata_in_transaction(&mut tx, actor, source_id)
                .await?;
            if !source.filename.to_ascii_lowercase().ends_with(".typ")
                || source.mime != "text/plain"
            {
                return Err(CoreError::Invalid);
            }
        }
        let (id, revision) = match (input.form_id, input.expected_revision) {
            (None, None) => {
                let id = sqlx::query(
                    "INSERT INTO colaboracion_formularios (current_revision) VALUES (1)",
                )
                .execute(&mut *tx)
                .await?
                .last_insert_id();
                (id, 1)
            }
            (Some(id), Some(expected)) if id > 0 && expected > 0 => {
                let current: u64 = sqlx::query_scalar(
                    "SELECT current_revision FROM colaboracion_formularios WHERE id=? FOR UPDATE",
                )
                .bind(id)
                .fetch_optional(&mut *tx)
                .await?
                .ok_or(CoreError::NotFound)?;
                if current != expected {
                    return Err(CoreError::PreconditionFailed);
                }
                let revision = current.checked_add(1).ok_or(CoreError::Invalid)?;
                sqlx::query("UPDATE colaboracion_formularios SET current_revision=? WHERE id=?")
                    .bind(revision)
                    .bind(id)
                    .execute(&mut *tx)
                    .await?;
                (id, revision)
            }
            _ => return Err(CoreError::Invalid),
        };
        let hash = input.definition.hash()?;
        sqlx::query("INSERT INTO colaboracion_formulario_versiones (form_id,revision,definition_json,content_hash,published_by) VALUES (?,?,?,?,?)")
            .bind(id).bind(revision).bind(serde_json::to_value(&input.definition).map_err(|_|CoreError::Invalid)?).bind(&hash).bind(actor).execute(&mut *tx).await?;
        let result = PublishedForm {
            id,
            revision,
            hash,
            definition: input.definition,
        };
        sqlx::query("INSERT INTO colaboracion_formulario_recibos (actor_id,key_hash,request_hash,response_json) VALUES (?,?,?,?)")
            .bind(actor).bind(key_hash).bind(request_hash).bind(serde_json::to_value(&result).map_err(|_|CoreError::Invalid)?).execute(&mut *tx).await?;
        CoreAdmin::record_domain_event(
            &mut tx,
            actor,
            "colaboracion",
            "form.publish",
            &id.to_string(),
            serde_json::json!({"revision":revision}),
        )
        .await?;
        tx.commit().await?;
        Ok(result)
    }
}
fn decode(
    id: u64,
    revision: u64,
    hash: String,
    value: serde_json::Value,
) -> Result<PublishedForm, CoreError> {
    let definition: FormDefinition =
        serde_json::from_value(value).map_err(|_| CoreError::Invalid)?;
    definition.validate()?;
    if definition.hash()? != hash {
        return Err(CoreError::Conflict);
    }
    Ok(PublishedForm {
        id,
        revision,
        hash,
        definition,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn schema(kind: VariableKind) -> FormDefinition {
        FormDefinition {
            template_source_id: None,
            title: "Sintético".into(),
            section: FormSection::Legal,
            variables: vec![FormVariable {
                key: "dato".into(),
                label: "Dato".into(),
                kind,
                required: true,
            }],
        }
    }
    #[test]
    fn schemas_reject_duplicates_unknown_fields_and_executable_attributes() {
        let mut s = schema(VariableKind::Text);
        s.variables.push(s.variables[0].clone());
        assert!(s.validate().is_err());
        assert!(serde_json::from_value::<FormDefinition>(serde_json::json!({"title":"X","section":"legal","variables":[],"typst":"#read(\"secret\")"})).is_err());
        assert!(
            schema(VariableKind::Text)
                .validate_values(&BTreeMap::from([("other".into(), serde_json::json!("x"))]))
                .is_err()
        );
    }
    #[test]
    fn typed_values_preserve_exact_decimals_and_report_missing() {
        let s = schema(VariableKind::Decimal);
        assert_eq!(s.validate_values(&FormValues::new()).unwrap(), vec!["dato"]);
        for value in [
            serde_json::json!(1.2),
            serde_json::json!("NaN"),
            serde_json::json!("1e3"),
            serde_json::json!(".5"),
            serde_json::json!("1."),
        ] {
            assert!(
                s.validate_values(&BTreeMap::from([("dato".into(), value)]))
                    .is_err()
            );
        }
        assert!(
            s.validate_values(&BTreeMap::from([(
                "dato".into(),
                serde_json::json!("999999999999999999.123456789012")
            )]))
            .unwrap()
            .is_empty()
        );
        assert!(
            schema(VariableKind::Date)
                .validate_values(&BTreeMap::from([(
                    "dato".into(),
                    serde_json::json!("2026-02-30")
                )]))
                .is_err()
        );
        assert!(
            schema(VariableKind::Boolean)
                .validate_values(&BTreeMap::from([("dato".into(), serde_json::json!(false))]))
                .unwrap()
                .is_empty()
        );
    }
}
