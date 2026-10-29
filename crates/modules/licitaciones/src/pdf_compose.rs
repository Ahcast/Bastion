//! QPDF adapter for a future isolated PDF worker. It accepts only numbered,
//! pre-rendered files in a private job directory; it does not read storage or
//! compile user templates. The caller must isolate and resource-limit the job.
use crate::validate_final_pdf_budget;
use bastion_core::CoreError;
use std::{
    fs,
    path::Path,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

fn run(mut command: Command) -> Result<String, CoreError> {
    command
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let mut child = command.spawn().map_err(|_| CoreError::Overloaded)?;
    let started = Instant::now();
    loop {
        match child.try_wait().map_err(|_| CoreError::Overloaded)? {
            Some(status) => {
                if !status.success() {
                    return Err(CoreError::Invalid);
                }
                use std::io::Read;
                let mut bytes = Vec::new();
                child
                    .stdout
                    .take()
                    .ok_or(CoreError::Overloaded)?
                    .take(4097)
                    .read_to_end(&mut bytes)
                    .map_err(|_| CoreError::Overloaded)?;
                if bytes.len() > 4096 {
                    return Err(CoreError::Invalid);
                }
                return String::from_utf8(bytes).map_err(|_| CoreError::Invalid);
            }
            None if started.elapsed() >= Duration::from_secs(120) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(CoreError::Overloaded);
            }
            None => thread::sleep(Duration::from_millis(50)),
        }
    }
}
pub(crate) fn regular_file(path: &Path) -> Result<(), CoreError> {
    let meta = fs::symlink_metadata(path).map_err(|_| CoreError::Invalid)?;
    if !meta.file_type().is_file() {
        return Err(CoreError::Invalid);
    }
    Ok(())
}
fn absent(path: &Path) -> bool {
    matches!(fs::symlink_metadata(path), Err(ref e) if e.kind() == std::io::ErrorKind::NotFound)
}
pub(crate) fn pages(qpdf: &Path, file: &Path) -> Result<u32, CoreError> {
    regular_file(file)?;
    let mut command = Command::new(qpdf);
    command.arg("--show-npages").arg(file);
    let count = run(command)?
        .trim()
        .parse::<u32>()
        .map_err(|_| CoreError::Invalid)?;
    if count == 0 {
        return Err(CoreError::Invalid);
    }
    Ok(count)
}
/// Merge `piece-000001.pdf`, ... in numerical order and apply `folios.pdf`.
/// The files must be immutable snapshots in a new, private job directory.
/// `folios.pdf` is generated separately with one page for each measured page.
/// This function MUST run inside the dedicated no-network, resource-limited
/// worker; it is not an endpoint and does not provide isolation on its own.
pub fn compose_prepared_pdf(qpdf: &Path, root: &Path, pieces: u32) -> Result<u32, CoreError> {
    if !qpdf.is_absolute() || !root.is_absolute() || pieces == 0 || pieces > 4000 {
        return Err(CoreError::Invalid);
    }
    regular_file(qpdf)?;
    let meta = fs::symlink_metadata(root).map_err(|_| CoreError::Invalid)?;
    if !meta.file_type().is_dir() {
        return Err(CoreError::Invalid);
    }
    let merged = root.join("merged.pdf");
    let output = root.join("result.pdf");
    let folios = root.join("folios.pdf");
    if !absent(&merged) || !absent(&output) {
        return Err(CoreError::Conflict);
    }
    let mut sum = 0u32;
    let mut command = Command::new(qpdf);
    command.args(["--empty", "--pages"]);
    for i in 1..=pieces {
        let part = root.join(format!("piece-{i:06}.pdf"));
        sum = sum
            .checked_add(pages(qpdf, &part)?)
            .ok_or(CoreError::Invalid)?;
        command.arg(part);
    }
    if pages(qpdf, &folios)? != sum {
        return Err(CoreError::Invalid);
    }
    command.arg("--").arg(&merged);
    run(command)?;
    if pages(qpdf, &merged)? != sum {
        return Err(CoreError::Invalid);
    }
    let mut overlay = Command::new(qpdf);
    overlay
        .arg(&merged)
        .arg("--overlay")
        .arg(&folios)
        .arg("--")
        .arg(&output);
    run(overlay)?;
    if pages(qpdf, &output)? != sum {
        return Err(CoreError::Invalid);
    }
    validate_final_pdf_budget(fs::metadata(&output).map_err(|_| CoreError::Invalid)?.len())?;
    Ok(sum)
}

/// Compile prepared, explicitly approved sources in the private job root.
/// The service must supply fixed package and font directories and must run
/// this only in the isolated worker, after checking source authorization.
pub fn render_prepared_sources(
    typst: &Path,
    root: &Path,
    pieces: u32,
    source_numbers: &[u32],
    global_numbers: &[u32],
    packages: Option<&Path>,
    fonts: Option<&Path>,
) -> Result<(), CoreError> {
    if !typst.is_absolute()
        || !root.is_absolute()
        || pieces == 0
        || pieces > 4000
        || source_numbers.len() > pieces as usize
    {
        return Err(CoreError::Invalid);
    }
    regular_file(typst)?;
    if !fs::symlink_metadata(root)
        .map_err(|_| CoreError::Invalid)?
        .file_type()
        .is_dir()
    {
        return Err(CoreError::Invalid);
    }
    for directory in [packages, fonts].into_iter().flatten() {
        if !directory.is_absolute()
            || !fs::symlink_metadata(directory)
                .map_err(|_| CoreError::Invalid)?
                .file_type()
                .is_dir()
        {
            return Err(CoreError::Invalid);
        }
    }
    let unique: std::collections::BTreeSet<_> = source_numbers.iter().copied().collect();
    let globals: std::collections::BTreeSet<_> = global_numbers.iter().copied().collect();
    if unique.len() != source_numbers.len()
        || globals.len() != global_numbers.len()
        || !globals.is_subset(&unique)
        || unique.iter().any(|n| *n == 0 || *n > pieces)
    {
        return Err(CoreError::Invalid);
    }
    if !globals.is_empty() {
        regular_file(&root.join("globales.typ"))?;
        regular_file(&root.join("globales.json"))?;
    }
    for &number in source_numbers {
        let input = root.join(format!("source-{number:06}.typ"));
        let output = root.join(format!("piece-{number:06}.pdf"));
        regular_file(&input)?;
        if !absent(&output) {
            return Err(CoreError::Conflict);
        }
        let mut command = Command::new(typst);
        command
            .arg("compile")
            .arg("--root")
            .arg(root)
            .arg("--package-cache-path")
            .arg(root.join("package-cache"));
        if let Some(path) = packages {
            command.arg("--package-path").arg(path);
        }
        if let Some(path) = fonts {
            command
                .arg("--ignore-system-fonts")
                .arg("--font-path")
                .arg(path);
        }
        let source = if globals.contains(&number) {
            let bound = root.join(format!("bound-{number:06}.typ"));
            bind_source_file(&input, &bound)?;
            bound
        } else {
            input
        };
        command.arg(source).arg(&output);
        run(command)?;
        regular_file(&output)?;
    }
    Ok(())
}
fn bind_source_file(source: &Path, bound: &Path) -> Result<(), CoreError> {
    use std::io::{Read, Write};
    let mut input = fs::File::open(source).map_err(|_| CoreError::Invalid)?;
    let size = input.metadata().map_err(|_| CoreError::Invalid)?.len();
    if size > 100_000_000 {
        return Err(CoreError::Invalid);
    }
    let mut output = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(bound)
        .map_err(|_| CoreError::Invalid)?;
    output
        .write_all(b"#import \"globales.typ\": variables\n")
        .map_err(|_| CoreError::Overloaded)?;
    let prefix_len = std::cmp::min(size, 3) as usize;
    let mut prefix = [0u8; 3];
    input
        .read_exact(&mut prefix[..prefix_len])
        .map_err(|_| CoreError::Invalid)?;
    if prefix_len != 3 || prefix != [0xef, 0xbb, 0xbf] {
        output
            .write_all(&prefix[..prefix_len])
            .map_err(|_| CoreError::Overloaded)?;
    }
    let copied = std::io::copy(&mut input.take(100_000_001), &mut output)
        .map_err(|_| CoreError::Overloaded)?;
    if copied + prefix_len as u64 != size {
        return Err(CoreError::Invalid);
    }
    Ok(())
}
/// Measure all pages, create a transparent numbered overlay, and merge. This
/// does not create the job sandbox; it must only run inside that sandbox.
pub fn measure_and_compose_pdf(
    typst: &Path,
    pdfinfo: &Path,
    qpdf: &Path,
    root: &Path,
    pieces: u32,
) -> Result<u32, CoreError> {
    if !typst.is_absolute()
        || !pdfinfo.is_absolute()
        || !qpdf.is_absolute()
        || !root.is_absolute()
        || pieces == 0
        || pieces > 4000
    {
        return Err(CoreError::Invalid);
    }
    regular_file(typst)?;
    regular_file(pdfinfo)?;
    regular_file(qpdf)?;
    if !fs::symlink_metadata(root)
        .map_err(|_| CoreError::Invalid)?
        .file_type()
        .is_dir()
    {
        return Err(CoreError::Invalid);
    }
    let mut geometry = Vec::new();
    for number in 1..=pieces {
        let input = root.join(format!("piece-{number:06}.pdf"));
        let count = pages(qpdf, &input)?;
        if count > 100_000
            || geometry
                .len()
                .checked_add(count as usize)
                .filter(|n| *n <= 100_000)
                .is_none()
        {
            return Err(CoreError::Invalid);
        }
        let info = root.join(format!("page-info-{number:06}.txt"));
        let output = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&info)
            .map_err(|_| CoreError::Invalid)?;
        let mut command = Command::new(pdfinfo);
        command
            .env_clear()
            .env("LC_ALL", "C")
            .stdin(Stdio::null())
            .stdout(Stdio::from(output))
            .stderr(Stdio::null())
            .args(["-f", "1", "-l"])
            .arg(count.to_string())
            .arg("-box")
            .arg(&input);
        let mut child = command.spawn().map_err(|_| CoreError::Overloaded)?;
        let start = Instant::now();
        loop {
            match child.try_wait().map_err(|_| CoreError::Overloaded)? {
                Some(status) if status.success() => break,
                Some(_) => return Err(CoreError::Invalid),
                None if start.elapsed() >= Duration::from_secs(120) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(CoreError::Overloaded);
                }
                None => thread::sleep(Duration::from_millis(50)),
            }
        }
        if fs::metadata(&info).map_err(|_| CoreError::Invalid)?.len() > 50_000_000 {
            return Err(CoreError::Invalid);
        }
        let measured = fs::read_to_string(&info).map_err(|_| CoreError::Invalid)?;
        geometry.extend(crate::folio::parse_pdfinfo_pages(&measured, count)?);
        fs::remove_file(info).map_err(|_| CoreError::Overloaded)?;
    }
    for (name, bytes) in crate::folio::prepare_folio_sources(&geometry)? {
        use std::io::Write;
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(root.join(name))
            .map_err(|_| CoreError::Invalid)?;
        file.write_all(&bytes).map_err(|_| CoreError::Overloaded)?;
    }
    let mut compile = Command::new(typst);
    compile
        .arg("compile")
        .arg("--root")
        .arg(root)
        .arg(root.join("folios.typ"))
        .arg(root.join("folios.pdf"));
    run(compile)?;
    let actual = compose_prepared_pdf(qpdf, root, pieces)?;
    if actual as usize != geometry.len() {
        return Err(CoreError::Invalid);
    }
    Ok(actual)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn merges_and_numbers_when_test_binaries_are_available() {
        let (Some(typst), Some(qpdf)) = (
            std::env::var_os("BASTION_TYPST_BIN"),
            std::env::var_os("BASTION_QPDF_BIN"),
        ) else {
            return;
        };
        let root = std::env::temp_dir().join(format!("bastion-compose-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        for (filename, source) in [
            (
                "piece-000001",
                "#page(width: 612pt, height: 792pt)[Primera]",
            ),
            (
                "piece-000002",
                "#page(width: 792pt, height: 612pt)[Segunda]",
            ),
            (
                "folios",
                "#page(width: 612pt, height: 792pt)[#place(bottom + center)[1]]\n#page(width: 792pt, height: 612pt)[#place(bottom + center)[2]]",
            ),
        ] {
            if filename == "folios" {
                for (name, bytes) in crate::folio::prepare_folio_sources(&[
                    crate::folio::FolioPage {
                        width_pt: 612.0,
                        height_pt: 792.0,
                    },
                    crate::folio::FolioPage {
                        width_pt: 792.0,
                        height_pt: 612.0,
                    },
                ])
                .unwrap()
                {
                    fs::write(root.join(name), bytes).unwrap();
                }
            } else {
                fs::write(root.join(format!("{filename}.typ")), source).unwrap();
            }
            let status = Command::new(&typst)
                .arg("compile")
                .arg("--root")
                .arg(&root)
                .arg(root.join(format!("{filename}.typ")))
                .arg(root.join(format!("{filename}.pdf")))
                .status()
                .unwrap();
            assert!(status.success());
        }
        assert_eq!(compose_prepared_pdf(Path::new(&qpdf), &root, 2).unwrap(), 2);
        assert!(root.join("result.pdf").is_file());
        assert!(compose_prepared_pdf(Path::new(&qpdf), &root, 2).is_err());
        let _ = fs::remove_dir_all(&root);
    }
    #[test]
    fn measures_and_composes_a_mixed_size_pdf_when_binaries_are_available() {
        let (Some(typst), Some(qpdf), Some(pdfinfo)) = (
            std::env::var_os("BASTION_TYPST_BIN"),
            std::env::var_os("BASTION_QPDF_BIN"),
            std::env::var_os("BASTION_PDFINFO_BIN"),
        ) else {
            return;
        };
        let root =
            std::env::temp_dir().join(format!("bastion-measured-compose-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        std::fs::write(
            root.join("source-000001.typ"),
            "#page(width: 612pt, height: 792pt)[Primera]",
        )
        .unwrap();
        std::fs::write(
            root.join("source-000002.typ"),
            "#page(width: 792pt, height: 612pt)[Segunda]",
        )
        .unwrap();
        render_prepared_sources(Path::new(&typst), &root, 3, &[1, 2], &[], None, None).unwrap();
        assert!(
            render_prepared_sources(Path::new(&typst), &root, 3, &[1], &[], None, None).is_err()
        );
        assert!(
            Command::new(&qpdf)
                .arg(root.join("piece-000001.pdf"))
                .arg("--rotate=+90:1")
                .arg(root.join("piece-000003.pdf"))
                .status()
                .unwrap()
                .success()
        );
        assert_eq!(
            measure_and_compose_pdf(
                Path::new(&typst),
                Path::new(&pdfinfo),
                Path::new(&qpdf),
                &root,
                3
            )
            .unwrap(),
            3
        );
        assert_eq!(
            pages(Path::new(&qpdf), &root.join("result.pdf")).unwrap(),
            3
        );
        let folios: serde_json::Value =
            serde_json::from_slice(&fs::read(root.join("folios.json")).unwrap()).unwrap();
        assert_eq!(folios[2]["width_pt"], 792.0);
        assert_eq!(folios[2]["height_pt"], 612.0);
        let _ = fs::remove_dir_all(&root);
    }
    #[test]
    fn approved_global_sources_compile_with_typed_values() {
        let Some(bin) = std::env::var_os("BASTION_TYPST_BIN") else {
            return;
        };
        let root =
            std::env::temp_dir().join(format!("bastion-worker-global-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let values = vec![
            crate::GlobalVariable {
                key: "nombre".into(),
                label: "Nombre".into(),
                kind: bastion_colaboracion::VariableKind::Text,
                value: Some(serde_json::json!("#panic()")),
            },
            crate::GlobalVariable {
                key: "numero".into(),
                label: "Número".into(),
                kind: bastion_colaboracion::VariableKind::Integer,
                value: Some(serde_json::json!("42")),
            },
        ];
        for (name, data) in crate::prepare_global_typst_sources(&values, 7).unwrap() {
            fs::write(root.join(name), data).unwrap();
        }
        let original = "\u{feff}#assert.eq(variables.numero,42)\n#assert.eq(variables.total_paginas,7)\n#variables.nombre";
        fs::write(root.join("source-000001.typ"), original).unwrap();
        assert!(
            render_prepared_sources(Path::new(&bin), &root, 1, &[1], &[2], None, None).is_err()
        );
        render_prepared_sources(Path::new(&bin), &root, 1, &[1], &[1], None, None).unwrap();
        assert!(
            fs::read(root.join("piece-000001.pdf"))
                .unwrap()
                .starts_with(b"%PDF-")
        );
        assert_eq!(
            fs::read_to_string(root.join("source-000001.typ")).unwrap(),
            original
        );
        let bound = fs::read_to_string(root.join("bound-000001.typ")).unwrap();
        assert!(bound.starts_with("#import \"globales.typ\": variables\n#assert"));
        let _ = fs::remove_dir_all(root);
    }
}
