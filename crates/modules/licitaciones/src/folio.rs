use bastion_core::CoreError;
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct FolioPage {
    pub width_pt: f64,
    pub height_pt: f64,
}

/// Parse C-locale `pdfinfo -f 1 -l N -box` output. The reported size is the
/// visible crop box, before the separately reported rotation is applied.
pub fn parse_pdfinfo_pages(output: &str, count: u32) -> Result<Vec<FolioPage>, CoreError> {
    if count == 0 || count > 100_000 || output.len() > 50_000_000 {
        return Err(CoreError::Invalid);
    }
    let mut sizes = vec![None; count as usize];
    let mut rotations = vec![None; count as usize];
    for line in output.lines() {
        let parts: Vec<_> = line.split_whitespace().collect();
        if parts.len() < 4 || parts[0] != "Page" {
            continue;
        }
        let Ok(number) = parts[1].parse::<usize>() else {
            continue;
        };
        if number == 0 || number > count as usize {
            return Err(CoreError::Invalid);
        }
        let index = number - 1;
        match parts[2] {
            "size:" => {
                if parts.len() < 7 || parts[4] != "x" || parts[6] != "pts" || sizes[index].is_some()
                {
                    return Err(CoreError::Invalid);
                }
                let w = parts[3].parse::<f64>().map_err(|_| CoreError::Invalid)?;
                let h = parts[5].parse::<f64>().map_err(|_| CoreError::Invalid)?;
                if !w.is_finite()
                    || !h.is_finite()
                    || w <= 0.0
                    || h <= 0.0
                    || w > 100_000.0
                    || h > 100_000.0
                {
                    return Err(CoreError::Invalid);
                }
                sizes[index] = Some((w, h));
            }
            "rot:" => {
                if rotations[index].is_some() {
                    return Err(CoreError::Invalid);
                }
                let rotation = parts[3].parse::<u16>().map_err(|_| CoreError::Invalid)?;
                if !matches!(rotation, 0 | 90 | 180 | 270) {
                    return Err(CoreError::Invalid);
                }
                rotations[index] = Some(rotation);
            }
            _ => {}
        }
    }
    sizes
        .into_iter()
        .zip(rotations)
        .map(|(size, rotation)| {
            let (w, h) = size.ok_or(CoreError::Invalid)?;
            let rot = rotation.ok_or(CoreError::Invalid)?;
            let (width_pt, height_pt) = if rot == 90 || rot == 270 {
                (h, w)
            } else {
                (w, h)
            };
            Ok(FolioPage {
                width_pt,
                height_pt,
            })
        })
        .collect()
}

/// Creates a transparent overlay, numbering pages in their visible orientation.
/// Content may be covered if it extends into the margin, as the user requested.
pub fn prepare_folio_sources(pages: &[FolioPage]) -> Result<BTreeMap<String, Vec<u8>>, CoreError> {
    if pages.is_empty()
        || pages.len() > 100_000
        || pages.iter().any(|p| {
            !p.width_pt.is_finite()
                || !p.height_pt.is_finite()
                || p.width_pt <= 0.0
                || p.height_pt <= 0.0
                || p.width_pt > 100_000.0
                || p.height_pt > 100_000.0
        })
    {
        return Err(CoreError::Invalid);
    }
    let mut result = BTreeMap::new();
    result.insert(
        "folios.json".into(),
        serde_json::to_vec(pages).map_err(|_| CoreError::Invalid)?,
    );
    result.insert("folios.typ".into(), FOLIOS.as_bytes().to_vec());
    Ok(result)
}
const FOLIOS: &str = r#"#let folios = json("folios.json")
#for (index, p) in folios.enumerate() {
  page(width: p.width_pt * 1pt, height: p.height_pt * 1pt, margin: 0pt, fill: none)[
    #place(bottom + center, dy: -12pt)[#text(size: 9pt)[#(index + 1)]]
  ]
}
"#;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_crop_size_and_display_rotation() {
        let info = "Pages: 3\nPage    1 size: 612 x 792 pts (letter)\nPage    1 rot: 0\nPage    2 size: 792 x 612 pts (letter)\nPage    2 rot: 0\nPage    3 size: 570 x 710 pts\nPage    3 rot: 90\nPage    3 CropBox: 20 40 590 750\n";
        let pages = parse_pdfinfo_pages(info, 3).unwrap();
        assert_eq!(
            pages[0],
            FolioPage {
                width_pt: 612.0,
                height_pt: 792.0
            }
        );
        assert_eq!(
            pages[1],
            FolioPage {
                width_pt: 792.0,
                height_pt: 612.0
            }
        );
        assert_eq!(
            pages[2],
            FolioPage {
                width_pt: 710.0,
                height_pt: 570.0
            }
        );
        assert!(parse_pdfinfo_pages(info, 4).is_err());
        assert!(parse_pdfinfo_pages(&info.replace("rot: 90", "rot: 45"), 3).is_err());
        assert!(parse_pdfinfo_pages(&info.replace("570 x 710", "NaN x 710"), 3).is_err());
        let sources = prepare_folio_sources(&pages).unwrap();
        let data: serde_json::Value = serde_json::from_slice(&sources["folios.json"]).unwrap();
        assert_eq!(data[2]["width_pt"], 710.0);
    }
    #[test]
    fn overlay_compiles_when_typst_is_available() {
        let Some(bin) = std::env::var_os("BASTION_TYPST_BIN") else {
            return;
        };
        let root = std::env::temp_dir().join(format!("bastion-folios-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        for (name, bytes) in prepare_folio_sources(&[
            FolioPage {
                width_pt: 612.0,
                height_pt: 792.0,
            },
            FolioPage {
                width_pt: 710.0,
                height_pt: 570.0,
            },
        ])
        .unwrap()
        {
            std::fs::write(root.join(name), bytes).unwrap();
        }
        let output = std::process::Command::new(bin)
            .arg("compile")
            .arg("--root")
            .arg(&root)
            .arg(root.join("folios.typ"))
            .arg(root.join("folios.pdf"))
            .output()
            .unwrap();
        let _ = std::fs::remove_dir_all(&root);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
