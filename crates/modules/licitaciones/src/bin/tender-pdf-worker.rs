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
#[cfg(any(test, target_os = "linux"))]
fn status_is_restricted(status: &str) -> bool {
    let field = |key: &str| {
        status
            .lines()
            .find_map(|line| line.strip_prefix(key))
            .map(str::trim)
    };
    let uid = field("Uid:")
        .and_then(|v| v.split_whitespace().nth(1))
        .and_then(|v| v.parse::<u32>().ok());
    let capabilities = ["CapEff:", "CapPrm:", "CapInh:", "CapAmb:"]
        .iter()
        .all(|key| field(key).and_then(|v| u64::from_str_radix(v, 16).ok()) == Some(0));
    uid.is_some_and(|uid| uid != 0) && capabilities && field("NoNewPrivs:") == Some("1")
}
#[cfg(any(test, target_os = "linux"))]
fn parse_namespace(value: &str) -> Option<&str> {
    let value = value.trim();
    let digits = value.strip_prefix("net:[")?.strip_suffix(']')?;
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    Some(value)
}
#[cfg(target_os = "linux")]
fn trusted_host_namespace() -> Option<PathBuf> {
    use std::os::unix::fs::MetadataExt;
    if let Ok(link) = std::fs::read_link("/proc/1/ns/net") {
        return Some(link);
    }
    // A root-owned service creates this marker from PID 1 on every boot.
    // Neither the file nor its fixed parent may be writable by the worker.
    let parent = std::fs::symlink_metadata("/run").ok()?;
    let file = std::fs::symlink_metadata("/run/bastion-tender-host-netns").ok()?;
    if !parent.file_type().is_dir()
        || parent.uid() != 0
        || parent.mode() & 0o022 != 0
        || !file.file_type().is_file()
        || file.uid() != 0
        || file.mode() & 0o022 != 0
        || file.len() > 80
    {
        return None;
    }
    let text = std::fs::read_to_string("/run/bastion-tender-host-netns").ok()?;
    Some(PathBuf::from(parse_namespace(&text)?))
}
#[cfg(target_os = "linux")]
fn network_is_private_and_nonroot() -> bool {
    let Ok(own) = std::fs::read_link("/proc/self/ns/net") else {
        return false;
    };
    let Some(init) = trusted_host_namespace() else {
        return false;
    };
    let Ok(status) = std::fs::read_to_string("/proc/self/status") else {
        return false;
    };
    own != init && status_is_restricted(&status)
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
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() == 1 && args[0] == "--check-sandbox" {
        if network_is_private_and_nonroot() {
            println!("sandbox-ok");
            return;
        }
        eprintln!("No se cumplen las restricciones de aislamiento.");
        std::process::exit(1);
    }
    if !args.is_empty() {
        eprintln!("Opción no admitida.");
        std::process::exit(1);
    }
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
    #[test]
    fn process_status_requires_no_privilege_or_capabilities() {
        let good = "Uid:\t1001\t1001\t1001\t1001\nCapEff:\t0000000000000000\nCapPrm:\t0000000000000000\nCapInh:\t0000000000000000\nCapAmb:\t0000000000000000\nNoNewPrivs:\t1\n";
        assert!(status_is_restricted(good));
        assert!(!status_is_restricted(
            &good.replace("1001\t1001", "1001\t0")
        ));
        assert!(!status_is_restricted(&good.replace(
            "CapEff:\t0000000000000000",
            "CapEff:\t0000000000000001"
        )));
        assert!(!status_is_restricted(
            &good.replace("NoNewPrivs:\t1", "NoNewPrivs:\t0")
        ));
        assert!(!status_is_restricted(""));
    }
    #[test]
    fn namespace_marker_requires_exact_kernel_identifier() {
        assert_eq!(
            parse_namespace("net:[4026531993]\n"),
            Some("net:[4026531993]")
        );
        assert!(parse_namespace("net:[]").is_none());
        assert!(parse_namespace("net:[1] other").is_none());
        assert!(parse_namespace("net:[1/2]").is_none());
    }
}
