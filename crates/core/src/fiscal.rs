use super::*;
use crate::{DocumentCompany, SarSettings};

pub(super) fn fiscal_parts(number: &str) -> Result<(&str, u32), CoreError> {
    let bytes = number.as_bytes();
    if bytes.len() != 19
        || ![3, 7, 10].iter().all(|i| bytes[*i] == b'-')
        || !bytes
            .iter()
            .enumerate()
            .all(|(i, b)| [3, 7, 10].contains(&i) || b.is_ascii_digit())
    {
        return Err(CoreError::Invalid);
    }
    Ok((
        &number[..11],
        number[11..].parse().map_err(|_| CoreError::Invalid)?,
    ))
}

impl CoreAdmin {
    /// Called by the invoice domain on its own transaction. Core owns the SAR
    /// setting and the per-prefix high-water mark; no number is recycled.
    pub async fn allocate_invoice_number(
        &self,
        tx: &mut sqlx::Transaction<'_, MySql>,
        actor: u64,
        invoice_date: chrono::NaiveDate,
    ) -> Result<(String, SarSettings, DocumentCompany), CoreError> {
        self.authorize_in_transaction(tx, actor, "facturas", "documentos", "editar")
            .await?;
        sqlx::query("SELECT module_id FROM module_registry WHERE module_id='core' FOR UPDATE")
            .fetch_one(&mut **tx)
            .await?;
        let value: Option<serde_json::Value> = sqlx::query_scalar("SELECT value_json FROM settings WHERE module_id='core' AND scope_key='global' AND setting_key='company.sar' FOR UPDATE").fetch_optional(&mut **tx).await?;
        let mut settings: SarSettings = serde_json::from_value(value.ok_or(CoreError::Conflict)?)
            .map_err(|_| CoreError::Invalid)?;
        let today = chrono::Utc::now()
            .with_timezone(&chrono::FixedOffset::west_opt(21600).ok_or(CoreError::Invalid)?)
            .date_naive();
        let limit_date = settings.limit_date.ok_or(CoreError::Conflict)?;
        if limit_date < today || invoice_date > limit_date {
            return Err(CoreError::Conflict);
        }
        let (prefix, sequence) = fiscal_parts(&settings.current_invoice)?;
        let prefix = prefix.to_owned();
        let numbers: Vec<_> = settings
            .limit_range
            .split(|c: char| !c.is_ascii_digit() && c != '-')
            .filter(|part| !part.is_empty())
            .map(fiscal_parts)
            .collect::<Result<_, _>>()?;
        if numbers.is_empty() || numbers.len() > 2 || numbers.iter().any(|(p, _)| *p != prefix) {
            return Err(CoreError::Invalid);
        }
        let lower = if numbers.len() == 2 { numbers[0].1 } else { 1 };
        let upper = numbers.last().ok_or(CoreError::Invalid)?.1;
        let mut watermarks: serde_json::Value = sqlx::query_scalar("SELECT value_json FROM settings WHERE module_id='core' AND scope_key='global' AND setting_key='company.sar.sequence' FOR UPDATE").fetch_optional(&mut **tx).await?.unwrap_or_else(||serde_json::json!({}));
        let high = watermarks
            .get(&prefix)
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let next = u64::from(sequence)
            .max(high)
            .checked_add(1)
            .ok_or(CoreError::Invalid)?;
        if lower > upper || next < u64::from(lower) || next > u64::from(upper) || next > 99_999_999
        {
            return Err(CoreError::Conflict);
        }
        let company_value: Option<serde_json::Value> = sqlx::query_scalar("SELECT value_json FROM settings WHERE module_id='core' AND scope_key='global' AND setting_key='company.profile' FOR UPDATE").fetch_optional(&mut **tx).await?;
        let company: DocumentCompany =
            serde_json::from_value(company_value.ok_or(CoreError::Conflict)?)
                .map_err(|_| CoreError::Invalid)?;
        company.validate()?;
        let number = format!("{prefix}{next:08}");
        settings.current_invoice = number.clone();
        watermarks[&prefix] = serde_json::json!(next);
        for (key, value) in [
            (
                "company.sar",
                serde_json::to_value(&settings).map_err(|_| CoreError::Invalid)?,
            ),
            ("company.sar.sequence", watermarks),
        ] {
            sqlx::query("INSERT INTO settings(lookup_hash,module_id,scope_key,setting_key,schema_version,value_json,updated_by) VALUES (?,'core','global',?,1,?,?) ON DUPLICATE KEY UPDATE value_json=VALUES(value_json),updated_by=VALUES(updated_by)")
                .bind(hash_key(&format!("core\0global\0{key}"))).bind(key).bind(value).bind(actor).execute(&mut **tx).await?;
        }
        Self::record_domain_event(
            tx,
            actor,
            "core",
            "sar.number.allocate",
            &number,
            serde_json::json!({"schema_version":1}),
        )
        .await?;
        Ok((number, settings, company))
    }

    pub(super) async fn validate_sar_watermark(
        &self,
        tx: &mut sqlx::Transaction<'_, MySql>,
        settings: &SarSettings,
    ) -> Result<(), CoreError> {
        let marks: Option<serde_json::Value> = sqlx::query_scalar("SELECT value_json FROM settings WHERE module_id='core' AND scope_key='global' AND setting_key='company.sar.sequence' FOR UPDATE").fetch_optional(&mut **tx).await?;
        if let Some(marks) = marks {
            let (prefix, sequence) = fiscal_parts(&settings.current_invoice)?;
            if marks
                .get(prefix)
                .and_then(|n| n.as_u64())
                .is_some_and(|high| u64::from(sequence) < high)
            {
                return Err(CoreError::Conflict);
            }
        }
        Ok(())
    }
}
