use super::*;
use base64::{Engine, engine::general_purpose::STANDARD};
use bastion_core::{DocumentCompany, DocumentService};
use serde::Serialize;

const TEMPLATE: &str = include_str!("../templates/cotizacion-v1.typ");
#[derive(Serialize)]
pub struct QuoteDocument {
    pub filename: String,
    pub mime: String,
    pub content_base64: String,
    #[serde(skip_serializing)]
    pub object_key: String,
}
#[derive(Serialize)]
pub struct QuoteDocumentReadiness {
    pub pdf: bool,
    pub attachments: bool,
    pub company: bool,
    pub email: bool,
}

impl QuoteService {
    pub async fn document_readiness(
        &self,
        actor: u64,
        core: &bastion_core::CoreService,
    ) -> Result<QuoteDocumentReadiness, CoreError> {
        self.check_access(actor, "ver").await?;
        let company = self.admin.document_company().await.is_ok();
        Ok(QuoteDocumentReadiness {
            pdf: company && DocumentService::from_environment().configured(),
            attachments: DocumentService::from_environment().configured(),
            company,
            email: core.business_mail_configured(actor).await?,
        })
    }
    pub async fn pdf(
        &self,
        actor: u64,
        id: u64,
        expected: u64,
    ) -> Result<QuoteDocument, CoreError> {
        let current = self.detail(actor, id).await?;
        if current.version != expected {
            return Err(CoreError::PreconditionFailed);
        }
        let storage = DocumentService::from_environment();
        if !storage.configured() {
            return Err(CoreError::Overloaded);
        }
        let company = self.admin.document_company().await?;
        let mut payload = pdf_payload(&current, company, storage.branding())?;
        let updated: Option<chrono::NaiveDateTime> = sqlx::query_scalar(
            "SELECT updated_at FROM cotizaciones_drafts WHERE id=? AND version=?",
        )
        .bind(id)
        .bind(expected)
        .fetch_optional(self.database.pool())
        .await?;
        payload["creation_timestamp"] = serde_json::json!(
            updated
                .ok_or(CoreError::PreconditionFailed)?
                .and_utc()
                .timestamp()
        );
        payload["branding_fingerprint"] = serde_json::json!(storage.branding_fingerprint()?);
        let mut identity = serde_json::to_vec(&payload).map_err(|_| CoreError::Invalid)?;
        identity.extend_from_slice(TEMPLATE.as_bytes());
        let hash = Sha256::digest(identity);
        let prefix = format!("cotizaciones/{id}/{expected}/pdf-v1-{}-", hex::encode(hash));
        // A renderer owns its key, so failed/concurrent renders cannot remove or
        // overwrite another renderer's confirmed document during cleanup.
        let mut key = format!("{prefix}{}.pdf", DocumentService::object_suffix());
        let filename = format!(
            "{}.pdf",
            current.quote_number.as_deref().unwrap_or("cotizacion")
        );
        let previous: Option<(String,Vec<u8>)> = sqlx::query_as(
            "SELECT object_key,sha256 FROM cotizaciones_documents WHERE quote_id=? AND quote_version=? AND kind='pdf' AND object_key LIKE ? ORDER BY object_key LIMIT 1",
        )
        .bind(id)
        .bind(expected)
        .bind(format!("{prefix}%"))
        .fetch_optional(self.database.pool())
        .await?;
        let bytes = if let Some((stored_key, hash)) = previous {
            key = stored_key;
            let bytes = storage.get(key.clone()).await?;
            if Sha256::digest(&bytes).as_slice() != hash.as_slice() {
                return Err(CoreError::Overloaded);
            }
            bytes
        } else {
            let bytes = storage.render(TEMPLATE, payload).await?;
            storage.put(key.clone(), bytes.clone()).await?;
            let persist=async {
                let mut tx=self.begin(actor,"ver").await?;
                let latest=load(&mut tx,id).await?;
                if latest.version!=expected {return Err(CoreError::PreconditionFailed);}
                sqlx::query("INSERT INTO cotizaciones_documents (object_key,quote_id,quote_version,kind,filename,mime,sha256,byte_size,created_at) VALUES (?,?,?,'pdf',?,'application/pdf',?,?,UTC_TIMESTAMP(6))")
                    .bind(&key).bind(id).bind(expected).bind(&filename).bind(Sha256::digest(&bytes).to_vec()).bind(bytes.len() as u32).execute(&mut *tx).await?;
                CoreAdmin::record_domain_event(&mut tx,actor,"cotizaciones","quote.pdf",&id.to_string(),serde_json::json!({"version":expected,"template":"cotizacion-v1"})).await?;
                tx.commit().await?;
                Ok::<(),CoreError>(())
            }.await;
            if let Err(error) = persist {
                sqlx::query("INSERT IGNORE INTO cotizaciones_document_deletions (object_key,available_at) VALUES (?,UTC_TIMESTAMP(6)+INTERVAL 5 MINUTE)").bind(&key).execute(self.database.pool()).await?;
                return Err(error);
            }
            bytes
        };
        // Recheck authorization after storage IO, including module deactivation.
        self.check_access(actor, "ver").await?;
        Ok(QuoteDocument {
            filename,
            mime: "application/pdf".into(),
            content_base64: STANDARD.encode(bytes),
            object_key: key,
        })
    }
    pub async fn clean_one_document(&self) -> Result<bool, CoreError> {
        let mut tx = self.database.pool().begin().await?;
        let key:Option<String>=sqlx::query_scalar("SELECT object_key FROM cotizaciones_document_deletions WHERE available_at<=UTC_TIMESTAMP(6) ORDER BY available_at LIMIT 1 FOR UPDATE").fetch_optional(&mut *tx).await?;
        let Some(key) = key else {
            tx.commit().await?;
            return Ok(false);
        };
        sqlx::query("UPDATE cotizaciones_document_deletions SET attempts=attempts+1,available_at=UTC_TIMESTAMP(6)+INTERVAL 5 MINUTE WHERE object_key=?").bind(&key).execute(&mut *tx).await?;
        tx.commit().await?;
        // A concurrent idempotent generator may have committed the same key.
        let live: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM cotizaciones_documents WHERE object_key=?")
                .bind(&key)
                .fetch_one(self.database.pool())
                .await?;
        if live == 0 {
            DocumentService::from_environment()
                .remove(key.clone())
                .await?;
        }
        sqlx::query("DELETE FROM cotizaciones_document_deletions WHERE object_key=?")
            .bind(key)
            .execute(self.database.pool())
            .await?;
        Ok(true)
    }
}

fn pdf_payload(
    quote: &QuoteDraft,
    company: DocumentCompany,
    branding: (bool, bool),
) -> Result<serde_json::Value, CoreError> {
    let items=quote.data.items.iter().zip(&quote.calculation.items).map(|(item,calculated)|serde_json::json!({"description":item.description,"unit_price":item.unit_price,"quantity":item.quantity,"discount_amount":calculated.discount_amount,"tax_label":match calculated.tax_type {crate::TaxType::Exempt=>"Exento",crate::TaxType::Isv15=>"ISV 15%",crate::TaxType::Isv18=>"ISV 18%"},"total":calculated.total})).collect::<Vec<_>>();
    Ok(
        serde_json::json!({"number":quote.quote_number.as_deref().unwrap_or("Sin numerar"),"company":company,"contact_email":quote.data.contact_email,"client_name":quote.data.client_name,"client_rtn":quote.data.client_rtn,"order_number":quote.data.order_number,"date":quote.data.quote_date.format("%d/%m/%Y").to_string(),"valid_until":quote.calculation.valid_until.parse::<chrono::NaiveDate>().map_err(|_|CoreError::Invalid)?.format("%d/%m/%Y").to_string(),"totals":quote.calculation.totals,"items":items,"terms":quote.data.terms,"prepared_by_name":quote.data.prepared_by_name,"amount_words":amount_words(&quote.calculation.totals.total)?,"logo":branding.0,"signature":branding.1}),
    )
}
fn amount_words(value: &str) -> Result<String, CoreError> {
    let (whole, cents) = value.split_once('.').ok_or(CoreError::Invalid)?;
    let whole = whole.parse::<u128>().map_err(|_| CoreError::Invalid)?;
    if cents.len() != 2 || !cents.bytes().all(|c| c.is_ascii_digit()) {
        return Err(CoreError::Invalid);
    }
    Ok(format!(
        "{} {} CON {cents}/100",
        apocope(integer_words(whole)),
        if whole == 1 { "LEMPIRA" } else { "LEMPIRAS" }
    ))
}
fn apocope(value: String) -> String {
    value
        .strip_suffix("VEINTIUNO")
        .map(|s| format!("{s}VEINTIÚN"))
        .or_else(|| value.strip_suffix("UNO").map(|s| format!("{s}UN")))
        .unwrap_or(value)
}
fn integer_words(value: u128) -> String {
    const SMALL: [&str; 30] = [
        "CERO",
        "UNO",
        "DOS",
        "TRES",
        "CUATRO",
        "CINCO",
        "SEIS",
        "SIETE",
        "OCHO",
        "NUEVE",
        "DIEZ",
        "ONCE",
        "DOCE",
        "TRECE",
        "CATORCE",
        "QUINCE",
        "DIECISÉIS",
        "DIECISIETE",
        "DIECIOCHO",
        "DIECINUEVE",
        "VEINTE",
        "VEINTIUNO",
        "VEINTIDÓS",
        "VEINTITRÉS",
        "VEINTICUATRO",
        "VEINTICINCO",
        "VEINTISÉIS",
        "VEINTISIETE",
        "VEINTIOCHO",
        "VEINTINUEVE",
    ];
    if value < 30 {
        return SMALL[value as usize].into();
    }
    if value < 100 {
        let tens = [
            "",
            "",
            "",
            "TREINTA",
            "CUARENTA",
            "CINCUENTA",
            "SESENTA",
            "SETENTA",
            "OCHENTA",
            "NOVENTA",
        ][value as usize / 10];
        return if value % 10 == 0 {
            tens.into()
        } else {
            format!("{tens} Y {}", integer_words(value % 10))
        };
    }
    if value == 100 {
        return "CIEN".into();
    }
    if value < 1000 {
        let hundreds = [
            "",
            "CIENTO",
            "DOSCIENTOS",
            "TRESCIENTOS",
            "CUATROCIENTOS",
            "QUINIENTOS",
            "SEISCIENTOS",
            "SETECIENTOS",
            "OCHOCIENTOS",
            "NOVECIENTOS",
        ][value as usize / 100];
        return if value % 100 == 0 {
            hundreds.into()
        } else {
            format!("{hundreds} {}", integer_words(value % 100))
        };
    }
    let (power, singular, plural) = [
        (10u128.pow(36), "SEXTILLÓN", "SEXTILLONES"),
        (10u128.pow(30), "QUINTILLÓN", "QUINTILLONES"),
        (10u128.pow(24), "CUATRILLÓN", "CUATRILLONES"),
        (10u128.pow(18), "TRILLÓN", "TRILLONES"),
        (10u128.pow(12), "BILLÓN", "BILLONES"),
        (1_000_000, "MILLÓN", "MILLONES"),
        (1_000, "MIL", "MIL"),
    ]
    .into_iter()
    .find(|(power, _, _)| value >= *power)
    .unwrap();
    let prefix = if value / power == 1 {
        if power == 1000 {
            "MIL".into()
        } else {
            format!("UN {singular}")
        }
    } else {
        format!("{} {plural}", apocope(integer_words(value / power)))
    };
    if value % power == 0 {
        prefix
    } else {
        format!("{prefix} {}", integer_words(value % power))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn payload() -> serde_json::Value {
        let input: QuoteInput = serde_json::from_value(serde_json::json!({"client_name":"Cliente sintético","order_number":"PROC-1","quote_date":"2026-10-22","valid_days":30,"contact_email":"contact@example.test","prepared_by_name":"Preparador sintético","notes":"PRIVATE-NOTE-MUST-NOT-LEAVE","terms":"Términos #read(\"/etc/passwd\")","discount_type":"amount","discount_value":"0","items":[{"description":"Equipo #include(\"/etc/passwd\")","quantity":"2","unit_price":"100","discount_type":"amount","discount_value":"0","tax_type":"isv15"}]})).unwrap();
        let mut input = input;
        input.id_client = Some(41);
        input.client_rtn = "08011900000000".into();
        let calculation = calculate(&input).unwrap();
        let quote = QuoteDraft {
            id: 1,
            version: 1,
            status: "Borrador".into(),
            currency: "HNL".into(),
            data: calculation.data.clone(),
            quote_number: Some("COT-000001".into()),
            calculation,
        };
        pdf_payload(
            &quote,
            DocumentCompany {
                name: "Empresa sintética".into(),
                ..Default::default()
            },
            (false, false),
        )
        .unwrap()
    }
    #[test]
    fn pdf_uses_persisted_decimal_totals_and_excludes_private_notes() {
        let value = payload();
        assert!(!value.to_string().contains("PRIVATE-NOTE"));
        assert_eq!(value["totals"]["total"], "230.00");
        assert_eq!(value["items"][0]["total"], "200.00");
        assert_eq!(value["valid_until"], "21/11/2026");
        assert_eq!(value["client_rtn"], "08011900000000");
    }
    #[tokio::test]
    async fn real_typst_template_renders_untrusted_text_as_data() {
        if std::env::var_os("BASTION_TYPST_BIN").is_none() {
            assert!(
                std::env::var_os("CI").is_none(),
                "CI must install the pinned Typst compiler"
            );
            return;
        }
        let bytes = DocumentService::from_environment()
            .render(TEMPLATE, payload())
            .await
            .unwrap();
        assert!(bytes.starts_with(b"%PDF-"));
        assert!(bytes.len() > 1000);
    }
    #[test]
    fn words_use_decimal_strings_without_floating_point_rounding() {
        assert_eq!(
            amount_words("230.01").unwrap(),
            "DOSCIENTOS TREINTA LEMPIRAS CON 01/100"
        );
        assert_eq!(amount_words("1.00").unwrap(), "UN LEMPIRA CON 00/100");
        assert_eq!(integer_words(1_000_021), "UN MILLÓN VEINTIUNO");
        assert_eq!(integer_words(1_000_000_000_000), "UN BILLÓN");
        assert!(amount_words("1.001").is_err());
    }
}
