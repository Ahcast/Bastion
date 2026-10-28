use super::*;
use base64::{Engine, engine::general_purpose::STANDARD};
use bastion_core::DocumentService;

const TEMPLATE: &str = include_str!("../templates/factura-v1.typ");

fn under_thousand(n: u64) -> String {
    const SMALL: [&str; 30] = [
        "cero",
        "un",
        "dos",
        "tres",
        "cuatro",
        "cinco",
        "seis",
        "siete",
        "ocho",
        "nueve",
        "diez",
        "once",
        "doce",
        "trece",
        "catorce",
        "quince",
        "dieciséis",
        "diecisiete",
        "dieciocho",
        "diecinueve",
        "veinte",
        "veintiún",
        "veintidós",
        "veintitrés",
        "veinticuatro",
        "veinticinco",
        "veintiséis",
        "veintisiete",
        "veintiocho",
        "veintinueve",
    ];
    const TENS: [&str; 10] = [
        "",
        "",
        "",
        "treinta",
        "cuarenta",
        "cincuenta",
        "sesenta",
        "setenta",
        "ochenta",
        "noventa",
    ];
    const HUNDREDS: [&str; 10] = [
        "",
        "ciento",
        "doscientos",
        "trescientos",
        "cuatrocientos",
        "quinientos",
        "seiscientos",
        "setecientos",
        "ochocientos",
        "novecientos",
    ];
    if n < 30 {
        return SMALL[n as usize].into();
    }
    if n < 100 {
        return if n % 10 == 0 {
            TENS[(n / 10) as usize].into()
        } else {
            format!("{} y {}", TENS[(n / 10) as usize], SMALL[(n % 10) as usize])
        };
    }
    if n == 100 {
        return "cien".into();
    }
    if n % 100 == 0 {
        return HUNDREDS[(n / 100) as usize].into();
    }
    format!(
        "{} {}",
        HUNDREDS[(n / 100) as usize],
        under_thousand(n % 100)
    )
}

fn under_million(n: u64) -> String {
    if n < 1000 {
        return under_thousand(n);
    }
    let thousands = n / 1000;
    let head = if thousands == 1 {
        "mil".into()
    } else {
        format!("{} mil", under_thousand(thousands))
    };
    if n % 1000 == 0 {
        head
    } else {
        format!("{head} {}", under_thousand(n % 1000))
    }
}

fn amount_in_words(value: &str) -> Result<String, CoreError> {
    let (whole, fraction) = value.split_once('.').ok_or(CoreError::Invalid)?;
    if fraction.len() != 2 || !fraction.bytes().all(|b| b.is_ascii_digit()) {
        return Err(CoreError::Invalid);
    }
    let n: u64 = whole.parse().map_err(|_| CoreError::Invalid)?;
    if n > 999_999_999_999 {
        return Err(CoreError::Invalid);
    }
    let words = if n < 1_000_000 {
        under_million(n)
    } else {
        let millions = n / 1_000_000;
        let head = if millions == 1 {
            "un millón".into()
        } else {
            format!("{} millones", under_million(millions))
        };
        if n % 1_000_000 == 0 {
            head
        } else {
            format!("{head} {}", under_million(n % 1_000_000))
        }
    };
    let unit = if n == 1 { "lempira" } else { "lempiras" };
    let connector = if n >= 1_000_000 && n % 1_000_000 == 0 {
        " de"
    } else {
        ""
    };
    Ok(format!("{words}{connector} {unit} con {fraction}/100"))
}

#[cfg(test)]
mod words_tests {
    use super::amount_in_words;
    #[test]
    fn amounts_are_written_in_spanish_with_exact_cents() {
        for (amount, expected) in [
            ("0.00", "cero lempiras con 00/100"),
            ("1.01", "un lempira con 01/100"),
            ("21.50", "veintiún lempiras con 50/100"),
            ("100.00", "cien lempiras con 00/100"),
            ("115.23", "ciento quince lempiras con 23/100"),
            ("1000.00", "mil lempiras con 00/100"),
            ("1000000.00", "un millón de lempiras con 00/100"),
            (
                "21001001.99",
                "veintiún millones mil un lempiras con 99/100",
            ),
        ] {
            assert_eq!(amount_in_words(amount).unwrap(), expected);
        }
    }
}

#[derive(serde::Serialize)]
pub struct InvoiceDocument {
    pub filename: String,
    pub mime: String,
    pub content_base64: String,
}

impl InvoiceService {
    pub async fn document_readiness(&self, actor: u64) -> Result<serde_json::Value, CoreError> {
        self.check_access(actor, "ver").await?;
        Ok(
            serde_json::json!({"pdf":DocumentService::from_environment().configured() && self.admin.document_company().await.is_ok()}),
        )
    }
    pub async fn document(
        &self,
        actor: u64,
        id: u64,
        expected: u64,
        kind: &str,
    ) -> Result<InvoiceDocument, CoreError> {
        if !matches!(kind, "invoice" | "delivery" | "receipt") {
            return Err(CoreError::Invalid);
        }
        let current = self.detail(actor, id).await?;
        if current.version != expected {
            return Err(CoreError::PreconditionFailed);
        }
        if kind == "receipt" && (current.status == "Borrador" || current.invoice_number.is_none()) {
            return Err(CoreError::Conflict);
        }
        let storage = DocumentService::from_environment();
        if !storage.configured() {
            return Err(CoreError::Overloaded);
        }
        let company = if let Some(company) = &current.company_snapshot {
            company.clone()
        } else {
            self.admin.document_company().await?
        };
        let items=current.data.items.iter().zip(&current.calculation.items).map(|(row,c)|serde_json::json!({"description":row.description,"quantity":row.quantity,"unit_price":row.unit_price,"discount":c.discount_amount,"total":c.total})).collect::<Vec<_>>();
        let updated:Option<chrono::NaiveDateTime>=sqlx::query_scalar("SELECT updated_at FROM facturas_drafts WHERE id=? AND version=? AND deleted_at IS NULL").bind(id).bind(expected).fetch_optional(self.database.pool()).await?;
        let total_words = if kind == "receipt" {
            amount_in_words(&current.calculation.totals.total)?
        } else {
            String::new()
        };
        let payload = serde_json::json!({"kind":kind,"number":current.invoice_number.clone().unwrap_or_else(||format!("{} {id}", if kind == "invoice" { "Proforma" } else { "Borrador" })),"issued":current.invoice_number.is_some(),"status":current.status,"date":current.data.invoice_date.to_string(),"payment_term_type":current.data.payment_term_type,"credit_days":current.data.credit_days,"discount_type":current.data.discount_type,"discount_value":current.data.discount_value,"purchase_order_exempt_number":current.data.purchase_order_exempt_number,"registration_certificate_number":current.data.registration_certificate_number,"sag_registration_number":current.data.sag_registration_number,"company":company,"client_name":current.data.client_name,"client_rtn":current.data.client_rtn,"order_number":current.data.order_number,"public_notes":current.data.public_notes,"totals":current.calculation.totals,"total_words":total_words,"paid_total":current.calculation.paid_total,"balance":current.calculation.balance,"items":items,"payments":current.payments,"sar":current.sar_snapshot,"logo":storage.branding().0,"creation_timestamp":updated.ok_or(CoreError::PreconditionFailed)?.and_utc().timestamp(),"branding_fingerprint":storage.branding_fingerprint()?});
        let mut identity = serde_json::to_vec(&payload).map_err(|_| CoreError::Invalid)?;
        identity.extend_from_slice(TEMPLATE.as_bytes());
        let prefix = format!(
            "facturas/{id}/{expected}/{kind}-v1-{}-",
            hex::encode(Sha256::digest(identity))
        );
        let previous:Option<(String,Vec<u8>)>=sqlx::query_as("SELECT object_key,sha256 FROM facturas_documents WHERE invoice_id=? AND invoice_version=? AND kind=? AND object_key LIKE ? ORDER BY object_key LIMIT 1").bind(id).bind(expected).bind(kind).bind(format!("{prefix}%")).fetch_optional(self.database.pool()).await?;
        let key = previous
            .as_ref()
            .map(|(k, _)| k.clone())
            .unwrap_or_else(|| format!("{prefix}{}.pdf", DocumentService::object_suffix()));
        let filename = format!(
            "{kind}-{}.pdf",
            current
                .invoice_number
                .as_deref()
                .map(str::to_owned)
                .unwrap_or_else(|| format!("borrador-{id}"))
        );
        let bytes = if let Some((_, hash)) = previous {
            let bytes = storage.get(key.clone()).await?;
            if Sha256::digest(&bytes).as_slice() != hash.as_slice() {
                return Err(CoreError::Overloaded);
            }
            bytes
        } else {
            let bytes = storage.render(TEMPLATE, payload).await?;
            storage.put(key.clone(), bytes.clone()).await?;
            let persist=async {
                let mut tx=self.begin(actor,"ver").await?;let latest=load(&mut tx,id).await?;
                if latest.version!=expected {return Err(CoreError::PreconditionFailed);}
                sqlx::query("INSERT INTO facturas_documents(object_key,invoice_id,invoice_version,kind,filename,sha256,byte_size,created_at) VALUES (?,?,?,?,?,?,?,UTC_TIMESTAMP(6))").bind(&key).bind(id).bind(expected).bind(kind).bind(&filename).bind(Sha256::digest(&bytes).to_vec()).bind(bytes.len() as u32).execute(&mut *tx).await?;
                CoreAdmin::record_domain_event(&mut tx,actor,"facturas","invoice.document",&id.to_string(),serde_json::json!({"version":expected,"kind":kind,"template":"factura-v1"})).await?;tx.commit().await?;Ok::<(),CoreError>(())
            }.await;
            if let Err(error) = persist {
                sqlx::query("INSERT IGNORE INTO facturas_document_deletions(object_key,available_at) VALUES (?,UTC_TIMESTAMP(6)+INTERVAL 5 MINUTE)").bind(&key).execute(self.database.pool()).await?;
                return Err(error);
            }
            bytes
        };
        let latest = self.detail(actor, id).await?;
        if latest.version != expected {
            return Err(CoreError::PreconditionFailed);
        }
        Ok(InvoiceDocument {
            filename,
            mime: "application/pdf".into(),
            content_base64: STANDARD.encode(bytes),
        })
    }

    pub async fn clean_one_document(&self) -> Result<bool, CoreError> {
        let mut tx = self.database.pool().begin().await?;
        let key:Option<String>=sqlx::query_scalar("SELECT object_key FROM facturas_document_deletions WHERE available_at<=UTC_TIMESTAMP(6) ORDER BY available_at LIMIT 1 FOR UPDATE").fetch_optional(&mut *tx).await?;
        let Some(key) = key else {
            tx.commit().await?;
            return Ok(false);
        };
        sqlx::query("UPDATE facturas_document_deletions SET attempts=attempts+1,available_at=UTC_TIMESTAMP(6)+INTERVAL 5 MINUTE WHERE object_key=?").bind(&key).execute(&mut *tx).await?;
        tx.commit().await?;
        let live: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM facturas_documents WHERE object_key=?")
                .bind(&key)
                .fetch_one(self.database.pool())
                .await?;
        if live == 0 {
            DocumentService::from_environment()
                .remove(key.clone())
                .await?;
        }
        sqlx::query("DELETE FROM facturas_document_deletions WHERE object_key=?")
            .bind(key)
            .execute(self.database.pool())
            .await?;
        Ok(true)
    }
}
