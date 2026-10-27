#[derive(Clone, Debug)]
pub enum SpreadsheetCell {
    Text(String),
    Number(String),
}

pub fn workbook<L: AsRef<str>>(
    summary: &[(L, SpreadsheetCell)],
    headers: &[&str],
    rows: &[Vec<SpreadsheetCell>],
) -> String {
    let summary_rows = summary
        .iter()
        .map(|(label, value)| row(&[SpreadsheetCell::Text(label.as_ref().into()), value.clone()]))
        .collect::<String>();
    let header_row = row(&headers
        .iter()
        .map(|header| SpreadsheetCell::Text((*header).into()))
        .collect::<Vec<_>>());
    let detail_rows = rows.iter().map(|cells| row(cells)).collect::<String>();
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<Workbook xmlns=\"urn:schemas-microsoft-com:office:spreadsheet\" xmlns:ss=\"urn:schemas-microsoft-com:office:spreadsheet\"><Worksheet ss:Name=\"Resumen\"><Table>{summary_rows}</Table></Worksheet><Worksheet ss:Name=\"Detalle\"><Table>{header_row}{detail_rows}</Table></Worksheet></Workbook>"
    )
}

fn row(cells: &[SpreadsheetCell]) -> String {
    let content = cells
        .iter()
        .map(|cell| {
            let (kind, value) = match cell {
                SpreadsheetCell::Text(value) => ("String", xml_text(value)),
                SpreadsheetCell::Number(value) => {
                    if decimal_number(value) {
                        ("Number", value.clone())
                    } else {
                        ("String", xml_text(value))
                    }
                }
            };
            format!("<Cell><Data ss:Type=\"{kind}\">{value}</Data></Cell>")
        })
        .collect::<String>();
    format!("<Row>{content}</Row>")
}

fn decimal_number(value: &str) -> bool {
    let value = value.strip_prefix('-').unwrap_or(value);
    let (whole, fraction) = value.split_once('.').unwrap_or((value, ""));
    !whole.is_empty()
        && whole.bytes().all(|byte| byte.is_ascii_digit())
        && fraction.bytes().all(|byte| byte.is_ascii_digit())
        && value.matches('.').count() <= 1
        && value
            .trim_start_matches('0')
            .chars()
            .filter(|c| c.is_ascii_digit())
            .count()
            <= 15
}

fn xml_text(value: &str) -> String {
    value
        .chars()
        .filter(|character| {
            matches!(*character as u32, 0x9 | 0xA | 0xD | 0x20..=0xD7FF | 0xE000..=0xFFFD | 0x10000..=0x10FFFF)
        })
        .map(|character| match character {
            '&' => "&amp;".to_owned(),
            '<' => "&lt;".to_owned(),
            '>' => "&gt;".to_owned(),
            '"' => "&quot;".to_owned(),
            '\'' => "&apos;".to_owned(),
            _ => character.to_string(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exports_literal_text_safe_xml_and_exact_large_amounts() {
        let xml = workbook(
            &[("Total", SpreadsheetCell::Number("135.02".into()))],
            &["Cliente", "Importe"],
            &[vec![
                SpreadsheetCell::Text("=HYPERLINK(\"x\") <&>\u{0001}\u{fffe}".into()),
                SpreadsheetCell::Number("123456789012345.67".into()),
            ]],
        );
        assert!(xml.contains("ss:Name=\"Resumen\""));
        assert!(xml.contains("ss:Name=\"Detalle\""));
        assert!(xml.contains("ss:Type=\"Number\">135.02"));
        assert!(xml.contains("ss:Type=\"String\">=HYPERLINK(&quot;x&quot;) &lt;&amp;&gt;"));
        assert!(xml.contains("ss:Type=\"String\">123456789012345.67"));
        assert!(!xml.contains("ss:Formula"));
        assert!(!xml.contains(['\u{0001}', '\u{fffe}']));
    }
}
