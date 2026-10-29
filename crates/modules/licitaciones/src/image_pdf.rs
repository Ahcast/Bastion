//! Fixed Typst source for a single already-authorized image snapshot.
//! The worker supplies `attachment.bin`, not a filename from the user.
//! BMP and TIFF need a controlled conversion before reaching this renderer.

pub const IMAGE_PAGE_TYPST: &str = r#"#set page(paper: "us-letter", margin: (x: 1.3cm, y: 1.5cm))
#align(center + horizon)[#image("attachment.bin", width: 100%, height: 100%, fit: "contain")]
"#;

/// Convert selected BMP/TIFF files only inside the isolated worker.
pub fn convert_prepared_images(
    root: &std::path::Path,
    converter: &std::path::Path,
    numbers: &[u32],
) -> Result<(), bastion_core::CoreError> {
    use bastion_core::CoreError;
    use std::{collections::BTreeSet, fs, process::Command};
    if !root.is_absolute()
        || !converter.is_absolute()
        || !fs::symlink_metadata(root)
            .map_err(|_| CoreError::Invalid)?
            .file_type()
            .is_dir()
    {
        return Err(CoreError::Invalid);
    }
    crate::pdf_compose::regular_file(converter)?;
    let mut seen = BTreeSet::new();
    for n in numbers {
        if !(3..=4000).contains(n) || !seen.insert(*n) {
            return Err(CoreError::Invalid);
        }
        crate::pdf_compose::regular_file(&root.join(format!("attachment-{n:06}.bin")))?;
        if fs::symlink_metadata(root.join(format!("source-{n:06}.typ"))).is_ok() {
            return Err(CoreError::Conflict);
        }
        let mut command = Command::new("/usr/bin/python3");
        command
            .arg("-I")
            .arg(converter)
            .arg(root)
            .arg(n.to_string());
        crate::pdf_compose::run(command)?;
        crate::pdf_compose::regular_file(&root.join(format!("source-{n:06}.typ")))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    #[test]
    fn renders_a_real_image_when_typst_is_available() {
        let Some(bin) = std::env::var_os("BASTION_TYPST_BIN") else {
            return;
        };
        let root = std::env::temp_dir().join(format!("bastion-image-pdf-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let image=STANDARD.decode("iVBORw0KGgoAAAANSUhEUgAAAAIAAAADCAIAAAA2iEnWAAAAFElEQVR4nGP0qZjJwMDAxAAGUAoAGe4BY53ckeUAAAAASUVORK5CYII=").unwrap();
        std::fs::write(root.join("attachment.bin"), image).unwrap();
        std::fs::write(root.join("image.typ"), IMAGE_PAGE_TYPST).unwrap();
        let output = std::process::Command::new(bin)
            .arg("compile")
            .arg("--root")
            .arg(&root)
            .arg(root.join("image.typ"))
            .arg(root.join("image.pdf"))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let bytes = std::fs::read(root.join("image.pdf")).unwrap();
        assert!(bytes.starts_with(b"%PDF-"));
        let _ = std::fs::remove_dir_all(&root);
    }
}
