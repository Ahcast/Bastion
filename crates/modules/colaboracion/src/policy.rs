use bastion_core::CoreError;
use serde::{Deserialize, Serialize};

pub const BLOCKED_EXTENSIONS: &[&str] = &[
    "exe", "dll", "com", "bat", "cmd", "ps1", "psm1", "psd1", "vbs", "vbe", "js", "jse", "mjs",
    "cjs", "sh", "bash", "zsh", "py", "pyc", "pl", "pm", "rb", "lua", "php", "phtml", "asp",
    "aspx", "jsp", "jar", "msi", "scr", "hta", "lnk", "url", "reg", "wasm", "html", "htm", "svg",
    "docm", "xlsm", "pptm", "dotm", "xlam", "ppam", "ppsm", "xlsb", "wsf", "wsh", "sct", "scf",
    "gadget", "so", "dylib", "elf", "apk", "app", "deb", "rpm", "dmg", "iso", "img",
];
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FilePolicy {
    pub max_file_mb: u32,
    pub trash_retention_days: u32,
    pub allowed_extensions: Vec<String>,
    pub blocked_extensions: Vec<String>,
}
impl Default for FilePolicy {
    fn default() -> Self {
        Self {
            max_file_mb: 50,
            trash_retention_days: 30,
            allowed_extensions: [
                "pdf", "doc", "docx", "xls", "xlsx", "ppt", "pptx", "odt", "ods", "odp", "txt",
                "csv", "rtf", "typ", "png", "jpg", "jpeg", "gif", "webp", "bmp", "tif", "tiff",
                "zip",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect(),
            blocked_extensions: BLOCKED_EXTENSIONS.iter().map(|s| (*s).to_owned()).collect(),
        }
    }
}
impl FilePolicy {
    pub fn validate(&self) -> Result<(), CoreError> {
        if !(1..=100).contains(&self.max_file_mb)
            || !(1..=365).contains(&self.trash_retention_days)
            || self.allowed_extensions.is_empty()
            || self.allowed_extensions.len() > 40
            || self.blocked_extensions.len() > 100
            || self
                .allowed_extensions
                .iter()
                .any(|ext| expected_mime(ext).is_none())
            || self.blocked_extensions.iter().any(|ext| {
                ext.is_empty()
                    || ext.len() > 20
                    || !ext
                        .bytes()
                        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
            })
            || self
                .allowed_extensions
                .iter()
                .any(|ext| self.blocked_extensions.contains(ext))
        {
            return Err(CoreError::Invalid);
        }
        for extensions in [&self.allowed_extensions, &self.blocked_extensions] {
            let mut seen = std::collections::BTreeSet::new();
            if extensions.iter().any(|ext| !seen.insert(ext)) {
                return Err(CoreError::Invalid);
            }
        }
        Ok(())
    }
    pub fn max_bytes(&self) -> usize {
        self.max_file_mb as usize * 1024 * 1024
    }
    pub fn check(&self, filename: &str, mime: &str, bytes: &[u8]) -> Result<(), CoreError> {
        let name = filename.to_lowercase();
        let pieces: Vec<_> = name.split('.').collect();
        if pieces.iter().any(|ext| {
            BLOCKED_EXTENSIONS.contains(ext) || self.blocked_extensions.iter().any(|b| b == ext)
        }) {
            return Err(CoreError::Invalid);
        }
        let ext = pieces.last().copied().unwrap_or("");
        if !self.allowed_extensions.iter().any(|allowed| allowed == ext)
            || !expected_mime(ext).is_some_and(|mimes| mimes.contains(&mime))
        {
            return Err(CoreError::Invalid);
        }
        if bytes.starts_with(b"MZ") || bytes.starts_with(b"\x7fELF") || bytes.starts_with(b"#!") {
            return Err(CoreError::Invalid);
        }
        if matches!(ext, "txt" | "csv" | "typ") {
            let text = std::str::from_utf8(bytes)
                .map_err(|_| CoreError::Invalid)?
                .trim_start()
                .to_ascii_lowercase();
            if [
                "<?php",
                "<script",
                "<!doctype html",
                "<html",
                "<svg",
                "@echo",
            ]
            .iter()
            .any(|start| text.starts_with(start))
            {
                return Err(CoreError::Invalid);
            }
        }
        if matches!(
            ext,
            "zip" | "docx" | "xlsx" | "pptx" | "odt" | "ods" | "odp"
        ) {
            check_zip_names(bytes, self)?;
        }
        Ok(())
    }
}
fn expected_mime(ext: &str) -> Option<&'static [&'static str]> {
    Some(match ext {
        "pdf" => &["application/pdf"],
        "doc" => &["application/msword"],
        "xls" => &["application/vnd.ms-excel"],
        "ppt" => &["application/vnd.ms-powerpoint"],
        "docx" => &["application/vnd.openxmlformats-officedocument.wordprocessingml.document"],
        "xlsx" => &["application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"],
        "pptx" => &["application/vnd.openxmlformats-officedocument.presentationml.presentation"],
        "odt" => &["application/vnd.oasis.opendocument.text"],
        "ods" => &["application/vnd.oasis.opendocument.spreadsheet"],
        "odp" => &["application/vnd.oasis.opendocument.presentation"],
        "txt" => &["text/plain"],
        "typ" => &["text/plain"],
        "csv" => &["text/plain", "text/csv"],
        "rtf" => &["text/rtf", "application/rtf"],
        "png" => &["image/png"],
        "jpg" | "jpeg" => &["image/jpeg"],
        "gif" => &["image/gif"],
        "webp" => &["image/webp"],
        "bmp" => &["image/bmp", "image/x-ms-bmp"],
        "tif" | "tiff" => &["image/tiff"],
        "zip" => &["application/zip", "application/x-zip"],
        _ => return None,
    })
}
// Inspect the bounded ZIP central directory without extracting user files.
// Encrypted, split, ZIP64, nested archives and >2000 entries fail closed.
fn check_zip_names(bytes: &[u8], policy: &FilePolicy) -> Result<(), CoreError> {
    let invalid = || CoreError::Invalid;
    let end = bytes.len().checked_sub(22).ok_or_else(invalid)?;
    let eocd = (end.saturating_sub(65535)..=end)
        .rev()
        .find(|at| bytes.get(*at..*at + 4) == Some(b"PK\x05\x06"))
        .ok_or_else(invalid)?;
    let word = |at: usize| -> Result<usize, CoreError> {
        let b = bytes.get(at..at + 2).ok_or_else(invalid)?;
        Ok(u16::from_le_bytes([b[0], b[1]]) as usize)
    };
    let dword = |at: usize| -> Result<usize, CoreError> {
        let b = bytes.get(at..at + 4).ok_or_else(invalid)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize)
    };
    let count = word(eocd + 10)?;
    let mut at = dword(eocd + 16)?;
    if word(eocd + 4)? != 0
        || word(eocd + 6)? != 0
        || word(eocd + 8)? != count
        || count > 2000
        || at + dword(eocd + 12)? != eocd
        || eocd + 22 + word(eocd + 20)? != bytes.len()
    {
        return Err(invalid());
    }
    let mut total = 0u64;
    for _ in 0..count {
        if bytes.get(at..at + 4) != Some(b"PK\x01\x02") || word(at + 8)? & 1 != 0 {
            return Err(invalid());
        }
        let size = dword(at + 24)?;
        total = total.checked_add(size as u64).ok_or_else(invalid)?;
        if total > 500 * 1024 * 1024 {
            return Err(invalid());
        }
        let length = word(at + 28)?;
        let name = std::str::from_utf8(bytes.get(at + 46..at + 46 + length).ok_or_else(invalid)?)
            .map_err(|_| invalid())?
            .to_lowercase();
        if name.starts_with('/')
            || name.contains('\\')
            || name.contains(':')
            || name.split('/').any(|part| part == "..")
            || name.chars().any(char::is_control)
        {
            return Err(invalid());
        }
        let base = name.rsplit('/').next().unwrap_or("");
        if base.split('.').any(|ext| {
            BLOCKED_EXTENSIONS.contains(&ext) || policy.blocked_extensions.iter().any(|v| v == ext)
        }) || ["zip", "7z", "rar", "tar", "gz"].contains(&base.rsplit('.').next().unwrap_or(""))
            || base == "vbaproject.bin"
        {
            return Err(invalid());
        }
        at = at
            .checked_add(46 + length + word(at + 30)? + word(at + 32)?)
            .ok_or_else(invalid)?;
        if at > eocd {
            return Err(invalid());
        }
    }
    if at != eocd {
        return Err(invalid());
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn defaults_and_mime_extension_pairs_block_disguised_scripts() {
        let p = FilePolicy::default();
        p.validate().unwrap();
        assert_eq!(p.max_file_mb, 50);
        assert_eq!(p.trash_retention_days, 30);
        assert!(
            p.check("contrato.pdf", "application/pdf", b"%PDF-1.7")
                .is_ok()
        );
        assert!(p.allowed_extensions.iter().any(|ext| ext == "typ"));
        assert!(
            p.check(
                "plantilla.typ",
                "text/plain",
                b"= Cotizacion\n#let total = 10\n"
            )
            .is_ok()
        );
        assert!(
            p.check("captura.jpeg", "image/jpeg", &[0xff, 0xd8, 0xff])
                .is_ok()
        );
        assert!(
            p.check("captura.png", "image/png", b"\x89PNG\r\n\x1a\n")
                .is_ok()
        );
        for (name, mime, bytes) in [
            ("foto.jpg", "application/pdf", b"%PDF-".as_slice()),
            ("factura.exe.pdf", "application/pdf", b"%PDF-"),
            ("foto.png", "image/png", b"MZbinary"),
            ("script.txt", "text/plain", b"#!/bin/sh"),
            ("correo.txt", "text/plain", b"<script>x</script>"),
            (
                "malicioso.typ",
                "text/plain",
                b"  <!doctype html><script>x</script>",
            ),
        ] {
            assert!(p.check(name, mime, bytes).is_err());
        }
        let mut bad = p;
        bad.max_file_mb = 101;
        assert!(bad.validate().is_err());
    }
    #[test]
    fn zip_directory_rejects_hidden_executables_and_traversal_without_extraction() {
        fn archive(name: &str, flags: u16) -> Vec<u8> {
            let mut b = vec![0u8; 46];
            b[0..4].copy_from_slice(b"PK\x01\x02");
            b[8..10].copy_from_slice(&flags.to_le_bytes());
            b[28..30].copy_from_slice(&(name.len() as u16).to_le_bytes());
            b.extend_from_slice(name.as_bytes());
            let size = b.len();
            let mut end = vec![0u8; 22];
            end[0..4].copy_from_slice(b"PK\x05\x06");
            end[8..10].copy_from_slice(&1u16.to_le_bytes());
            end[10..12].copy_from_slice(&1u16.to_le_bytes());
            end[12..16].copy_from_slice(&(size as u32).to_le_bytes());
            b.extend_from_slice(&end);
            b
        }
        let p = FilePolicy::default();
        assert!(check_zip_names(&archive("manual.pdf", 0), &p).is_ok());
        for name in [
            "folder/invoice.exe",
            "../manual.pdf",
            "nested.zip",
            "word/vbaProject.bin",
            "folder/script.ps1",
            "file.pdf:payload.exe",
        ] {
            assert!(check_zip_names(&archive(name, 0), &p).is_err(), "{name}");
        }
        assert!(check_zip_names(&archive("manual.pdf", 1), &p).is_err());
        assert!(check_zip_names(b"PK invalid", &p).is_err());
    }
}
