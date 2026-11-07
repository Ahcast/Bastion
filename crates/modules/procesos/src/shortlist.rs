use std::{collections::BTreeSet, path::Path};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::{LocalDeliveryError, ReportAvailability, local::read_regular};

const MAX_BYTES: usize = 8 * 1024 * 1024;

#[derive(Serialize)]
pub struct TenderShortlist {
    pub availability: ReportAvailability,
    pub checked_at: DateTime<Utc>,
    pub generated_at: Option<DateTime<Utc>>,
    pub scan_status: Option<String>,
    pub items: Vec<ShortlistItem>,
}

#[derive(Deserialize)]
struct Feed {
    schema_version: u32,
    generated_at: DateTime<Utc>,
    scan_status: String,
    items: Vec<ShortlistItem>,
}

#[derive(Deserialize, Serialize)]
pub struct ShortlistItem {
    pub id: String,
    pub version: String,
    pub first_seen_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub expediente: String,
    pub entidad: String,
    pub unidad_compra: String,
    pub modalidad: String,
    pub tipo_adquisicion: String,
    pub etapa: String,
    pub objeto: String,
    pub tipo_fuente: String,
    pub fuente: String,
    pub fecha_recepcion: DateTime<Utc>,
    pub lugar_recepcion: String,
    pub coincidencias_interes: String,
    pub posibles_interes: String,
    pub evidencia_interes: String,
    pub url: String,
    pub extraido_en: DateTime<Utc>,
}

impl TenderShortlist {
    pub fn read(csv_path: &Path) -> Self {
        let now = Utc::now();
        let result = csv_path
            .parent()
            .ok_or(LocalDeliveryError::Invalid)
            .and_then(|parent| {
                read_regular(&parent.join("licitaciones/lista_corta.json"), MAX_BYTES)
            })
            .and_then(|bytes| Self::parse(&bytes, now));
        match result {
            Ok(feed) => feed,
            Err(error) => Self {
                availability: match error {
                    LocalDeliveryError::Missing => ReportAvailability::Missing,
                    LocalDeliveryError::Io => ReportAvailability::Unreadable,
                    _ => ReportAvailability::Invalid,
                },
                checked_at: now,
                generated_at: None,
                scan_status: None,
                items: Vec::new(),
            },
        }
    }

    fn parse(bytes: &[u8], now: DateTime<Utc>) -> Result<Self, LocalDeliveryError> {
        let mut feed: Feed =
            serde_json::from_slice(bytes).map_err(|_| LocalDeliveryError::Invalid)?;
        let mut identifiers = BTreeSet::new();
        if feed.schema_version != 2
            || feed.items.len() > 5000
            || !matches!(feed.scan_status.as_str(), "exitoso" | "parcial")
            || feed
                .items
                .iter()
                .any(|item| !item.valid() || !identifiers.insert(item.id.clone()))
        {
            return Err(LocalDeliveryError::Invalid);
        }
        feed.items.retain(|item| item.fecha_recepcion > now);
        feed.items.sort_by(|a, b| {
            a.priority().cmp(&b.priority()).then(
                a.fecha_recepcion
                    .cmp(&b.fecha_recepcion)
                    .then(a.id.cmp(&b.id)),
            )
        });
        Ok(Self {
            availability: ReportAvailability::Available,
            checked_at: now,
            generated_at: Some(feed.generated_at),
            scan_status: Some(feed.scan_status),
            items: feed.items,
        })
    }
}

impl ShortlistItem {
    pub fn has_interest(&self) -> bool {
        !self.coincidencias_interes.is_empty()
    }

    fn priority(&self) -> usize {
        ["JICA", "BID", "UCP"]
            .iter()
            .position(|tag| {
                self.coincidencias_interes
                    .split(" | ")
                    .any(|value| value == *tag)
            })
            .unwrap_or(if self.posibles_interes.is_empty() {
                4
            } else {
                3
            })
    }

    fn valid(&self) -> bool {
        let url_valid = Url::parse(&self.url).is_ok_and(|url| {
            matches!(url.scheme(), "http" | "https")
                && url.host_str() == Some("sicc.honducompras.gob.hn")
                && url.username().is_empty()
                && url.password().is_none()
                && url.port().is_none()
                && matches!(
                    url.path().to_lowercase().rsplit('/').next(),
                    Some("procesohistorico.aspx" | "proceso.aspx")
                )
                && url.query().is_some()
        });
        let labels_valid = self.coincidencias_interes.len() <= "BID | UCP | JICA".len()
            && (self.coincidencias_interes.is_empty()
                || self
                    .coincidencias_interes
                    .split(" | ")
                    .all(|tag| matches!(tag, "BID" | "UCP" | "JICA")))
            && matches!(self.posibles_interes.as_str(), "" | "JICA");
        [&self.id, &self.version]
            .iter()
            .all(|value| value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit()))
            && url_valid
            && self.url.len() <= 4096
            && labels_valid
            && !self.expediente.trim().is_empty()
            && !self.entidad.trim().is_empty()
            && !self.objeto.trim().is_empty()
            && ((!self.has_interest() && self.posibles_interes.is_empty())
                || !self.evidencia_interes.trim().is_empty())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    fn fixture() -> Value {
        json!({"schema_version":2,"generated_at":"2026-11-06T12:00:00-06:00","scan_status":"parcial",
            "items":[{"id":"a".repeat(64),"version":"b".repeat(64),
            "first_seen_at":"2026-11-06T12:00:00-06:00","updated_at":"2026-11-06T12:00:00-06:00",
            "expediente":"LPN-1","entidad":"ENEE","unidad_compra":"Unidad de Proyectos BID",
            "modalidad":"Licitación pública nacional","tipo_adquisicion":"Servicios","etapa":"Recepción de Ofertas",
            "objeto":"Rehabilitación de compuertas","tipo_fuente":"Recursos externos","fuente":"Japón",
            "fecha_recepcion":"2026-11-30T14:00:00-06:00","lugar_recepcion":"UCP-BID-JICA/ENEE",
            "coincidencias_interes":"BID | UCP | JICA","posibles_interes":"",
            "evidencia_interes":"JICA (lugar_recepcion) UCP-BID-JICA/ENEE",
            "url":"http://sicc.honducompras.gob.hn/HC/Procesos/ProcesoHistorico.aspx?id=1",
            "extraido_en":"2026-11-06T12:00:00-06:00"}]})
    }

    #[test]
    fn partial_feed_keeps_metadata_but_expired_rows_are_removed() {
        let before = "2026-11-06T18:00:00Z".parse().unwrap();
        let feed = TenderShortlist::parse(fixture().to_string().as_bytes(), before).unwrap();
        assert_eq!(feed.scan_status.as_deref(), Some("parcial"));
        assert_eq!(feed.items[0].unidad_compra, "Unidad de Proyectos BID");
        let after = "2026-12-01T18:00:00Z".parse().unwrap();
        assert!(
            TenderShortlist::parse(fixture().to_string().as_bytes(), after)
                .unwrap()
                .items
                .is_empty()
        );
    }

    #[test]
    fn includes_unmatched_tenders_and_orders_jica_bid_ucp_first() {
        let mut data = fixture();
        let mut items = Vec::new();
        for (index, (tags, possible)) in [
            ("", ""),
            ("UCP", ""),
            ("BID", ""),
            ("", "JICA"),
            ("JICA", ""),
        ]
        .into_iter()
        .enumerate()
        {
            let mut item = data["items"][0].clone();
            item["id"] = json!(format!("{index:064x}"));
            item["coincidencias_interes"] = json!(tags);
            item["posibles_interes"] = json!(possible);
            items.push(item);
        }
        data["items"] = json!(items);
        let before = "2026-11-06T18:00:00Z".parse().unwrap();
        let feed = TenderShortlist::parse(data.to_string().as_bytes(), before).unwrap();
        assert_eq!(
            feed.items
                .iter()
                .map(ShortlistItem::priority)
                .collect::<Vec<_>>(),
            [0, 1, 2, 3, 4]
        );
    }

    #[test]
    fn invalid_links_duplicates_and_unknown_schema_are_rejected() {
        for (key, value) in [
            ("url", "javascript:alert(1)"),
            ("url", "https://example.test/"),
            ("coincidencias_interes", "UNKNOWN"),
            ("fecha_recepcion", "not a date"),
        ] {
            let mut invalid = fixture();
            invalid["items"][0][key] = json!(value);
            assert!(TenderShortlist::parse(invalid.to_string().as_bytes(), Utc::now()).is_err());
        }
        let mut invalid = fixture();
        invalid["schema_version"] = json!(1);
        assert!(TenderShortlist::parse(invalid.to_string().as_bytes(), Utc::now()).is_err());
        let mut invalid = fixture();
        invalid["items"]
            .as_array_mut()
            .unwrap()
            .push(fixture()["items"][0].clone());
        assert!(TenderShortlist::parse(invalid.to_string().as_bytes(), Utc::now()).is_err());
    }
}
