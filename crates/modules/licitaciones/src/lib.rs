#![forbid(unsafe_code)]

mod attachments;
mod folio;
mod frontmatter;
mod globals_typst;
pub mod image_pdf;
mod lots;
mod order;
mod pagination;
pub mod pdf_compose;
mod pdf_jobs;
pub mod pdf_layout;
mod pdf_stage;
mod render_plan;
mod render_template;
mod service;
mod typst_data;
pub use attachments::{
    Attachment, AttachmentContent, AttachmentCopy, AttachmentDelete, AttachmentList,
    AttachmentSection, AttachmentUpload, RenderAttachment,
};
use bastion_colaboracion::{
    FormDefinition, FormSection, FormValues, FormVariable, PublishedForm, VariableKind,
};
use bastion_core::CoreError;
pub use folio::{FolioPage, parse_pdfinfo_pages, prepare_folio_sources};
pub use frontmatter::prepare_frontmatter_sources;
pub use globals_typst::{bind_global_typst_source, prepare_global_typst_sources};
pub use lots::{Lot, LotInput, LotItem, LotSummary};
pub use order::{PdfOrder, PdfOrderInput, PdfOrderItem};
pub use pagination::{MeasuredPiece, PageConvergence, PagePlan, PageRange, plan_pdf_pages};
pub use pdf_jobs::PdfJob;
pub use pdf_stage::{TenderTemplateBundle, WorkerRequest};
pub use render_plan::{RenderPiece, RenderPlan};
pub use render_template::RenderTemplate;
use serde::{Deserialize, Serialize};
pub use service::TenderService;
pub use typst_data::{TenderTypstSnapshot, TypstLotSources, prepare_typst_lot_sources};

pub const MAX_INPUT_BYTES: u64 = 100_000_000;
pub const MAX_EXPEDIENT_BYTES: u64 = 1_000_000_000;
pub const MAX_FINAL_PDF_BYTES: u64 = 1_000_000_000;

/// Independent budgets; revisions count until an explicit retention policy exists.
pub fn validate_input_budget(retained_bytes: u64, actual_bytes: u64) -> Result<u64, CoreError> {
    if actual_bytes == 0 || actual_bytes > MAX_INPUT_BYTES {
        return Err(CoreError::Invalid);
    }
    retained_bytes
        .checked_add(actual_bytes)
        .filter(|v| *v <= MAX_EXPEDIENT_BYTES)
        .ok_or(CoreError::Invalid)
}
pub fn validate_final_pdf_budget(actual_bytes: u64) -> Result<(), CoreError> {
    if actual_bytes == 0 || actual_bytes > MAX_FINAL_PDF_BYTES {
        return Err(CoreError::Invalid);
    }
    Ok(())
}
#[derive(Clone, Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct ExpedientSummary {
    pub id: u64,
    #[serde(default)]
    pub name: String,
    pub version: u64,
    pub state: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FormCopy {
    pub id: u64,
    pub source: PublishedForm,
    pub values: FormValues,
    #[serde(default)]
    pub legacy_values: FormValues,
    pub missing_required: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Expedient {
    pub id: u64,
    // Old persistent idempotency receipts predate the editable name.
    #[serde(default)]
    pub name: String,
    pub version: u64,
    pub state: String,
    pub forms: Vec<FormCopy>,
    #[serde(default)]
    pub variables: Vec<GlobalVariable>,
    #[serde(default)]
    pub initial_forms: Vec<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InitialFormsInput {
    pub forms: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GlobalVariable {
    pub key: String,
    pub label: String,
    pub kind: VariableKind,
    pub value: Option<serde_json::Value>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GlobalVariablesInput {
    pub variables: Vec<GlobalVariable>,
}
impl GlobalVariablesInput {
    pub fn validate(&self) -> Result<(), CoreError> {
        let schema = FormDefinition {
            title: "Variables".into(),
            section: FormSection::Legal,
            template_source_id: None,
            variables: self
                .variables
                .iter()
                .map(|v| FormVariable {
                    key: v.key.clone(),
                    label: v.label.clone(),
                    kind: v.kind,
                    required: false,
                })
                .collect(),
        };
        let values = self
            .variables
            .iter()
            .filter_map(|v| v.value.clone().map(|value| (v.key.clone(), value)))
            .collect();
        schema.validate_values(&values)?;
        Ok(())
    }
    pub fn validate_form_config(&self) -> Result<(), CoreError> {
        let months = [
            "enero",
            "febrero",
            "marzo",
            "abril",
            "mayo",
            "junio",
            "julio",
            "agosto",
            "septiembre",
            "octubre",
            "noviembre",
            "diciembre",
        ];
        for variable in &self.variables {
            if !matches!(
                variable.key.as_str(),
                "lpn" | "inst" | "descripcion" | "mes" | "stylo" | "dia" | "year" | "lotes"
            ) {
                continue;
            }
            let Some(value) = &variable.value else {
                continue;
            };
            let text = value.as_str().ok_or(CoreError::Invalid)?;
            let valid = match variable.key.as_str() {
                "lpn" | "inst" | "descripcion" => {
                    variable.kind == VariableKind::Text && !text.trim().is_empty()
                }
                "mes" => variable.kind == VariableKind::Text && months.contains(&text),
                "stylo" => variable.kind == VariableKind::Text && matches!(text, "bid" | "std"),
                "dia" => {
                    variable.kind == VariableKind::Integer
                        && text.parse::<u32>().is_ok_and(|n| (1..=31).contains(&n))
                }
                "year" => {
                    variable.kind == VariableKind::Integer
                        && text.parse::<i32>().is_ok_and(|n| (1..=9999).contains(&n))
                }
                "lotes" => {
                    variable.kind == VariableKind::Integer
                        && text.parse::<u32>().is_ok_and(|n| (1..=1000).contains(&n))
                }
                _ => true,
            };
            if !valid {
                return Err(CoreError::Invalid);
            }
        }
        let get = |key: &str| {
            self.variables
                .iter()
                .find(|v| v.key == key)
                .and_then(|v| v.value.as_ref())
                .and_then(|v| v.as_str())
        };
        if let (Some(day), Some(month), Some(year)) = (get("dia"), get("mes"), get("year")) {
            let month = months
                .iter()
                .position(|m| *m == month)
                .ok_or(CoreError::Invalid)? as u32
                + 1;
            let day = day.parse::<u32>().map_err(|_| CoreError::Invalid)?;
            let year = year.parse::<i32>().map_err(|_| CoreError::Invalid)?;
            if chrono::NaiveDate::from_ymd_opt(year, month, day).is_none() {
                return Err(CoreError::Invalid);
            }
        }
        Ok(())
    }
}
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PageQuery {
    pub page: Option<u32>,
    pub page_size: Option<u32>,
}
impl PageQuery {
    pub fn bounds(&self) -> Result<(u32, u32), CoreError> {
        let page = self.page.unwrap_or(1);
        let size = self.page_size.unwrap_or(25);
        if !(1..=10000).contains(&page) || !(1..=100).contains(&size) {
            return Err(CoreError::Invalid);
        }
        Ok((page, size))
    }
}
#[derive(Serialize)]
pub struct ExpedientPage {
    pub items: Vec<ExpedientSummary>,
    pub page: u32,
    pub page_size: u32,
    pub has_more: bool,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CopyFormInput {
    pub source_id: u64,
    pub revision: u64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ValuesInput {
    pub values: FormValues,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EmptyInput {}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NameInput {
    pub name: String,
}
impl NameInput {
    pub fn normalize(self) -> Result<String, CoreError> {
        if self.name.chars().any(char::is_control) {
            return Err(CoreError::Invalid);
        }
        let name = self.name.trim();
        if name.is_empty() || name.chars().count() > 255 {
            return Err(CoreError::Invalid);
        }
        Ok(name.to_owned())
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeleteInput {
    pub confirm_id: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn editable_names_are_inert_bounded_and_compatible_with_old_receipts() {
        assert_eq!(
            NameInput {
                name: "  Oferta de equipos  ".into()
            }
            .normalize()
            .unwrap(),
            "Oferta de equipos"
        );
        assert!(
            NameInput {
                name: "é".repeat(255)
            }
            .normalize()
            .is_ok()
        );
        for name in [
            "".to_string(),
            "   ".into(),
            "x".repeat(256),
            "a\nb".into(),
            "a\0b".into(),
        ] {
            assert!(NameInput { name }.normalize().is_err());
        }
        assert!(
            serde_json::from_str::<NameInput>(r#"{"name":"Oferta","state":"enviada"}"#).is_err()
        );
        let old: Expedient =
            serde_json::from_str(r#"{"id":1,"version":1,"state":"borrador","forms":[]}"#).unwrap();
        assert!(old.name.is_empty());
    }
    #[test]
    fn budgets_keep_decimal_mb_and_final_pdf_separate() {
        assert_eq!(
            validate_input_budget(900_000_000, 100_000_000).unwrap(),
            1_000_000_000
        );
        assert!(validate_input_budget(900_000_001, 100_000_000).is_err());
        assert!(validate_input_budget(0, 100_000_001).is_err());
        assert!(validate_input_budget(u64::MAX, 1).is_err());
        assert!(validate_input_budget(0, 0).is_err());
        assert!(validate_final_pdf_budget(1_000_000_000).is_ok());
        assert!(validate_final_pdf_budget(1_000_000_001).is_err());
    }
    #[test]
    fn global_variables_are_typed_unique_and_inert() {
        let variable = GlobalVariable {
            key: "nombre".into(),
            label: "Nombre".into(),
            kind: VariableKind::Text,
            value: Some(serde_json::json!("<script>")),
        };
        assert!(
            GlobalVariablesInput {
                variables: vec![variable.clone()]
            }
            .validate()
            .is_ok()
        );
        assert!(
            GlobalVariablesInput {
                variables: vec![variable.clone(), variable]
            }
            .validate()
            .is_err()
        );
        assert!(
            GlobalVariablesInput {
                variables: vec![GlobalVariable {
                    key: "Monto".into(),
                    label: "Monto".into(),
                    kind: VariableKind::Decimal,
                    value: Some(serde_json::json!(1.5))
                }]
            }
            .validate()
            .is_err()
        );
    }
    #[test]
    fn form_config_validates_calendar_and_enumerations() {
        let variable = |key: &str, kind, value: &str| GlobalVariable {
            key: key.into(),
            label: key.into(),
            kind,
            value: Some(serde_json::json!(value)),
        };
        let invalid = GlobalVariablesInput {
            variables: vec![
                variable("dia", VariableKind::Integer, "29"),
                variable("mes", VariableKind::Text, "febrero"),
                variable("year", VariableKind::Integer, "2025"),
            ],
        };
        assert!(invalid.validate_form_config().is_err());
        assert!(
            GlobalVariablesInput {
                variables: vec![variable("stylo", VariableKind::Text, "other")]
            }
            .validate_form_config()
            .is_err()
        );
        assert!(
            GlobalVariablesInput {
                variables: vec![variable("mes", VariableKind::Text, "abril")]
            }
            .validate_form_config()
            .is_ok()
        );
    }
    #[test]
    fn clients_cannot_assign_state_schema_or_business_fields() {
        assert!(serde_json::from_str::<EmptyInput>(r#"{"state":"enviada"}"#).is_err());
        assert!(serde_json::from_str::<EmptyInput>(r#"{"nombre":"inventado"}"#).is_err());
        assert!(serde_json::from_str::<ValuesInput>(r#"{"values":{},"schema":{}}"#).is_err());
        assert!(
            PageQuery {
                page: Some(u32::MAX),
                page_size: None
            }
            .bounds()
            .is_err()
        );
    }
}
