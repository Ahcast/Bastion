use super::*;
use crate::calculation::{cents, money};
use crate::{InvoicePayment, PaymentInput};
use chrono::Datelike;

fn command_hash(
    key: &str,
    action: &str,
    id: u64,
    expected: u64,
    body: &impl serde::Serialize,
) -> Result<(Vec<u8>, Vec<u8>), CoreError> {
    if !(16..=128).contains(&key.len())
        || !key
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
    {
        return Err(CoreError::Invalid);
    }
    Ok((
        Sha256::digest(key.as_bytes()).to_vec(),
        Sha256::digest(
            serde_json::to_vec(&(action, id, expected, body)).map_err(|_| CoreError::Invalid)?,
        )
        .to_vec(),
    ))
}
async fn replay(
    tx: &mut Transaction<'_, MySql>,
    actor: u64,
    key: &[u8],
    hash: &[u8],
) -> Result<Option<InvoiceDraft>, CoreError> {
    let previous: Option<(Vec<u8>,serde_json::Value)> = sqlx::query_as("SELECT request_hash,response_json FROM facturas_command_receipts WHERE actor_id=? AND key_hash=?").bind(actor).bind(key).fetch_optional(&mut **tx).await?;
    if let Some((saved, value)) = previous {
        if saved != hash {
            return Err(CoreError::Conflict);
        }
        let result: InvoiceDraft = serde_json::from_value(value).map_err(|_| CoreError::Invalid)?;
        load(tx, result.id).await?;
        return Ok(Some(result));
    }
    Ok(None)
}
async fn record(
    tx: &mut Transaction<'_, MySql>,
    actor: u64,
    key: Vec<u8>,
    hash: Vec<u8>,
    result: &InvoiceDraft,
    action: &str,
) -> Result<(), CoreError> {
    sqlx::query("INSERT INTO facturas_command_receipts(actor_id,key_hash,request_hash,response_json) VALUES (?,?,?,?)").bind(actor).bind(key).bind(hash).bind(serde_json::to_value(result).map_err(|_|CoreError::Invalid)?).execute(&mut **tx).await?;
    CoreAdmin::record_domain_event(
        tx,
        actor,
        "facturas",
        action,
        &result.id.to_string(),
        serde_json::json!({"version":result.version,"status":result.status}),
    )
    .await
}
pub(super) fn normalize_payment(mut payment: PaymentInput) -> Result<PaymentInput, CoreError> {
    if cents(&payment.amount)? < 1
        || !(1000..=9999).contains(&payment.date.year())
        || payment.method.chars().count() > 120
        || payment.reference.chars().count() > 180
        || payment
            .method
            .chars()
            .chain(payment.reference.chars())
            .any(char::is_control)
    {
        return Err(CoreError::Invalid);
    }
    payment.amount = money(cents(&payment.amount)?);
    payment.method = payment.method.trim().into();
    payment.reference = payment.reference.trim().into();
    Ok(payment)
}
pub(super) fn settle(calculation: &mut InvoiceCalculation, paid: i128) -> Result<(), CoreError> {
    let total = cents(&calculation.totals.total)?;
    if paid > total {
        return Err(CoreError::Invalid);
    }
    calculation.paid_total = money(paid);
    calculation.balance = money(total - paid);
    Ok(())
}
pub(super) async fn insert_payment(
    tx: &mut Transaction<'_, MySql>,
    actor: u64,
    id: u64,
    payment: &PaymentInput,
) -> Result<InvoicePayment, CoreError> {
    let payment_id=sqlx::query("INSERT INTO facturas_payments(invoice_id,payment_date,amount,method,reference,created_by,created_at) VALUES (?,?,?,?,?,?,UTC_TIMESTAMP(6))").bind(id).bind(payment.date).bind(&payment.amount).bind(&payment.method).bind(&payment.reference).bind(actor).execute(&mut **tx).await?.last_insert_id();
    Ok(InvoicePayment {
        id: payment_id,
        amount: payment.amount.clone(),
        date: payment.date,
        method: payment.method.clone(),
        reference: payment.reference.clone(),
    })
}
async fn persist(
    tx: &mut Transaction<'_, MySql>,
    actor: u64,
    invoice: &InvoiceDraft,
) -> Result<(), CoreError> {
    sqlx::query("UPDATE facturas_drafts SET version=?,status=?,invoice_number=?,sar_snapshot_json=?,company_snapshot_json=?,calculation_json=?,updated_by=?,updated_at=UTC_TIMESTAMP(6) WHERE id=?")
        .bind(invoice.version).bind(&invoice.status).bind(&invoice.invoice_number).bind(serde_json::to_value(&invoice.sar_snapshot).map_err(|_|CoreError::Invalid)?).bind(serde_json::to_value(&invoice.company_snapshot).map_err(|_|CoreError::Invalid)?).bind(serde_json::to_value(&invoice.calculation).map_err(|_|CoreError::Invalid)?).bind(actor).bind(invoice.id).execute(&mut **tx).await?;
    Ok(())
}

impl InvoiceService {
    pub async fn delete(
        &self,
        actor: u64,
        id: u64,
        expected: u64,
        key: &str,
    ) -> Result<(), CoreError> {
        let (key, hash) = command_hash(key, "delete", id, expected, &())?;
        let mut tx = self.begin(actor, "eliminar").await?;
        let previous: Option<Vec<u8>> = sqlx::query_scalar(
            "SELECT request_hash FROM facturas_command_receipts WHERE actor_id=? AND key_hash=?",
        )
        .bind(actor)
        .bind(&key)
        .fetch_optional(&mut *tx)
        .await?;
        if let Some(previous) = previous {
            if previous != hash {
                return Err(CoreError::Conflict);
            }
            tx.commit().await?;
            return Ok(());
        }
        let current = load(&mut tx, id).await?;
        if current.version != expected {
            return Err(CoreError::PreconditionFailed);
        }
        if current.status != "Borrador"
            || !current.payments.is_empty()
            || current.invoice_number.is_some()
        {
            return Err(CoreError::Conflict);
        }
        let version = expected.checked_add(1).ok_or(CoreError::Invalid)?;
        sqlx::query("UPDATE facturas_drafts SET deleted_at=UTC_TIMESTAMP(6),updated_at=UTC_TIMESTAMP(6),updated_by=?,version=? WHERE id=?").bind(actor).bind(version).bind(id).execute(&mut *tx).await?;
        sqlx::query("INSERT INTO facturas_document_deletions(object_key,available_at) SELECT object_key,UTC_TIMESTAMP(6) FROM facturas_documents WHERE invoice_id=? ON DUPLICATE KEY UPDATE available_at=VALUES(available_at)").bind(id).execute(&mut *tx).await?;
        sqlx::query("DELETE FROM facturas_documents WHERE invoice_id=?")
            .bind(id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("INSERT INTO facturas_command_receipts(actor_id,key_hash,request_hash,response_json) VALUES (?,?,?,?)").bind(actor).bind(key).bind(hash).bind(serde_json::json!({"id":id,"deleted":true})).execute(&mut *tx).await?;
        CoreAdmin::record_domain_event(
            &mut tx,
            actor,
            "facturas",
            "invoice.delete",
            &id.to_string(),
            serde_json::json!({"version":version,"source_relations_preserved":true}),
        )
        .await?;
        tx.commit().await?;
        Ok(())
    }
    pub async fn preview_saved(
        &self,
        actor: u64,
        id: u64,
        expected: u64,
        input: &InvoiceInput,
    ) -> Result<InvoiceCalculation, CoreError> {
        let mut tx = self.begin(actor, "editar").await?;
        let saved = load(&mut tx, id).await?;
        if saved.version != expected {
            return Err(CoreError::PreconditionFailed);
        }
        if saved.status != "Borrador" {
            return Err(CoreError::Conflict);
        }
        let mut result = calculate(input)?;
        let mut paid = cents(&saved.calculation.paid_total)?;
        if let Some(payment) = input.initial_payment.clone() {
            if !saved.payments.is_empty() {
                return Err(CoreError::Conflict);
            }
            paid += cents(&normalize_payment(payment)?.amount)?;
        }
        settle(&mut result, paid)?;
        tx.commit().await?;
        Ok(result)
    }
    pub async fn clone_invoice(
        &self,
        actor: u64,
        id: u64,
        expected: u64,
        key: &str,
    ) -> Result<InvoiceDraft, CoreError> {
        let (key, hash) = command_hash(key, "clone", id, expected, &())?;
        let mut tx = self.begin(actor, "editar").await?;
        if let Some(result) = replay(&mut tx, actor, &key, &hash).await? {
            tx.commit().await?;
            return Ok(result);
        }
        let original = load(&mut tx, id).await?;
        if original.version != expected {
            return Err(CoreError::PreconditionFailed);
        }
        let mut input = original.data;
        input.initial_payment = None;
        input.invoice_date = chrono::Utc::now()
            .with_timezone(&chrono::FixedOffset::west_opt(21600).ok_or(CoreError::Invalid)?)
            .date_naive();
        let calculation = calculate(&input)?;
        // Legacy clone clears payments and keeps the term. A cash clone must
        // receive its full payment before it can be saved/issued.
        let new_id=sqlx::query("INSERT INTO facturas_drafts(client_name,order_number,invoice_date,data_json,calculation_json,created_by,updated_by,created_at,updated_at) VALUES (?,?,?,?,?,?,?,UTC_TIMESTAMP(6),UTC_TIMESTAMP(6))").bind(&calculation.data.client_name).bind(&calculation.data.order_number).bind(calculation.data.invoice_date).bind(serde_json::to_value(&calculation.data).map_err(|_|CoreError::Invalid)?).bind(serde_json::to_value(&calculation).map_err(|_|CoreError::Invalid)?).bind(actor).bind(actor).execute(&mut *tx).await?.last_insert_id();
        let result = load(&mut tx, new_id).await?;
        record(&mut tx, actor, key, hash, &result, "invoice.clone").await?;
        tx.commit().await?;
        Ok(result)
    }

    pub async fn mark_sent(
        &self,
        actor: u64,
        id: u64,
        expected: u64,
        key: &str,
    ) -> Result<InvoiceDraft, CoreError> {
        let (key, hash) = command_hash(key, "send", id, expected, &())?;
        let mut tx = self.begin(actor, "editar").await?;
        if let Some(result) = replay(&mut tx, actor, &key, &hash).await? {
            tx.commit().await?;
            return Ok(result);
        }
        let mut result = load(&mut tx, id).await?;
        if result.version != expected {
            return Err(CoreError::PreconditionFailed);
        }
        if result.status != "Borrador" {
            return Err(CoreError::Conflict);
        }
        if result.data.client_name.trim().is_empty() || result.data.items.is_empty() {
            return Err(CoreError::Invalid);
        }
        result.calculation.ensure_saveable()?;
        let (number, sar, company) = self
            .admin
            .allocate_invoice_number(&mut tx, actor, result.data.invoice_date)
            .await?;
        result.invoice_number = Some(number);
        result.sar_snapshot = Some(sar);
        result.company_snapshot = Some(company);
        result.status = if result.calculation.balance == "0.00" {
            "Pagado"
        } else {
            "Enviado"
        }
        .into();
        result.version = expected.checked_add(1).ok_or(CoreError::Invalid)?;
        persist(&mut tx, actor, &result).await?;
        record(&mut tx, actor, key, hash, &result, "invoice.sent").await?;
        tx.commit().await?;
        Ok(result)
    }

    pub async fn add_payment(
        &self,
        actor: u64,
        id: u64,
        expected: u64,
        key: &str,
        payment: PaymentInput,
    ) -> Result<InvoiceDraft, CoreError> {
        let payment = normalize_payment(payment)?;
        let (key, hash) = command_hash(key, "payment", id, expected, &payment)?;
        let mut tx = self.begin(actor, "editar").await?;
        if let Some(result) = replay(&mut tx, actor, &key, &hash).await? {
            tx.commit().await?;
            return Ok(result);
        }
        let mut result = load(&mut tx, id).await?;
        if result.version != expected {
            return Err(CoreError::PreconditionFailed);
        }
        if result.status != "Enviado" {
            return Err(CoreError::Conflict);
        }
        if result.payments.len() >= 200 {
            return Err(CoreError::Invalid);
        }
        let paid = cents(&result.calculation.paid_total)?
            .checked_add(cents(&payment.amount)?)
            .ok_or(CoreError::Invalid)?;
        settle(&mut result.calculation, paid)?;
        result
            .payments
            .push(insert_payment(&mut tx, actor, id, &payment).await?);
        if result.calculation.balance == "0.00" {
            result.status = "Pagado".into();
        }
        result.version = expected.checked_add(1).ok_or(CoreError::Invalid)?;
        persist(&mut tx, actor, &result).await?;
        record(&mut tx, actor, key, hash, &result, "invoice.payment").await?;
        tx.commit().await?;
        Ok(result)
    }
}
