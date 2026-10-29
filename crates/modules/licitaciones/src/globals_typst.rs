use crate::{GlobalVariable, GlobalVariablesInput};
use bastion_colaboracion::VariableKind;
use bastion_core::CoreError;
use serde_json::{Value, json};
use std::collections::BTreeMap;

/// Prepare typed global values as inert data for approved Typst forms.
/// Large integers/decimals that Typst cannot represent are rejected here.
pub fn prepare_global_typst_sources(
    variables: &[GlobalVariable],
    total_pages: u32,
) -> Result<BTreeMap<String, Vec<u8>>, CoreError> {
    if total_pages == 0 {
        return Err(CoreError::Invalid);
    }
    let input = GlobalVariablesInput {
        variables: variables.to_vec(),
    };
    input.validate()?;
    input.validate_form_config()?;
    let mut values = Vec::new();
    for v in variables {
        if v.key == "total_paginas" {
            continue;
        }
        let value = v.value.as_ref().filter(|v| v.as_str() != Some(""));
        match (v.kind, value) {
            (VariableKind::Integer, Some(value)) => {
                value
                    .as_str()
                    .ok_or(CoreError::Invalid)?
                    .parse::<i64>()
                    .map_err(|_| CoreError::Invalid)?;
            }
            (VariableKind::Decimal, Some(value)) => {
                validate_typst_decimal(value.as_str().ok_or(CoreError::Invalid)?)?
            }
            _ => {}
        }
        values.push(json!({"key":v.key,"kind":v.kind,"value":value.unwrap_or(&Value::Null)}));
    }
    values.push(json!({"key":"total_paginas","kind":"computed","value":total_pages}));
    let mut result = BTreeMap::new();
    result.insert(
        "globales.json".into(),
        serde_json::to_vec(&values).map_err(|_| CoreError::Invalid)?,
    );
    result.insert("globales.typ".into(), GLOBALS.as_bytes().to_vec());
    Ok(result)
}
fn validate_typst_decimal(text: &str) -> Result<(), CoreError> {
    let unsigned = text.strip_prefix('-').unwrap_or(text);
    let digits: String = unsigned.chars().filter(|c| *c != '.').collect();
    let digits = digits.trim_start_matches('0');
    if digits.is_empty() {
        return Ok(());
    }
    let coefficient = digits.parse::<u128>().map_err(|_| CoreError::Invalid)?;
    if coefficient > 79_228_162_514_264_337_593_543_950_335 {
        return Err(CoreError::Invalid);
    }
    Ok(())
}
/// Prefix the fixed import only after a trusted service has approved the source.
/// No user data is interpolated into this source.
pub fn bind_global_typst_source(source: &str) -> Result<String, CoreError> {
    if source.len() > 100_000_000 {
        return Err(CoreError::Invalid);
    }
    Ok(format!(
        "#import \"globales.typ\": variables\n{}",
        source.strip_prefix('\u{feff}').unwrap_or(source)
    ))
}
const GLOBALS: &str = r#"#let variables = json("globales.json").map(v => (v.key, if v.value == none { none } else if v.kind == "integer" { int(v.value) } else if v.kind == "decimal" { decimal(v.value) } else { v.value })).to-dict()
"#;

#[cfg(test)]
mod tests {
    use super::*;
    fn v(key: &str, kind: VariableKind, value: Value) -> GlobalVariable {
        GlobalVariable {
            key: key.into(),
            label: key.into(),
            kind,
            value: Some(value),
        }
    }
    #[test]
    fn validates_numbers_and_replaces_page_count() {
        let vars = vec![
            v("nombre", VariableKind::Text, json!("#panic()")),
            v("numero", VariableKind::Integer, json!("42")),
            v("monto", VariableKind::Decimal, json!("10.25")),
            v("total_paginas", VariableKind::Integer, json!("999")),
        ];
        let files = prepare_global_typst_sources(&vars, 7).unwrap();
        let value: Value = serde_json::from_slice(&files["globales.json"]).unwrap();
        assert_eq!(
            value
                .as_array()
                .unwrap()
                .iter()
                .filter(|v| v["key"] == "total_paginas")
                .count(),
            1
        );
        assert_eq!(value.as_array().unwrap().last().unwrap()["value"], 7);
        assert!(
            !String::from_utf8(files["globales.typ"].clone())
                .unwrap()
                .contains("#panic")
        );
        assert!(
            prepare_global_typst_sources(
                &[v("n", VariableKind::Integer, json!("9223372036854775808"))],
                1
            )
            .is_err()
        );
        assert!(
            prepare_global_typst_sources(
                &[v(
                    "n",
                    VariableKind::Decimal,
                    json!("79228162514264337593543950336")
                )],
                1
            )
            .is_err()
        );
        assert!(prepare_global_typst_sources(&vars, 0).is_err());
    }
    #[test]
    fn typed_globals_are_available_and_user_text_is_inert() {
        let Some(bin) = std::env::var_os("BASTION_TYPST_BIN") else {
            return;
        };
        let root =
            std::env::temp_dir().join(format!("bastion-typst-globals-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let vars = vec![
            v("nombre", VariableKind::Text, json!("#panic()")),
            v("numero", VariableKind::Integer, json!("42")),
            v("monto", VariableKind::Decimal, json!("10.25")),
            v("activo", VariableKind::Boolean, json!(true)),
        ];
        for (name, data) in prepare_global_typst_sources(&vars, 7).unwrap() {
            std::fs::write(root.join(name), data).unwrap();
        }
        let source = "#assert.eq(variables.numero, 42)\n#assert.eq(variables.monto, decimal(\"10.25\"))\n#assert.eq(variables.activo, true)\n#assert.eq(variables.total_paginas, 7)\n#variables.nombre\n";
        std::fs::write(
            root.join("main.typ"),
            bind_global_typst_source(source).unwrap(),
        )
        .unwrap();
        let output = std::process::Command::new(bin)
            .arg("compile")
            .arg("--root")
            .arg(&root)
            .arg(root.join("main.typ"))
            .arg(root.join("main.pdf"))
            .output()
            .unwrap();
        let _ = std::fs::remove_dir_all(root);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
