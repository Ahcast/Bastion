use super::*;

impl QuoteService {
    /// Locks the source first. Destination owners join this same transaction.
    pub async fn conversion_source(
        &self,
        actor: u64,
        id: u64,
        tx: &mut Transaction<'_, MySql>,
    ) -> Result<QuoteDraft, CoreError> {
        self.admin
            .authorize(actor, "cotizaciones", "documentos", "ver")
            .await?;
        self.admin
            .authorize(actor, "cotizaciones", "documentos", "editar")
            .await?;
        let enabled:Option<bool>=sqlx::query_scalar("SELECT (included_in_release AND environment_allowed AND enabled) FROM module_registry WHERE module_id='cotizaciones' FOR UPDATE")
            .fetch_optional(&mut **tx).await?;
        if enabled != Some(true) {
            return Err(CoreError::NotFound);
        }
        load(tx, id).await
    }
    pub async fn finish_conversion(
        &self,
        actor: u64,
        id: u64,
        expected: u64,
        tx: &mut Transaction<'_, MySql>,
    ) -> Result<u64, CoreError> {
        let quote = self.conversion_source(actor, id, tx).await?;
        if quote.version != expected {
            return Err(CoreError::PreconditionFailed);
        }
        let next = expected.checked_add(1).ok_or(CoreError::Invalid)?;
        sqlx::query("UPDATE cotizaciones_drafts SET data_json=JSON_SET(data_json,'$.status','Convertido'),calculation_json=JSON_SET(calculation_json,'$.data.status','Convertido'),version=?,updated_by=?,updated_at=UTC_TIMESTAMP(6) WHERE id=? AND version=?")
            .bind(next).bind(actor).bind(id).bind(expected).execute(&mut **tx).await?;
        CoreAdmin::record_domain_event(tx,actor,"cotizaciones","quote.status",&id.to_string(),serde_json::json!({"version":next,"previous_status":quote.status,"status":"Convertido"})).await?;
        Ok(next)
    }
}
