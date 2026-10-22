use super::*;
use lettre::{
    message::{Attachment, MultiPart},
    transport::smtp::client::{Tls, TlsParameters},
};

pub struct BusinessMailAttachment {
    pub filename: String,
    pub mime: String,
    pub bytes: Vec<u8>,
}
pub struct BusinessMail {
    pub to: Vec<String>,
    pub cc: Vec<String>,
    pub bcc: Vec<String>,
    pub subject: String,
    pub message: String,
    pub attachments: Vec<BusinessMailAttachment>,
}
impl BusinessMail {
    fn build(self, sender: Mailbox) -> Result<Message, CoreError> {
        let addresses = self
            .to
            .iter()
            .chain(&self.cc)
            .chain(&self.bcc)
            .collect::<Vec<_>>();
        if addresses.is_empty()
            || addresses.len() > 50
            || self.subject.chars().count() > 255
            || self.subject.contains(['\r', '\n'])
            || self.message.chars().count() > 5000
            || self.attachments.len() > 11
            || self
                .attachments
                .iter()
                .map(|file| file.bytes.len())
                .sum::<usize>()
                > 8 * 1024 * 1024
        {
            return Err(CoreError::Invalid);
        }
        let mut builder = Message::builder().from(sender).subject(self.subject);
        for address in self.to {
            builder = builder.to(address.parse().map_err(|_| CoreError::Invalid)?);
        }
        for address in self.cc {
            builder = builder.cc(address.parse().map_err(|_| CoreError::Invalid)?);
        }
        for address in self.bcc {
            builder = builder.bcc(address.parse().map_err(|_| CoreError::Invalid)?);
        }
        let html = self
            .message
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
            .replace('\n', "<br>");
        let mut body=MultiPart::mixed().multipart(MultiPart::alternative().singlepart(SinglePart::plain(self.message)).singlepart(SinglePart::html(format!("<div style=\"font-family:Arial,sans-serif;color:#1f2933;line-height:1.55\">{html}</div>"))));
        for file in self.attachments {
            if file.filename.is_empty()
                || file.filename.len() > 180
                || file.filename.contains(['\r', '\n', '/', '\\'])
            {
                return Err(CoreError::Invalid);
            }
            body = body.singlepart(Attachment::new(file.filename).body(
                file.bytes,
                file.mime.parse().map_err(|_| CoreError::Invalid)?,
            ));
        }
        builder.multipart(body).map_err(|_| CoreError::Invalid)
    }
}
impl CoreService {
    pub fn seal_domain_json(&self, value: &serde_json::Value) -> Result<Vec<u8>, CoreError> {
        self.encrypt(&serde_json::to_string(value).map_err(|_| CoreError::Invalid)?)
    }
    pub fn unseal_domain_json(&self, value: &[u8]) -> Result<serde_json::Value, CoreError> {
        serde_json::from_str(&self.decrypt(value)?).map_err(|_| CoreError::Invalid)
    }
    pub async fn business_mail_configured(&self, actor: u64) -> Result<bool, CoreError> {
        let credentials:i64=sqlx::query_scalar("SELECT COUNT(*) FROM user_smtp_credentials c JOIN users u ON u.id=c.user_id WHERE c.user_id=? AND u.status='active'").bind(actor).fetch_one(self.database.pool()).await?;
        let settings:i64=sqlx::query_scalar("SELECT COUNT(*) FROM settings WHERE module_id='core' AND scope_key='global' AND setting_key IN ('smtp.host','smtp.port','smtp.tls_mode')").fetch_one(self.database.pool()).await?;
        Ok(credentials == 1 && settings == 3)
    }
    /// Domain modules authorize the operation and actor before calling this shared SMTP port.
    pub async fn send_business_mail(
        &self,
        actor: u64,
        mail: BusinessMail,
    ) -> Result<(), CoreError> {
        let (email,encrypted):(String,Vec<u8>)=sqlx::query_as("SELECT c.smtp_email,c.password_ciphertext FROM user_smtp_credentials c JOIN users u ON u.id=c.user_id WHERE c.user_id=? AND u.status='active'").bind(actor).fetch_optional(self.database.pool()).await?.ok_or(CoreError::Invalid)?;
        let settings:Vec<(String,serde_json::Value)>=sqlx::query_as("SELECT setting_key,value_json FROM settings WHERE module_id='core' AND scope_key='global' AND setting_key IN ('smtp.host','smtp.port','smtp.tls_mode')").fetch_all(self.database.pool()).await?;
        let settings: std::collections::BTreeMap<_, _> = settings.into_iter().collect();
        let host = settings
            .get("smtp.host")
            .and_then(|v| v.as_str())
            .ok_or(CoreError::Invalid)?;
        let port = settings
            .get("smtp.port")
            .and_then(|v| v.as_u64())
            .and_then(|n| u16::try_from(n).ok())
            .ok_or(CoreError::Invalid)?;
        let mode = settings
            .get("smtp.tls_mode")
            .and_then(|v| v.as_str())
            .ok_or(CoreError::Invalid)?;
        if !matches!(
            (port, mode),
            (465, "implicit_tls") | (587, "starttls_required")
        ) || !smtp_host_resolves_publicly(host, port).await
        {
            return Err(CoreError::Invalid);
        }
        // Pin the checked address while keeping certificate/SNI validation on the configured hostname.
        let addresses = tokio::time::timeout(
            Duration::from_secs(5),
            tokio::net::lookup_host((host, port)),
        )
        .await
        .map_err(|_| CoreError::Overloaded)?
        .map_err(|_| CoreError::Overloaded)?
        .collect::<Vec<_>>();
        if addresses.is_empty() || addresses.iter().any(|address| !is_public_ip(address.ip())) {
            return Err(CoreError::Invalid);
        }
        let tls = TlsParameters::new(host.to_owned()).map_err(|_| CoreError::Invalid)?;
        let tls = if mode == "implicit_tls" {
            Tls::Wrapper(tls)
        } else {
            Tls::Required(tls)
        };
        let message = mail.build(email.parse().map_err(|_| CoreError::Invalid)?)?;
        let transport =
            AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(addresses[0].ip().to_string())
                .port(port)
                .tls(tls)
                .credentials(Credentials::new(email, self.decrypt(&encrypted)?))
                .timeout(Some(Duration::from_secs(15)))
                .build();
        transport
            .send(message)
            .await
            .map_err(|_| CoreError::Overloaded)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn copies_reach_the_envelope_without_disclosing_bcc_and_message_text_is_escaped() {
        let mail = BusinessMail {
            to: vec!["to@example.test".into()],
            cc: vec!["cc@example.test".into()],
            bcc: vec!["private@example.test".into()],
            subject: "Cotización".into(),
            message: "<script>mensaje</script>".into(),
            attachments: vec![BusinessMailAttachment {
                filename: "cotizacion.pdf".into(),
                mime: "application/pdf".into(),
                bytes: b"%PDF-synthetic".to_vec(),
            }],
        };
        let built = mail.build("sender@example.test".parse().unwrap()).unwrap();
        assert_eq!(built.envelope().to().len(), 3);
        let wire = String::from_utf8(built.formatted()).unwrap();
        assert!(!wire.contains("Bcc:"));
        assert!(!wire.contains("private@example.test"));
        assert!(wire.contains("application/pdf"));
        assert!(wire.contains("&lt;script&gt;"));
    }
}
