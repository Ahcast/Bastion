//! Fixed Typst source for a single already-authorized image snapshot.
//! The worker supplies `attachment.bin`, not a filename from the user.
//! BMP and TIFF need a controlled conversion before reaching this renderer.

pub const IMAGE_PAGE_TYPST: &str = r#"#set page(paper: "us-letter", margin: (x: 1.3cm, y: 1.5cm))
#align(center + horizon)[#image("attachment.bin", width: 100%, height: 100%, fit: "contain")]
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    #[test]
    fn renders_a_real_image_when_typst_is_available() {
        let Some(bin)=std::env::var_os("BASTION_TYPST_BIN") else {return;};
        let root=std::env::temp_dir().join(format!("bastion-image-pdf-{}",std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let image=STANDARD.decode("iVBORw0KGgoAAAANSUhEUgAAAAIAAAADCAIAAAA2iEnWAAAAFElEQVR4nGP0qZjJwMDAxAAGUAoAGe4BY53ckeUAAAAASUVORK5CYII=").unwrap();
        std::fs::write(root.join("attachment.bin"),image).unwrap();
        std::fs::write(root.join("image.typ"),IMAGE_PAGE_TYPST).unwrap();
        let output=std::process::Command::new(bin).arg("compile").arg("--root").arg(&root).arg(root.join("image.typ")).arg(root.join("image.pdf")).output().unwrap();
        assert!(output.status.success(),"{}",String::from_utf8_lossy(&output.stderr));
        let bytes=std::fs::read(root.join("image.pdf")).unwrap();
        assert!(bytes.starts_with(b"%PDF-"));
        let _=std::fs::remove_dir_all(&root);
    }
}
