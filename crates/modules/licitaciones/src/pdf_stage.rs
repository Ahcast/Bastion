//! Prepare private worker inputs. This does not run Typst or grant filesystem access.
use crate::{MAX_EXPEDIENT_BYTES, RenderPiece, TenderService, pdf_layout::LayoutRequest};
use bastion_core::CoreError;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

/// Trusted, privately installed templates from the user's approved first group.
/// These are loaded by the service configuration, never supplied by an API caller.
pub struct TenderTemplateBundle {
    pub bid_offer: String,
    pub std_offer: String,
    pub bid_lot: String,
    pub std_lot: String,
    pub functions: String,
    pub legal_integridad: String,
    pub legal_oferente: String,
}

impl TenderTemplateBundle {
    /// Read the fixed first-group templates from a root-owned, non-writable
    /// deployment directory. No template path comes from an API request.
    pub fn from_private_directory(root: &Path) -> Result<Self, CoreError> {
        #[cfg(not(target_os = "linux"))]
        {
            let _ = root;
            return Err(CoreError::Invalid);
        }
        #[cfg(target_os = "linux")]
        {
            use std::{io::Read, os::unix::fs::MetadataExt};
            if !root.is_absolute()
                || fs::canonicalize(root).map_err(|_| CoreError::Invalid)? != root
            {
                return Err(CoreError::Invalid);
            }
            for path in root.ancestors() {
                let meta = fs::symlink_metadata(path).map_err(|_| CoreError::Invalid)?;
                if !meta.file_type().is_dir() || meta.uid() != 0 || meta.mode() & 0o022 != 0 {
                    return Err(CoreError::Invalid);
                }
            }
            let read = |name: &str| -> Result<String, CoreError> {
                let path = root.join(name);
                let meta = fs::symlink_metadata(&path).map_err(|_| CoreError::Invalid)?;
                if !meta.file_type().is_file()
                    || meta.uid() != 0
                    || meta.mode() & 0o022 != 0
                    || meta.len() > 2_000_000
                {
                    return Err(CoreError::Invalid);
                }
                let mut bytes = Vec::new();
                fs::File::open(path)
                    .map_err(|_| CoreError::Invalid)?
                    .take(2_000_001)
                    .read_to_end(&mut bytes)
                    .map_err(|_| CoreError::Invalid)?;
                if bytes.len() > 2_000_000 {
                    return Err(CoreError::Invalid);
                }
                String::from_utf8(bytes).map_err(|_| CoreError::Invalid)
            };
            Ok(Self {
                bid_offer: read("carta-de-oferta.typ")?,
                std_offer: read("presentacion-de-oferta.typ")?,
                bid_lot: read("lote1-bid.typ")?,
                std_lot: read("lote1-std.typ")?,
                functions: read("funciones.typ")?,
                legal_integridad: read("dj-integridad.typ")?,
                legal_oferente: read("informacion_sobre_el_oferente.typ")?,
            })
        }
    }
}

#[derive(Serialize)]
pub struct WorkerRequest {
    pub pieces: u32,
    pub sources: Vec<u32>,
    pub global_sources: Vec<u32>,
    pub conversions: Vec<u32>,
    pub layout: LayoutRequest,
    #[serde(skip)]
    pub(crate) reference_checks: Vec<(u64, Vec<u8>)>,
}

fn private_empty_root(root: &Path) -> Result<(), CoreError> {
    if !root.is_absolute() {
        return Err(CoreError::Invalid);
    }
    let meta = fs::symlink_metadata(root).map_err(|_| CoreError::Invalid)?;
    if !meta.file_type().is_dir()
        || fs::read_dir(root)
            .map_err(|_| CoreError::Invalid)?
            .next()
            .is_some()
    {
        return Err(CoreError::Invalid);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if meta.permissions().mode() & 0o077 != 0 {
            return Err(CoreError::Invalid);
        }
    }
    Ok(())
}
async fn create(root: &Path, name: &str, data: Vec<u8>) -> Result<(), CoreError> {
    if name.is_empty()
        || name.len() > 128
        || !name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
        || name.starts_with('.')
    {
        return Err(CoreError::Invalid);
    }
    let path = root.join(name);
    tokio::task::spawn_blocking(move || {
        let mut f = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(|_| CoreError::Conflict)?;
        f.write_all(&data).map_err(|_| CoreError::Overloaded)
    })
    .await
    .map_err(|_| CoreError::Overloaded)?
}

impl TenderService {
    /// The caller must supply an exclusively owned private job directory and
    /// remove it after success or failure. It must start an isolated worker.
    pub async fn stage_pdf_inputs(
        &self,
        actor: u64,
        id: u64,
        expected: u64,
        root: PathBuf,
        bundle: &TenderTemplateBundle,
    ) -> Result<WorkerRequest, CoreError> {
        private_empty_root(&root)?;
        let detail = self.detail(actor, id).await?;
        if detail.version != expected {
            return Err(CoreError::PreconditionFailed);
        }
        let style = detail
            .variables
            .iter()
            .find(|v| v.key == "stylo")
            .and_then(|v| v.value.as_ref())
            .and_then(|v| v.as_str())
            .ok_or(CoreError::Invalid)?;
        let (offer, lot_template) = match style {
            "bid" => (&bundle.bid_offer, &bundle.bid_lot),
            "std" => (&bundle.std_offer, &bundle.std_lot),
            _ => return Err(CoreError::Invalid),
        };
        let snapshot = self
            .prepare_typst_snapshot(actor, id, expected, 1, lot_template)
            .await?;
        let plan = snapshot.render_plan()?;
        let attachments = self.attachments(actor, id).await?;
        if attachments.expedient_version != expected {
            return Err(CoreError::PreconditionFailed);
        }
        for (name, data) in &snapshot.sources.files {
            create(&root, name, data.clone()).await?;
        }
        create(&root, "funciones.typ", bundle.functions.as_bytes().to_vec()).await?;
        let mut source_numbers = Vec::new();
        let mut global_numbers = Vec::new();
        let mut conversions = Vec::new();
        let mut referenced = Vec::<(u64, Vec<u8>)>::new();
        let mut total = 0u64;
        for piece in &plan.pieces {
            match piece {
                RenderPiece::Offer { number, .. } => {
                    create(
                        &root,
                        &format!("source-{number:06}.typ"),
                        offer.as_bytes().to_vec(),
                    )
                    .await?;
                    source_numbers.push(*number);
                }
                RenderPiece::Lot { number, lot, style } => {
                    let name = format!("lote{lot}-{style}.typ");
                    let data = snapshot
                        .sources
                        .files
                        .get(&name)
                        .ok_or(CoreError::Invalid)?;
                    create(&root, &format!("source-{number:06}.typ"), data.clone()).await?;
                    source_numbers.push(*number);
                }
                RenderPiece::InitialForm { number, kind } => {
                    let source = match kind.as_str() {
                        "integridad" => &bundle.legal_integridad,
                        "oferente" => &bundle.legal_oferente,
                        _ => return Err(CoreError::Invalid),
                    };
                    create(
                        &root,
                        &format!("source-{number:06}.typ"),
                        source.as_bytes().to_vec(),
                    )
                    .await?;
                    source_numbers.push(*number);
                    global_numbers.push(*number);
                }
                RenderPiece::Form {
                    number, form_id, ..
                } => {
                    let file = self.render_template(actor, id, expected, *form_id).await?;
                    let hash = Sha256::digest(file.source.as_bytes()).to_vec();
                    referenced.push((file.source_id, hash));
                    create(
                        &root,
                        &format!("source-{number:06}.typ"),
                        file.source.into_bytes(),
                    )
                    .await?;
                    source_numbers.push(*number);
                    global_numbers.push(*number);
                }
                RenderPiece::Attachment {
                    number,
                    id: attachment_id,
                } => {
                    let metadata = attachments
                        .items
                        .iter()
                        .find(|f| f.id == *attachment_id)
                        .ok_or(CoreError::NotFound)?;
                    let file = self
                        .render_attachment(actor, id, expected, attachment_id)
                        .await?;
                    total = total
                        .checked_add(file.bytes.len() as u64)
                        .filter(|n| *n <= MAX_EXPEDIENT_BYTES)
                        .ok_or(CoreError::Invalid)?;
                    if metadata.is_reference {
                        referenced.push((
                            metadata.source_id.ok_or(CoreError::Invalid)?,
                            Sha256::digest(&file.bytes).to_vec(),
                        ));
                    }
                    if file.mime == "application/pdf" {
                        if !file.bytes.starts_with(b"%PDF-") {
                            return Err(CoreError::Invalid);
                        }
                        create(&root, &format!("piece-{number:06}.pdf"), file.bytes).await?;
                    } else if matches!(file.mime.as_str(), "image/bmp" | "image/tiff") {
                        create(&root, &format!("attachment-{number:06}.bin"), file.bytes).await?;
                        source_numbers.push(*number);
                        conversions.push(*number);
                    } else if matches!(
                        file.mime.as_str(),
                        "image/png" | "image/jpeg" | "image/gif" | "image/webp"
                    ) {
                        let name = format!("attachment-{number:06}.bin");
                        create(&root, &name, file.bytes).await?;
                        let source = format!(
                            "#set page(paper: \"us-letter\", margin: (x: 1.3cm, y: 1.5cm))\n#align(center + horizon)[#image(\"{name}\", width: 100%, height: 100%, fit: \"contain\")]\n"
                        );
                        create(
                            &root,
                            &format!("source-{number:06}.typ"),
                            source.into_bytes(),
                        )
                        .await?;
                        source_numbers.push(*number);
                    } else {
                        return Err(CoreError::Invalid);
                    }
                }
            }
        }
        let mut tx = self.begin(actor, "ver").await?;
        let current = crate::service::load(&mut tx, id).await?;
        if current.version != expected {
            return Err(CoreError::PreconditionFailed);
        }
        for (source_id, hash) in &referenced {
            self.library
                .validate_file_snapshot_in_transaction(&mut tx, actor, *source_id, hash)
                .await?;
        }
        tx.commit().await?;
        Ok(WorkerRequest {
            pieces: u32::try_from(plan.pieces.len()).map_err(|_| CoreError::Invalid)? + 2,
            sources: source_numbers,
            global_sources: global_numbers,
            conversions,
            layout: plan.layout,
            reference_checks: referenced,
        })
    }
}
