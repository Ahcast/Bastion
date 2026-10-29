use crate::{
    TenderTypstSnapshot,
    pdf_layout::{LayoutEntry, LayoutRequest},
};
use bastion_core::CoreError;
use std::collections::BTreeSet;

/// Identifiers only: the service must authorize and stage each source again
/// before starting the isolated worker. No caller-supplied paths are accepted.
#[derive(Debug, PartialEq, Eq)]
pub enum RenderPiece {
    Offer {
        number: u32,
        style: String,
    },
    Form {
        number: u32,
        form_id: u64,
        template_id: u64,
    },
    Lot {
        number: u32,
        lot: u32,
        style: String,
    },
    Attachment {
        number: u32,
        id: String,
    },
    InitialForm {
        number: u32,
        kind: String,
    },
}
pub struct RenderPlan {
    pub layout: LayoutRequest,
    pub pieces: Vec<RenderPiece>,
}

impl TenderTypstSnapshot {
    pub fn render_plan(&self) -> Result<RenderPlan, CoreError> {
        let value = |key: &str| -> Result<&str, CoreError> {
            self.expediente
                .variables
                .iter()
                .find(|v| v.key == key)
                .and_then(|v| v.value.as_ref())
                .and_then(|v| v.as_str())
                .filter(|v| !v.trim().is_empty())
                .ok_or(CoreError::Invalid)
        };
        let style = value("stylo")?;
        if !matches!(style, "bid" | "std") || self.order.len() > 3998 {
            return Err(CoreError::Invalid);
        }
        let mut seen = BTreeSet::new();
        let mut entries = Vec::with_capacity(self.order.len());
        let mut pieces = Vec::with_capacity(self.order.len());
        for (index, item) in self.order.iter().enumerate() {
            if !seen.insert(item.key.as_str())
                || item.label.trim().is_empty()
                || item.label.chars().count() > 1000
            {
                return Err(CoreError::Invalid);
            }
            let number = u32::try_from(index).map_err(|_| CoreError::Invalid)? + 3;
            let piece = if item.key == "oferta" {
                RenderPiece::Offer {
                    number,
                    style: style.into(),
                }
            } else if let Some(kind) = item.key.strip_prefix("inicial:") {
                if !matches!(kind, "integridad" | "oferente")
                    || !self.expediente.initial_forms.iter().any(|v| v == kind)
                {
                    return Err(CoreError::Invalid);
                }
                RenderPiece::InitialForm {
                    number,
                    kind: kind.into(),
                }
            } else if let Some(raw) = item.key.strip_prefix("formulario:") {
                let form_id = raw.parse::<u64>().map_err(|_| CoreError::Invalid)?;
                if form_id.to_string() != raw {
                    return Err(CoreError::Invalid);
                }
                let form = self
                    .expediente
                    .forms
                    .iter()
                    .find(|f| f.id == form_id)
                    .ok_or(CoreError::Invalid)?;
                let template_id = form
                    .source
                    .definition
                    .template_source_id
                    .ok_or(CoreError::Invalid)?;
                RenderPiece::Form {
                    number,
                    form_id,
                    template_id,
                }
            } else if let Some(raw) = item.key.strip_prefix("lote:") {
                let lot = raw.parse::<u32>().map_err(|_| CoreError::Invalid)?;
                if lot.to_string() != raw || !self.sources.offered_lots.contains(&lot) {
                    return Err(CoreError::Invalid);
                }
                RenderPiece::Lot {
                    number,
                    lot,
                    style: style.into(),
                }
            } else if let Some(id) = item.key.strip_prefix("archivo:") {
                if id.len() != 48
                    || !id
                        .bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
                {
                    return Err(CoreError::Invalid);
                }
                RenderPiece::Attachment {
                    number,
                    id: id.into(),
                }
            } else {
                return Err(CoreError::Invalid);
            };
            entries.push(LayoutEntry {
                key: item.key.clone(),
                title: item.label.clone(),
            });
            pieces.push(piece);
        }
        if !seen.contains("oferta")
            || self
                .expediente
                .initial_forms
                .iter()
                .any(|kind| !seen.contains(format!("inicial:{kind}").as_str()))
            || self
                .sources
                .offered_lots
                .iter()
                .any(|lot| !seen.contains(format!("lote:{lot}").as_str()))
        {
            return Err(CoreError::Invalid);
        }
        Ok(RenderPlan {
            layout: LayoutRequest {
                expediente: self.expediente.name.clone(),
                lpn: value("lpn")?.into(),
                institucion: value("inst")?.into(),
                entries,
            },
            pieces,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Expedient, GlobalVariable, PdfOrderItem, TypstLotSources};
    use bastion_colaboracion::VariableKind;
    use std::collections::BTreeMap;

    fn snapshot() -> TenderTypstSnapshot {
        let variables = [("stylo", "bid"), ("lpn", "LPN-1"), ("inst", "Institución")]
            .into_iter()
            .map(|(key, value)| GlobalVariable {
                key: key.into(),
                label: key.into(),
                kind: VariableKind::Text,
                value: Some(serde_json::json!(value)),
            })
            .collect();
        TenderTypstSnapshot {
            expediente: Expedient {
                id: 1,
                name: "Expediente".into(),
                version: 1,
                state: "borrador".into(),
                forms: vec![],
                variables,
                initial_forms: vec![],
            },
            order: vec![
                PdfOrderItem {
                    key: "lote:2".into(),
                    label: "Lote 2".into(),
                },
                PdfOrderItem {
                    key: "oferta".into(),
                    label: "Carta".into(),
                },
                PdfOrderItem {
                    key: format!("archivo:{}", "a".repeat(48)),
                    label: "Anexo".into(),
                },
            ],
            sources: TypstLotSources {
                files: BTreeMap::new(),
                offered_lots: vec![2],
            },
        }
    }
    #[test]
    fn maps_the_saved_order_to_worker_piece_numbers() {
        let plan = snapshot().render_plan().unwrap();
        assert_eq!(
            plan.layout
                .entries
                .iter()
                .map(|e| e.key.as_str())
                .collect::<Vec<_>>(),
            vec!["lote:2", "oferta", &format!("archivo:{}", "a".repeat(48))]
        );
        assert!(matches!(
            plan.pieces[0],
            RenderPiece::Lot {
                number: 3,
                lot: 2,
                ..
            }
        ));
        assert!(matches!(
            plan.pieces[1],
            RenderPiece::Offer { number: 4, .. }
        ));
        assert!(matches!(
            plan.pieces[2],
            RenderPiece::Attachment { number: 5, .. }
        ));
    }
    #[test]
    fn rejects_empty_lots_missing_offers_and_unsafe_identifiers() {
        let mut snap = snapshot();
        snap.order[0].key = "lote:1".into();
        assert!(snap.render_plan().is_err());
        let mut snap = snapshot();
        snap.order.remove(1);
        assert!(snap.render_plan().is_err());
        let mut snap = snapshot();
        snap.order[2].key = "archivo:../../secret".into();
        assert!(snap.render_plan().is_err());
    }
}
