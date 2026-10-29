use crate::{MeasuredPiece, PagePlan, plan_pdf_pages, prepare_frontmatter_sources};
use crate::pdf_compose::{measure_and_compose_pdf, pages, regular_file, render_prepared_sources};
use bastion_core::CoreError;
use serde::Deserialize;
use std::{collections::BTreeSet, fs, io::Write, path::Path};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LayoutEntry { pub key: String, pub title: String }
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LayoutRequest { pub expediente: String, pub lpn: String, pub institucion: String, pub entries: Vec<LayoutEntry> }

fn create_file(path:&Path,bytes:&[u8])->Result<(),CoreError>{
 let mut f=fs::OpenOptions::new().write(true).create_new(true).open(path).map_err(|_|CoreError::Invalid)?;
 f.write_all(bytes).map_err(|_|CoreError::Overloaded)
}
fn read_small_json(path:&Path)->Result<serde_json::Value,CoreError>{
 use std::io::Read;
 regular_file(path)?;
 let file=fs::File::open(path).map_err(|_|CoreError::Invalid)?;
 let mut bytes=Vec::new();file.take(2_000_001).read_to_end(&mut bytes).map_err(|_|CoreError::Invalid)?;
 if bytes.len()>2_000_000{return Err(CoreError::Invalid);}
 serde_json::from_slice(&bytes).map_err(|_|CoreError::Invalid)
}
fn update_total(root:&Path,total:u32)->Result<(),CoreError>{
 let path=root.join("config.json"); let mut data=read_small_json(&path)?;
 let object=data.as_object_mut().ok_or(CoreError::Invalid)?;
 if !object.contains_key("total_paginas"){return Err(CoreError::Invalid);}
 object.insert("total_paginas".into(),serde_json::json!(total));
 fs::write(&path,serde_json::to_vec(&data).map_err(|_|CoreError::Invalid)?).map_err(|_|CoreError::Overloaded)?;
 let path=root.join("globales.json"); let mut values=read_small_json(&path)?;
 let entries=values.as_array_mut().ok_or(CoreError::Invalid)?;
 let mut found=0;
 for entry in entries {
  if entry.get("key").and_then(|v|v.as_str())==Some("total_paginas") {
   if entry.get("kind").and_then(|v|v.as_str())!=Some("computed") {return Err(CoreError::Invalid);}
   entry.as_object_mut().ok_or(CoreError::Invalid)?.insert("value".into(),serde_json::json!(total));found+=1;
  }
 }
 if found!=1{return Err(CoreError::Invalid);}
 fs::write(path,serde_json::to_vec(&values).map_err(|_|CoreError::Invalid)?).map_err(|_|CoreError::Overloaded)
}

/// Complete a private job after sources and attachments are authorized and
/// staged. All source names are internal numbers; the worker must be isolated.
pub fn assemble_laid_out_pdf(typst:&Path,pdfinfo:&Path,qpdf:&Path,root:&Path,request:&LayoutRequest,source_numbers:&[u32],global_numbers:&[u32],packages:Option<&Path>,fonts:Option<&Path>)->Result<PagePlan,CoreError>{
 if !root.is_absolute() || !typst.is_absolute() || !pdfinfo.is_absolute() || !qpdf.is_absolute() || !fs::symlink_metadata(root).map_err(|_|CoreError::Invalid)?.file_type().is_dir() {return Err(CoreError::Invalid);}
 regular_file(typst)?;regular_file(pdfinfo)?;regular_file(qpdf)?;
 let pieces=u32::try_from(request.entries.len()).map_err(|_|CoreError::Invalid)?.checked_add(2).ok_or(CoreError::Invalid)?;
 if pieces>4000 {return Err(CoreError::Invalid);}
 let source_set:BTreeSet<_>=source_numbers.iter().copied().collect();
 let globals:BTreeSet<_>=global_numbers.iter().copied().collect();
 if source_set.len()!=source_numbers.len() || globals.len()!=global_numbers.len() || !globals.is_subset(&source_set) || source_set.iter().any(|n|*n<3 || *n>pieces){return Err(CoreError::Invalid);}
 let mut keys=BTreeSet::new();
 if request.entries.iter().any(|e|e.key.is_empty() || e.key.len()>250 || e.title.trim().is_empty() || e.title.chars().count()>1000 || !keys.insert(&e.key)) {return Err(CoreError::Invalid);}
 for n in 3..=pieces {
  let path=root.join(format!("piece-{n:06}.pdf"));
  if source_set.contains(&n) { if fs::symlink_metadata(path).is_ok(){return Err(CoreError::Conflict);} }
  else {regular_file(&path)?;}
 }
 let empty=plan_pdf_pages(1,1,&[])?;
 let front=prepare_frontmatter_sources(&request.expediente,&request.lpn,&request.institucion,&empty)?;
 for (name,bytes) in &front {create_file(&root.join(name),bytes)?;}
 create_file(&root.join("source-000001.typ"),&front["portada.typ"])?;
 create_file(&root.join("source-000002.typ"),&front["indice.typ"])?;
 let mut all_sources=vec![1];all_sources.extend_from_slice(source_numbers);
 let mut total_guess=1u32;let mut index_guess=1u32;let mut seen=BTreeSet::new();
 for _ in 0..8 {
  update_total(root,total_guess)?;
  render_prepared_sources(typst,root,pieces,&all_sources,global_numbers,packages,fonts)?;
  let cover_pages=pages(qpdf,&root.join("piece-000001.pdf"))?;
  let mut counts=Vec::with_capacity(request.entries.len());
  for (i,e) in request.entries.iter().enumerate() {
   let n=u32::try_from(i).map_err(|_|CoreError::Invalid)?+3;
   counts.push(MeasuredPiece {key:e.key.clone(),title:e.title.clone(),pages:pages(qpdf,&root.join(format!("piece-{n:06}.pdf")))?});
  }
  let tentative=plan_pdf_pages(cover_pages,index_guess,&counts)?;
  if tentative.total_pages>100_000{return Err(CoreError::Invalid);}
  fs::write(root.join("indice.json"),serde_json::to_vec(&tentative).map_err(|_|CoreError::Invalid)?).map_err(|_|CoreError::Overloaded)?;
  render_prepared_sources(typst,root,pieces,&[2],&[],packages,fonts)?;
  let actual_index=pages(qpdf,&root.join("piece-000002.pdf"))?;
  let final_plan=plan_pdf_pages(cover_pages,actual_index,&counts)?;
  if final_plan.total_pages>100_000{return Err(CoreError::Invalid);}
  if actual_index==index_guess && final_plan.total_pages==total_guess {
   let actual=measure_and_compose_pdf(typst,pdfinfo,qpdf,root,pieces)?;
   if actual!=final_plan.total_pages{return Err(CoreError::Invalid);}
   return Ok(final_plan);
  }
  let state=(total_guess,index_guess,counts.iter().map(|p|p.pages).collect::<Vec<_>>());
  if !seen.insert(state){return Err(CoreError::Conflict);}
  total_guess=final_plan.total_pages;index_guess=actual_index;
  for n in all_sources.iter().copied().chain(std::iter::once(2)) {
   let path=root.join(format!("piece-{n:06}.pdf"));regular_file(&path)?;fs::remove_file(path).map_err(|_|CoreError::Overloaded)?;
  }
  for n in global_numbers {
   let path=root.join(format!("bound-{n:06}.typ"));regular_file(&path)?;fs::remove_file(path).map_err(|_|CoreError::Overloaded)?;
  }
 }
 Err(CoreError::Conflict)
}

#[cfg(test)]
mod tests {
 use super::*;
 #[test]
 fn converges_with_form_and_multipage_attachment() {
  let (Some(typst),Some(qpdf),Some(pdfinfo))=(std::env::var_os("BASTION_TYPST_BIN"),std::env::var_os("BASTION_QPDF_BIN"),std::env::var_os("BASTION_PDFINFO_BIN")) else {return;};
  let root=std::env::temp_dir().join(format!("bastion-layout-{}",std::process::id()));fs::create_dir_all(&root).unwrap();
  fs::write(root.join("config.json"),br#"{"total_paginas":1}"#).unwrap();
  fs::write(root.join("config.typ"),b"#let datos_base = json(\"config.json\")\n").unwrap();
  let values=vec![crate::GlobalVariable {key:"nombre".into(),label:"Nombre".into(),kind:bastion_colaboracion::VariableKind::Text,value:Some(serde_json::json!("#panic()"))}];
  for (name,data) in crate::prepare_global_typst_sources(&values,1).unwrap(){fs::write(root.join(name),data).unwrap();}
  let form="#import \"config.typ\": datos_base\n#variables.nombre -- #datos_base.total_paginas";
  fs::write(root.join("source-000003.typ"),form).unwrap();
  fs::write(root.join("attachment.typ"),"#page[Anexo uno]\n#page[Anexo dos]").unwrap();
  assert!(std::process::Command::new(&typst).arg("compile").arg("--root").arg(&root).arg(root.join("attachment.typ")).arg(root.join("piece-000004.pdf")).status().unwrap().success());
  let request=LayoutRequest {expediente:"Prueba".into(),lpn:"LPN-1".into(),institucion:"Entidad".into(),entries:vec![LayoutEntry{key:"formulario:1".into(),title:"Formulario".into()},LayoutEntry{key:"archivo:1".into(),title:"Anexo".into()}]};
  let plan=assemble_laid_out_pdf(Path::new(&typst),Path::new(&pdfinfo),Path::new(&qpdf),&root,&request,&[3],&[3],None,None).unwrap();
  assert_eq!(plan.total_pages,5);assert_eq!(plan.entries[0].first_page,3);assert_eq!(plan.entries[1].first_page,4);assert_eq!(plan.entries[1].last_page,5);
  assert_eq!(read_small_json(&root.join("config.json")).unwrap()["total_paginas"],5);
  let index=read_small_json(&root.join("indice.json")).unwrap();assert_eq!(index["entries"][1]["first_page"],4);
  assert_eq!(pages(Path::new(&qpdf),&root.join("result.pdf")).unwrap(),5);
  assert_eq!(fs::read_to_string(root.join("source-000003.typ")).unwrap(),form);
  let _=fs::remove_dir_all(root);
 }
}
