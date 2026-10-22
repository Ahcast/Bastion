use std::collections::{BTreeMap, HashSet};

use bastion_core::CoreError;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub const MAX_CSV_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_ROWS: usize = 5_000;
pub const FIELDS: [&str; 24] = [
    "run_id",
    "proceso_id",
    "url",
    "entidad",
    "expediente",
    "fecha_recepcion",
    "fecha_recepcion_original",
    "descripcion",
    "items_fuente",
    "extraido_en",
    "extraccion_estado",
    "contenido_hash",
    "primera_deteccion",
    "ultima_deteccion",
    "ultima_modificacion",
    "puntaje",
    "rubro",
    "resumen_corto",
    "items",
    "hash_analizado",
    "analisis_estado",
    "analizado_en",
    "analisis_modelo",
    "analisis_error",
];

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScoutRow {
    pub proceso_id: String,
    pub entidad: String,
    pub expediente: String,
    pub fecha_recepcion: DateTime<Utc>,
    pub ultima_deteccion: DateTime<Utc>,
    pub puntaje: u8,
    pub rubro: String,
    pub resumen_corto: String,
    pub items: Vec<String>,
    pub contenido_hash: String,
    /// Exact source values; no model output is interpreted as markup or code.
    pub source: BTreeMap<String, String>,
}

pub fn parse_csv(input: &str) -> Result<Vec<ScoutRow>, CoreError> {
    if input.len() > MAX_CSV_BYTES || input.contains('\0') {
        return Err(CoreError::Invalid);
    }
    let mut reader =
        csv::ReaderBuilder::new().from_reader(input.trim_start_matches('\u{feff}').as_bytes());
    let headers = reader.headers().map_err(|_| CoreError::Invalid)?.clone();
    if !headers.iter().eq(FIELDS) {
        return Err(CoreError::Invalid);
    }
    let mut rows = Vec::new();
    let mut ids = HashSet::new();
    for record in reader.records() {
        if rows.len() >= MAX_ROWS {
            return Err(CoreError::Invalid);
        }
        let record = record.map_err(|_| CoreError::Invalid)?;
        let source: BTreeMap<String, String> = FIELDS
            .iter()
            .zip(record.iter())
            .map(|(key, value)| ((*key).to_owned(), value.to_owned()))
            .collect();
        let get = |key: &str| {
            source
                .get(key)
                .map(String::as_str)
                .ok_or(CoreError::Invalid)
        };
        for field in [
            "run_id",
            "entidad",
            "expediente",
            "descripcion",
            "rubro",
            "resumen_corto",
            "analisis_modelo",
        ] {
            if get(field)?.trim().is_empty() {
                return Err(CoreError::Invalid);
            }
        }
        let id = get("proceso_id")?;
        if !id.strip_prefix("hc:").is_some_and(is_hash)
            || !ids.insert(id.to_owned())
            || get("extraccion_estado")? != "completo"
            || get("analisis_estado")? != "analizado"
            || !is_hash(get("contenido_hash")?)
            || get("hash_analizado")? != get("contenido_hash")?
            || !get("analisis_error")?.is_empty()
            || get("entidad")?.chars().count() > 500
            || get("resumen_corto")?.chars().count() > 16_384
            || get("expediente")?.chars().count() > 255
            || get("rubro")?.chars().count() > 500
        {
            return Err(CoreError::Invalid);
        }
        let link = url::Url::parse(get("url")?).map_err(|_| CoreError::Invalid)?;
        if link.scheme() != "https"
            || link.host_str().is_none()
            || !link.username().is_empty()
            || link.password().is_some()
        {
            return Err(CoreError::Invalid);
        }
        let date = |key| {
            DateTime::parse_from_rfc3339(get(key)?)
                .map(|date| date.with_timezone(&Utc))
                .map_err(|_| CoreError::Invalid)
        };
        for field in [
            "extraido_en",
            "primera_deteccion",
            "ultima_modificacion",
            "analizado_en",
        ] {
            date(field)?;
        }
        let puntaje: u8 = get("puntaje")?.parse().map_err(|_| CoreError::Invalid)?;
        if puntaje > 100 {
            return Err(CoreError::Invalid);
        }
        let items: Vec<String> =
            serde_json::from_str(get("items")?).map_err(|_| CoreError::Invalid)?;
        if items.iter().any(|item| item.trim().is_empty()) {
            return Err(CoreError::Invalid);
        }
        let source_items: serde_json::Value =
            serde_json::from_str(get("items_fuente")?).map_err(|_| CoreError::Invalid)?;
        if !source_items.is_array() {
            return Err(CoreError::Invalid);
        }
        rows.push(ScoutRow {
            proceso_id: id.to_owned(),
            entidad: get("entidad")?.to_owned(),
            expediente: get("expediente")?.to_owned(),
            fecha_recepcion: date("fecha_recepcion")?,
            ultima_deteccion: date("ultima_deteccion")?,
            puntaje,
            rubro: get("rubro")?.to_owned(),
            resumen_corto: get("resumen_corto")?.to_owned(),
            contenido_hash: get("contenido_hash")?.to_owned(),
            items,
            source,
        });
    }
    Ok(rows)
}

fn is_hash(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(change: Option<(&str, &str)>) -> String {
        let mut values = BTreeMap::new();
        for field in FIELDS {
            values.insert(field, "valor".to_owned());
        }
        for field in [
            "fecha_recepcion",
            "extraido_en",
            "primera_deteccion",
            "ultima_deteccion",
            "ultima_modificacion",
            "analizado_en",
        ] {
            values.insert(field, "2026-10-21T19:00:00-06:00".into());
        }
        values.insert("proceso_id", format!("hc:{}", "a".repeat(64)));
        values.insert("url", "https://example.test/detalle?id=1".into());
        values.insert("contenido_hash", "b".repeat(64));
        values.insert("hash_analizado", "b".repeat(64));
        values.insert("analisis_estado", "analizado".into());
        values.insert("extraccion_estado", "completo".into());
        values.insert("analisis_error", String::new());
        values.insert("puntaje", "75".into());
        values.insert(
            "descripcion",
            "Equipo, cables\ny referencias \"especiales\"".into(),
        );
        values.insert("items", "[\"Dos equipos, modelo A\"]".into());
        values.insert("items_fuente", "[{\"cantidad\":\"2\"}]".into());
        if let Some((key, value)) = change {
            values.insert(key, value.into());
        }
        let mut writer = csv::Writer::from_writer(Vec::new());
        writer.write_record(FIELDS).unwrap();
        writer
            .write_record(FIELDS.map(|field| &values[field]))
            .unwrap();
        format!(
            "\u{feff}{}",
            String::from_utf8(writer.into_inner().unwrap()).unwrap()
        )
    }

    #[test]
    fn accepts_scout_bom_multiline_and_json_and_preserves_source() {
        let rows = parse_csv(&fixture(None)).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].puntaje, 75);
        assert_eq!(rows[0].items, ["Dos equipos, modelo A"]);
        assert!(rows[0].source["descripcion"].contains('\n'));
        assert_eq!(
            rows[0].fecha_recepcion.to_rfc3339(),
            "2026-10-22T01:00:00+00:00"
        );
    }

    #[test]
    fn rejects_pending_inconsistent_invalid_and_duplicate_rows() {
        for change in [
            ("analisis_estado", "pendiente"),
            ("extraccion_estado", "incompleto"),
            ("hash_analizado", "otro"),
            ("puntaje", "101"),
            ("puntaje", "1.5"),
            ("fecha_recepcion", "2026-10-21"),
            ("items", "{}"),
            ("items", "[1]"),
            ("items_fuente", "null"),
            ("url", "javascript:alert(1)"),
            ("proceso_id", ""),
            ("analisis_error", "fallo"),
        ] {
            assert!(parse_csv(&fixture(Some(change))).is_err(), "{change:?}");
        }
        let input = fixture(None);
        let duplicate = input.clone() + &input.lines().skip(1).collect::<Vec<_>>().join("\n");
        assert!(parse_csv(&duplicate).is_err());
        assert!(parse_csv(&input.replace("resumen_corto", "resumen")).is_err());
        assert!(parse_csv(&"x".repeat(MAX_CSV_BYTES + 1)).is_err());
    }
}
