/// A user-facing filename, independent from private object keys and host paths.
pub fn pdf_filename(reference: &str, fallback: &str) -> String {
    let clean = reference
        .trim()
        .chars()
        .map(|ch| {
            if ch.is_control() || matches!(ch, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|')
            {
                '_'
            } else {
                ch
            }
        })
        .collect::<String>();
    let stem = clean.trim_matches([' ', '.']);
    let stem = if stem.is_empty() { fallback } else { stem };
    let mut name = String::new();
    for ch in stem.chars() {
        if name.len() + ch.len_utf8() > 180 {
            break;
        }
        name.push(ch);
    }
    let device = name.split('.').next().unwrap_or("").to_ascii_uppercase();
    if matches!(device.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (device.len() == 4
            && (device.starts_with("COM") || device.starts_with("LPT"))
            && matches!(device.as_bytes()[3], b'1'..=b'9'))
    {
        name.insert(0, '_');
    }
    format!("{}.pdf", name.trim_end_matches([' ', '.']))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn business_references_remain_readable_and_cannot_be_paths_or_headers() {
        assert_eq!(pdf_filename("COT-000017", "cotizacion-1"), "COT-000017.pdf");
        assert_eq!(
            pdf_filename("000-001-01-00000023", "proforma-1"),
            "000-001-01-00000023.pdf"
        );
        assert_eq!(
            pdf_filename(" Exp. Licitación 24/2026 ", "expediente-1"),
            "Exp. Licitación 24_2026.pdf"
        );
        assert_eq!(pdf_filename("..\r\n/x", "expediente-1"), "___x.pdf");
        assert_eq!(pdf_filename(" . ", "expediente-1"), "expediente-1.pdf");
        assert_eq!(pdf_filename("CON", "expediente-1"), "_CON.pdf");
        let name = pdf_filename(&"ñ".repeat(200), "expediente-1");
        assert!(name.len() <= 185 && name.ends_with(".pdf"));
    }
}
