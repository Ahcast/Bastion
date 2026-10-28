use crate::PagePlan;
use bastion_core::CoreError;
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Serialize)]
struct CoverData<'a> { expediente: &'a str, lpn: &'a str, institucion: &'a str }

/// Creates fixed source files and separate data for the isolated PDF worker.
/// No user-supplied string is inserted into Typst source code.
pub fn prepare_frontmatter_sources(expediente: &str, lpn: &str, institucion: &str, plan: &PagePlan) -> Result<BTreeMap<String, Vec<u8>>, CoreError> {
    if [expediente, lpn, institucion].iter().any(|v| v.trim().is_empty() || v.chars().count() > 1000) { return Err(CoreError::Invalid); }
    let mut files = BTreeMap::new();
    let cover = CoverData { expediente, lpn, institucion };
    files.insert("portada.json".into(), serde_json::to_vec(&cover).map_err(|_| CoreError::Invalid)?);
    files.insert("indice.json".into(), serde_json::to_vec(plan).map_err(|_| CoreError::Invalid)?);
    files.insert("portada.typ".into(), COVER.as_bytes().to_vec());
    files.insert("indice.typ".into(), INDEX.as_bytes().to_vec());
    Ok(files)
}

const COVER: &str = r#"#let datos = json("portada.json")
#set page(paper: "us-letter", margin: (x: 2.3cm, y: 2.5cm))
#set text(font: "Carlito", size: 12pt, lang: "es")
#align(center + horizon)[
  #text(size: 22pt, weight: "bold")[Expediente de licitación]
  #v(1cm)
  #text(size: 17pt, weight: "bold")[#datos.expediente]
  #v(1cm)
  *Proceso:* #datos.lpn
  #v(.4cm)
  *Institución:* #datos.institucion
]
"#;
const INDEX: &str = r#"#let indice = json("indice.json")
#set page(paper: "us-letter", margin: (x: 2.3cm, y: 2.5cm))
#set text(font: "Carlito", size: 11pt, lang: "es")
#text(size: 18pt, weight: "bold")[Índice]
#v(.7cm)
#for entrada in indice.entries {
  grid(columns: (1fr, auto), column-gutter: 1em,
    [#entrada.title], [#entrada.first_page])
  v(.35cm)
}
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{plan_pdf_pages, MeasuredPiece};
    #[test]
    fn uses_measured_pages_and_separates_all_text_from_code() {
        let plan = plan_pdf_pages(1,1,&[MeasuredPiece {key:"archivo:1".into(), title:"#panic(\"usuario\")".into(), pages:3}]).unwrap();
        let sources=prepare_frontmatter_sources("Expediente", "LPN-1", "Entidad", &plan).unwrap();
        let index:serde_json::Value=serde_json::from_slice(&sources["indice.json"]).unwrap();
        assert_eq!(index["entries"][0]["first_page"],3);
        assert_eq!(index["entries"][0]["last_page"],5);
        assert!(!String::from_utf8(sources["indice.typ"].clone()).unwrap().contains("#panic"));
    }
    #[test]
    fn frontmatter_compiles_with_typst_when_available() {
        let Some(bin)=std::env::var_os("BASTION_TYPST_BIN") else {return;};
        let plan=plan_pdf_pages(1,1,&[MeasuredPiece {key:"archivo:1".into(),title:"#panic(\"input\")".into(),pages:3}]).unwrap();
        let files=prepare_frontmatter_sources("Expediente", "LPN-1", "Entidad", &plan).unwrap();
        let root=std::env::temp_dir().join(format!("bastion-frontmatter-{}",std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        for (name,data) in files {std::fs::write(root.join(name),data).unwrap();}
        for name in ["portada", "indice"] {
            let output=std::process::Command::new(&bin).arg("compile").arg("--root").arg(&root).arg(root.join(format!("{name}.typ"))).arg(root.join(format!("{name}.pdf"))).output().unwrap();
            assert!(output.status.success(),"{}",String::from_utf8_lossy(&output.stderr));
        }
        let _=std::fs::remove_dir_all(&root);
    }
}
