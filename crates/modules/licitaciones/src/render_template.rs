use crate::{MAX_INPUT_BYTES, TenderService, service::load};
use bastion_core::CoreError;

/// An authorized Typst file captured for one version of an expediente.
/// Its source is data for an isolated worker, never executable in the API.
pub struct RenderTemplate {
    pub source_id: u64,
    pub sha256: String,
    pub source: String,
}

impl TenderService {
    pub async fn render_template(
        &self,
        actor: u64,
        id: u64,
        expected_version: u64,
        form_id: u64,
    ) -> Result<RenderTemplate, CoreError> {
        let mut tx = self.begin(actor, "ver").await?;
        let expediente = load(&mut tx, id).await?;
        if expediente.version != expected_version {
            return Err(CoreError::PreconditionFailed);
        }
        let source_id = expediente
            .forms
            .iter()
            .find(|f| f.id == form_id)
            .ok_or(CoreError::NotFound)?
            .source
            .definition
            .template_source_id
            .ok_or(CoreError::Invalid)?;
        tx.commit().await?;
        let file = self
            .library
            .file_snapshot(actor, source_id, MAX_INPUT_BYTES as usize)
            .await?;
        if !file
            .filename
            .rsplit_once('.')
            .is_some_and(|(_, ext)| ext.eq_ignore_ascii_case("typ"))
        {
            return Err(CoreError::Invalid);
        }
        let source = String::from_utf8(file.bytes).map_err(|_| CoreError::Invalid)?;
        let mut tx = self.begin(actor, "ver").await?;
        let expediente = load(&mut tx, id).await?;
        if expediente.version != expected_version {
            return Err(CoreError::PreconditionFailed);
        }
        if !expediente
            .forms
            .iter()
            .any(|f| f.id == form_id && f.source.definition.template_source_id == Some(source_id))
        {
            return Err(CoreError::NotFound);
        }
        self.library
            .validate_file_snapshot_in_transaction(&mut tx, actor, source_id, &file.sha256)
            .await?;
        tx.commit().await?;
        let sha256 = file.sha256.iter().map(|b| format!("{b:02x}")).collect();
        Ok(RenderTemplate {
            source_id,
            sha256,
            source,
        })
    }
}
