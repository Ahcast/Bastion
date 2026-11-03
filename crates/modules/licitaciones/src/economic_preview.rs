use crate::{AttachmentContent, TenderService};
use bastion_core::CoreError;

impl TenderService {
    #[cfg(not(target_os = "linux"))]
    pub async fn preview_economic_offer(
        &self,
        _actor: u64,
        _id: u64,
        _expected: u64,
    ) -> Result<AttachmentContent, CoreError> {
        Err(CoreError::Overloaded)
    }

    /// Render only approved economic sources in the existing isolated worker.
    /// Preview bytes are returned directly, without replacing the final PDF.
    #[cfg(target_os = "linux")]
    pub async fn preview_economic_offer(
        &self,
        actor: u64,
        id: u64,
        expected: u64,
    ) -> Result<AttachmentContent, CoreError> {
        use base64::{Engine, engine::general_purpose::STANDARD};
        use bastion_core::DocumentService;
        use std::{fs, io::Read, os::unix::fs::DirBuilderExt, path::PathBuf};
        let mut tx = self.begin(actor, "editar").await?;
        let exp = crate::service::load(&mut tx, id).await?;
        if exp.version != expected {
            return Err(CoreError::PreconditionFailed);
        }
        tx.commit().await?;
        let style = exp
            .variables
            .iter()
            .find(|v| v.key == "stylo")
            .and_then(|v| v.value.as_ref())
            .and_then(|v| v.as_str())
            .ok_or(CoreError::Invalid)?;
        let templates = std::env::var_os("BASTION_TENDER_TEMPLATE_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/opt/bastion-pdf/private-templates-20261029"));
        let bundle = crate::TenderTemplateBundle::from_private_directory(&templates)?;
        let (offer, lot) = bundle.for_style(style)?;
        let snapshot = self
            .prepare_typst_snapshot_scoped(actor, id, expected, 1, lot, true)
            .await?;
        let identifier = DocumentService::object_suffix();
        let root = PathBuf::from("/var/lib/bastion-tender/incoming").join(&identifier);
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&root)
            .map_err(|_| CoreError::Overloaded)?;
        let _cleanup = crate::pdf_jobs::PrivateJobDir(root.clone());
        for (name, bytes) in &snapshot.sources.files {
            fs::write(root.join(name), bytes).map_err(|_| CoreError::Overloaded)?;
        }
        fs::write(root.join("funciones.typ"), bundle.functions.as_bytes())
            .map_err(|_| CoreError::Overloaded)?;
        for (index, item) in snapshot.order.iter().enumerate() {
            let bytes = if item.key == "oferta" {
                offer.as_bytes()
            } else {
                let number = item.key.strip_prefix("lote:").ok_or(CoreError::Invalid)?;
                snapshot
                    .sources
                    .files
                    .get(&format!("lote{number}-{style}.typ"))
                    .ok_or(CoreError::Invalid)?
                    .as_slice()
            };
            fs::write(root.join(format!("source-{:06}.typ", index + 1)), bytes)
                .map_err(|_| CoreError::Overloaded)?;
        }
        let count = u32::try_from(snapshot.order.len()).map_err(|_| CoreError::Invalid)?;
        let request = serde_json::json!({"pieces":count,"sources":(1..=count).collect::<Vec<_>>(),"economic_preview":true});
        crate::pdf_jobs::run_isolated_request(&root, &identifier, &request).await?;
        let path = root.join("result.pdf");
        let meta = fs::symlink_metadata(&path).map_err(|_| CoreError::Invalid)?;
        if !meta.file_type().is_file() || meta.len() == 0 || meta.len() > crate::MAX_INPUT_BYTES {
            return Err(CoreError::Invalid);
        }
        let mut bytes = Vec::new();
        fs::File::open(path)
            .map_err(|_| CoreError::Overloaded)?
            .take(crate::MAX_INPUT_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| CoreError::Overloaded)?;
        if bytes.len() as u64 != meta.len() || !bytes.starts_with(b"%PDF-") {
            return Err(CoreError::Invalid);
        }
        let mut tx = self.begin(actor, "editar").await?;
        if crate::service::load(&mut tx, id).await?.version != expected {
            return Err(CoreError::PreconditionFailed);
        }
        crate::service::event(&mut tx, actor, "pdf.economic_preview", id, expected).await?;
        tx.commit().await?;
        Ok(AttachmentContent {
            filename: format!("oferta-economica-{id}.pdf"),
            mime: "application/pdf".into(),
            content_base64: STANDARD.encode(bytes),
        })
    }
}
