//! Single-job PDF worker. The service manager must additionally give this
//! process a dedicated identity, private job mount, CPU/memory/time limits,
//! and no credentials. This worker refuses a shared network namespace.
#![forbid(unsafe_code)]
use serde::Deserialize;
use std::{
    io::{self, Read},
    path::PathBuf,
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    pieces: u32,
}

fn request(bytes: &[u8]) -> Result<Request, ()> {
    if bytes.len() > 4096 {
        return Err(());
    }
    let value: Request = serde_json::from_slice(bytes).map_err(|_| ())?;
    if value.pieces == 0 || value.pieces > 4000 {
        return Err(());
    }
    Ok(value)
}
#[cfg(target_os = "linux")]
fn network_is_private_and_nonroot() -> bool {
    let Ok(own) = std::fs::read_link("/proc/self/ns/net") else {
        return false;
    };
    let Ok(init) = std::fs::read_link("/proc/1/ns/net") else {
        return false;
    };
    let Ok(status) = std::fs::read_to_string("/proc/self/status") else {
        return false;
    };
    let nonroot = status
        .lines()
        .find_map(|line| line.strip_prefix("Uid:"))
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|uid| uid.parse::<u32>().ok())
        .is_some_and(|uid| uid != 0);
    own != init && nonroot
}
#[cfg(not(target_os = "linux"))]
fn network_is_private_and_nonroot() -> bool {
    false
}
fn trusted_path(key: &str) -> Result<PathBuf, ()> {
    let value = std::env::var_os(key).map(PathBuf::from).ok_or(())?;
    if !value.is_absolute() {
        return Err(());
    }
    Ok(value)
}
fn perform() -> Result<u32, ()> {
    if !network_is_private_and_nonroot() {
        return Err(());
    }
    let root = trusted_path("BASTION_WORKER_ROOT")?;
    let qpdf = trusted_path("BASTION_QPDF_BIN")?;
    let typst = trusted_path("BASTION_TYPST_BIN")?;
    let pdfinfo = trusted_path("BASTION_PDFINFO_BIN")?;
    let mut bytes = Vec::new();
    io::stdin()
        .lock()
        .take(4097)
        .read_to_end(&mut bytes)
        .map_err(|_| ())?;
    let job = request(&bytes)?;
    bastion_licitaciones::pdf_compose::measure_and_compose_pdf(
        &typst, &pdfinfo, &qpdf, &root, job.pieces,
    )
    .map_err(|_| ())
}
fn main() {
    match perform() {
        Ok(pages) => println!("{{\"pages\":{pages}}}"),
        Err(()) => {
            eprintln!("No se pudo componer el PDF en el entorno aislado.");
            std::process::exit(1);
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn worker_accepts_only_a_bounded_piece_count() {
        assert!(request(br#"{"pieces":3}"#).is_ok());
        assert!(request(br#"{"pieces":0}"#).is_err());
        assert!(request(br#"{"pieces":4001}"#).is_err());
        assert!(request(br#"{"pieces":1,"path":"/etc"}"#).is_err());
        assert!(request(&vec![b' '; 4097]).is_err());
    }
}
