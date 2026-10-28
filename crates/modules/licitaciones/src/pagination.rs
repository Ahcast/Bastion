use bastion_core::CoreError;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Page counts come from inspecting the rendered PDFs, never from filenames or
/// client input. The order must be the authenticated expediente's saved order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MeasuredPiece {
    pub key: String,
    pub title: String,
    pub pages: u32,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PageRange {
    pub key: String,
    pub title: String,
    pub first_page: u32,
    pub last_page: u32,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PagePlan {
    pub total_pages: u32,
    pub cover_pages: u32,
    pub index_pages: u32,
    pub entries: Vec<PageRange>,
}

pub fn plan_pdf_pages(
    cover_pages: u32,
    index_pages: u32,
    pieces: &[MeasuredPiece],
) -> Result<PagePlan, CoreError> {
    if cover_pages == 0 || index_pages == 0 {
        return Err(CoreError::Invalid);
    }
    let mut cursor = cover_pages
        .checked_add(index_pages)
        .ok_or(CoreError::Invalid)?;
    let mut keys = BTreeSet::new();
    let mut entries = Vec::with_capacity(pieces.len());
    for piece in pieces {
        if piece.pages == 0
            || piece.key.is_empty()
            || piece.title.trim().is_empty()
            || !keys.insert(&piece.key)
        {
            return Err(CoreError::Invalid);
        }
        let first_page = cursor.checked_add(1).ok_or(CoreError::Invalid)?;
        cursor = cursor.checked_add(piece.pages).ok_or(CoreError::Invalid)?;
        entries.push(PageRange {
            key: piece.key.clone(),
            title: piece.title.clone(),
            first_page,
            last_page: cursor,
        });
    }
    Ok(PagePlan {
        total_pages: cursor,
        cover_pages,
        index_pages,
        entries,
    })
}

/// An index or a displayed total can itself change pagination. Only return a
/// plan after two measured iterations agree; cycles and runaway layout fail.
#[derive(Default)]
pub struct PageConvergence {
    seen: Vec<PagePlan>,
}
impl PageConvergence {
    pub fn observe(&mut self, plan: PagePlan) -> Result<Option<PagePlan>, CoreError> {
        if self.seen.last() == Some(&plan) {
            return Ok(Some(plan));
        }
        if self.seen.len() >= 8 || self.seen.contains(&plan) {
            return Err(CoreError::Conflict);
        }
        self.seen.push(plan);
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn piece(key: &str, pages: u32) -> MeasuredPiece {
        MeasuredPiece {
            key: key.into(),
            title: key.into(),
            pages,
        }
    }
    #[test]
    fn pages_include_cover_index_and_every_attachment_in_selected_order() {
        let plan = plan_pdf_pages(
            1,
            2,
            &[
                piece("archivo:b", 5),
                piece("lote:2", 2),
                piece("formulario:4", 1),
            ],
        )
        .unwrap();
        assert_eq!(plan.total_pages, 11);
        assert_eq!(
            plan.entries
                .iter()
                .map(|p| (p.key.as_str(), p.first_page, p.last_page))
                .collect::<Vec<_>>(),
            vec![
                ("archivo:b", 4, 8),
                ("lote:2", 9, 10),
                ("formulario:4", 11, 11)
            ]
        );
    }
    #[test]
    fn rejects_empty_documents_duplicate_keys_and_overflow() {
        assert!(plan_pdf_pages(1, 1, &[piece("a", 0)]).is_err());
        assert!(plan_pdf_pages(1, 1, &[piece("a", 1), piece("a", 1)]).is_err());
        assert!(plan_pdf_pages(u32::MAX, 1, &[]).is_err());
        assert!(plan_pdf_pages(1, 1, &[piece("a", u32::MAX)]).is_err());
    }
    #[test]
    fn detects_stability_and_cycles_after_real_layout_changes() {
        let a = plan_pdf_pages(1, 1, &[piece("a", 1)]).unwrap();
        let b = plan_pdf_pages(1, 2, &[piece("a", 1)]).unwrap();
        let mut stable = PageConvergence::default();
        assert!(stable.observe(a.clone()).unwrap().is_none());
        assert!(stable.observe(b.clone()).unwrap().is_none());
        assert_eq!(stable.observe(b.clone()).unwrap(), Some(b.clone()));
        let mut cycle = PageConvergence::default();
        assert!(cycle.observe(a.clone()).unwrap().is_none());
        assert!(cycle.observe(b).unwrap().is_none());
        assert!(cycle.observe(a).is_err());
    }
}
