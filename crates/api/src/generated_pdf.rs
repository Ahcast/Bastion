use super::*;
use base64::{Engine, engine::general_purpose::STANDARD};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ViewQuery {
    version: u64,
}

impl ViewQuery {
    pub(super) fn expected(self, headers: &axum::http::HeaderMap) -> Result<u64, ApiError> {
        if self.version == 0 {
            return Err(ApiError(CoreError::Invalid));
        }
        // Native navigation carries the session cookie. Reject a cross-site page
        // trying to trigger document rendering with the user's session.
        if headers
            .get("sec-fetch-site")
            .is_some_and(|value| value == "cross-site")
        {
            return Err(ApiError(CoreError::Forbidden));
        }
        Ok(self.version)
    }
}

pub(super) fn inline(
    filename: &str,
    mime: &str,
    content_base64: &str,
) -> Result<Response, ApiError> {
    if mime != "application/pdf" {
        return Err(ApiError(CoreError::Invalid));
    }
    let bytes = STANDARD
        .decode(content_base64)
        .map_err(|_| ApiError(CoreError::Invalid))?;
    if !bytes.starts_with(b"%PDF-") {
        return Err(ApiError(CoreError::Invalid));
    }
    let filename = bastion_core::pdf_filename(
        filename.strip_suffix(".pdf").unwrap_or(filename),
        "documento",
    );
    let ascii: String = filename
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-' | '_' | ' ') {
                ch
            } else {
                '_'
            }
        })
        .collect();
    let encoded = url::form_urlencoded::byte_serialize(filename.as_bytes())
        .collect::<String>()
        .replace('+', "%20");
    let disposition = HeaderValue::from_str(&format!(
        "inline; filename=\"{ascii}\"; filename*=UTF-8''{encoded}"
    ))
    .map_err(|_| ApiError(CoreError::Invalid))?;
    let mut response = Response::new(axum::body::Body::from(bytes));
    let headers = response.headers_mut();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/pdf"));
    headers.insert(axum::http::header::CONTENT_DISPOSITION, disposition);
    headers.insert(CACHE_CONTROL, HeaderValue::from_static("private, no-store"));
    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn native_pdf_has_a_safe_business_filename_and_unmodified_bytes() {
        let bytes = b"%PDF-1.7\nsynthetic";
        let response = inline(
            "Licitación 24_2026.pdf",
            "application/pdf",
            &STANDARD.encode(bytes),
        )
        .ok()
        .expect("valid PDF");
        assert_eq!(response.headers()[CONTENT_TYPE], "application/pdf");
        assert_eq!(response.headers()[CACHE_CONTROL], "private, no-store");
        let disposition = response.headers()[axum::http::header::CONTENT_DISPOSITION]
            .to_str()
            .unwrap();
        assert!(disposition.starts_with("inline; filename=\"Licitaci_n 24_2026.pdf\""));
        assert!(disposition.contains("filename*=UTF-8''Licitaci%C3%B3n%2024_2026.pdf"));
        assert_eq!(
            axum::body::to_bytes(response.into_body(), 1024)
                .await
                .unwrap()
                .as_ref(),
            bytes
        );
        assert!(inline("x.pdf", "text/html", &STANDARD.encode(bytes)).is_err());
        assert!(inline("x.pdf", "application/pdf", &STANDARD.encode(b"<html>")).is_err());
    }

    #[test]
    fn native_preview_keeps_version_binding_and_rejects_cross_site_navigation() {
        let mut headers = axum::http::HeaderMap::new();
        assert!(ViewQuery { version: 0 }.expected(&headers).is_err());
        headers.insert("sec-fetch-site", HeaderValue::from_static("same-site"));
        assert_eq!(ViewQuery { version: 7 }.expected(&headers).ok(), Some(7));
        headers.insert("sec-fetch-site", HeaderValue::from_static("cross-site"));
        assert!(matches!(
            ViewQuery { version: 7 }.expected(&headers),
            Err(ApiError(CoreError::Forbidden))
        ));
    }
}
