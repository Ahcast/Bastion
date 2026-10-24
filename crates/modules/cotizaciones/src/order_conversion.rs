use super::*;

impl QuoteService {
    pub async fn begin_order_conversion(
        &self,
        actor: u64,
    ) -> Result<Transaction<'_, MySql>, CoreError> {
        self.begin(actor, "editar").await
    }
    pub async fn order_conversion_source(
        &self,
        tx: &mut Transaction<'_, MySql>,
        id: u64,
        expected: u64,
    ) -> Result<QuoteDraft, CoreError> {
        let source = load(tx, id).await?;
        if source.version != expected {
            return Err(CoreError::PreconditionFailed);
        }
        if !matches!(source.data.status, QuoteStatus::Aprobada) {
            return Err(CoreError::Conflict);
        }
        let converted: Option<u64> = sqlx::query_scalar(
            "SELECT order_id FROM cotizaciones_order_conversions WHERE quote_id=?",
        )
        .bind(id)
        .fetch_optional(&mut **tx)
        .await?;
        if converted.is_some() {
            return Err(CoreError::Conflict);
        }
        Ok(source)
    }
    pub async fn finish_order_conversion(
        &self,
        tx: &mut Transaction<'_, MySql>,
        actor: u64,
        mut source: QuoteDraft,
        order_id: u64,
    ) -> Result<(), CoreError> {
        let source_version = source.version;
        source.version = source.version.checked_add(1).ok_or(CoreError::Invalid)?;
        source.data.status = QuoteStatus::Convertido;
        source.calculation.data.status = QuoteStatus::Convertido;
        sqlx::query("INSERT INTO cotizaciones_order_conversions (quote_id,source_version,order_id,created_by,created_at) VALUES (?,?,?,?,UTC_TIMESTAMP(6))").bind(source.id).bind(source_version).bind(order_id).bind(actor).execute(&mut **tx).await?;
        sqlx::query("UPDATE cotizaciones_drafts SET data_json=?,calculation_json=?,updated_by=?,updated_at=UTC_TIMESTAMP(6),version=? WHERE id=? AND version=?")
            .bind(serde_json::to_value(&source.data).map_err(|_| CoreError::Invalid)?)
            .bind(serde_json::to_value(&source.calculation).map_err(|_| CoreError::Invalid)?)
            .bind(actor).bind(source.version).bind(source.id).bind(source_version).execute(&mut **tx).await?;
        CoreAdmin::record_domain_event(tx, actor, "cotizaciones", "quote.to_order", &source.id.to_string(), serde_json::json!({"version":source.version,"source_version":source_version,"order_id":order_id})).await?;
        Ok(())
    }
}
