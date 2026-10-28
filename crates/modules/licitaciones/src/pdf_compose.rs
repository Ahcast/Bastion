//! QPDF adapter for a future isolated PDF worker. It accepts only numbered,
//! pre-rendered files in a private job directory; it does not read storage or
//! compile user templates. The caller must isolate and resource-limit the job.
use crate::validate_final_pdf_budget;
use bastion_core::CoreError;
use std::{fs, path::Path, process::{Command, Stdio}, thread, time::{Duration, Instant}};

fn run(mut command: Command) -> Result<String, CoreError> {
    command.env_clear().stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null());
    let mut child = command.spawn().map_err(|_| CoreError::Overloaded)?;
    let started = Instant::now();
    loop {
        match child.try_wait().map_err(|_| CoreError::Overloaded)? {
            Some(status) => {
                if !status.success() { return Err(CoreError::Invalid); }
                use std::io::Read;
                let mut bytes=Vec::new();
                child.stdout.take().ok_or(CoreError::Overloaded)?.take(4097).read_to_end(&mut bytes).map_err(|_|CoreError::Overloaded)?;
                if bytes.len()>4096 {return Err(CoreError::Invalid);}
                return String::from_utf8(bytes).map_err(|_|CoreError::Invalid);
            }
            None if started.elapsed() >= Duration::from_secs(120) => {
                let _=child.kill(); let _=child.wait(); return Err(CoreError::Overloaded);
            }
            None => thread::sleep(Duration::from_millis(50)),
        }
    }
}
fn regular_file(path: &Path) -> Result<(), CoreError> {
    let meta=fs::symlink_metadata(path).map_err(|_|CoreError::Invalid)?;
    if !meta.file_type().is_file() { return Err(CoreError::Invalid); }
    Ok(())
}
fn pages(qpdf: &Path, file: &Path) -> Result<u32, CoreError> {
    regular_file(file)?;
    let mut command=Command::new(qpdf); command.arg("--show-npages").arg(file);
    let count=run(command)?.trim().parse::<u32>().map_err(|_|CoreError::Invalid)?;
    if count==0 {return Err(CoreError::Invalid);} Ok(count)
}
/// Merge `piece-000001.pdf`, ... in numerical order and apply `folios.pdf`.
/// The files must be immutable snapshots in a new, private job directory.
/// `folios.pdf` is generated separately with one page for each measured page.
/// This function MUST run inside the dedicated no-network, resource-limited
/// worker; it is not an endpoint and does not provide isolation on its own.
pub fn compose_prepared_pdf(qpdf: &Path, root: &Path, pieces: u32) -> Result<u32, CoreError> {
    if !qpdf.is_absolute() || !root.is_absolute() || pieces==0 || pieces>4000 {return Err(CoreError::Invalid);}
    regular_file(qpdf)?;
    let meta=fs::symlink_metadata(root).map_err(|_|CoreError::Invalid)?;
    if !meta.file_type().is_dir() {return Err(CoreError::Invalid);}
    let merged=root.join("merged.pdf"); let output=root.join("result.pdf"); let folios=root.join("folios.pdf");
    if merged.exists() || output.exists() {return Err(CoreError::Conflict);}
    let mut sum=0u32; let mut command=Command::new(qpdf); command.args(["--empty","--pages"]);
    for i in 1..=pieces {
        let part=root.join(format!("piece-{i:06}.pdf"));
        sum=sum.checked_add(pages(qpdf,&part)?).ok_or(CoreError::Invalid)?;
        command.arg(part);
    }
    if pages(qpdf,&folios)? != sum {return Err(CoreError::Invalid);}
    command.arg("--").arg(&merged); run(command)?;
    if pages(qpdf,&merged)? != sum {return Err(CoreError::Invalid);}
    let mut overlay=Command::new(qpdf); overlay.arg(&merged).arg("--overlay").arg(&folios).arg("--").arg(&output); run(overlay)?;
    if pages(qpdf,&output)? != sum {return Err(CoreError::Invalid);}
    validate_final_pdf_budget(fs::metadata(&output).map_err(|_|CoreError::Invalid)?.len())?;
    Ok(sum)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn merges_and_numbers_when_test_binaries_are_available() {
        let (Some(typst),Some(qpdf))=(std::env::var_os("BASTION_TYPST_BIN"),std::env::var_os("BASTION_QPDF_BIN")) else {return;};
        let root=std::env::temp_dir().join(format!("bastion-compose-{}",std::process::id()));
        fs::create_dir_all(&root).unwrap();
        for (filename,source) in [("piece-000001", "#page(width: 612pt, height: 792pt)[Primera]"),("piece-000002", "#page(width: 792pt, height: 612pt)[Segunda]"),("folios", "#page(width: 612pt, height: 792pt)[#place(bottom + center)[1]]\n#page(width: 792pt, height: 612pt)[#place(bottom + center)[2]]")] {
            fs::write(root.join(format!("{filename}.typ")),source).unwrap();
            let status=Command::new(&typst).arg("compile").arg("--root").arg(&root).arg(root.join(format!("{filename}.typ"))).arg(root.join(format!("{filename}.pdf"))).status().unwrap();
            assert!(status.success());
        }
        assert_eq!(compose_prepared_pdf(Path::new(&qpdf), &root, 2).unwrap(),2);
        assert!(root.join("result.pdf").is_file());
        assert!(compose_prepared_pdf(Path::new(&qpdf), &root, 2).is_err());
        let _=fs::remove_dir_all(&root);
    }
}
