use crate::lots::{config, scaled};
use crate::service::{key_hash, load};
use crate::{LotItem, TenderService};
use bastion_core::{CoreAdmin, CoreError};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{MySql, QueryBuilder, Transaction};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AwardSelection {
    pub lot_number: u32,
    pub item_number: u32,
    pub quantity: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AwardInput {
    pub items: Vec<AwardSelection>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AwardItem {
    pub lot_number: u32,
    pub item_number: u32,
    pub description: String,
    pub quantity: String,
    pub unit: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AwardSnapshot {
    pub expedient_name: String,
    pub expedient_version: u64,
    pub items: Vec<AwardItem>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Award {
    pub id: u64,
    pub expedient_id: u64,
    pub version: u64,
    pub snapshot: AwardSnapshot,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct AwardSummary {
    pub id: u64,
    pub expedient_id: u64,
    pub item_count: u64,
}

impl AwardInput {
    fn validate(&self) -> Result<(), CoreError> {
        if self.items.is_empty() || self.items.len() > 200 {
            return Err(CoreError::Invalid);
        }
        let mut seen = BTreeSet::new();
        for item in &self.items {
            if item.lot_number == 0
                || item.item_number == 0
                || scaled(&item.quantity, 12)? == 0
                || !seen.insert((item.lot_number, item.item_number))
            {
                return Err(CoreError::Invalid);
            }
        }
        Ok(())
    }
}

async fn award_load(
    tx: &mut Transaction<'_, MySql>,
    id: u64,
    award: u64,
) -> Result<Award, CoreError> {
    let snapshot: serde_json::Value = sqlx::query_scalar(
        "SELECT snapshot_json FROM licitaciones_adjudicaciones WHERE id=? AND expediente_id=?",
    )
    .bind(award)
    .bind(id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(CoreError::NotFound)?;
    Ok(Award {
        id: award,
        expedient_id: id,
        version: 1,
        snapshot: serde_json::from_value(snapshot).map_err(|_| CoreError::Invalid)?,
    })
}

impl TenderService {
    pub async fn awards(
        &self,
        actor: u64,
        id: u64,
        page: u32,
    ) -> Result<Vec<AwardSummary>, CoreError> {
        if !(1..=10000).contains(&page) {
            return Err(CoreError::Invalid);
        }
        let mut tx = self.begin(actor, "ver").await?;
        load(&mut tx, id).await?;
        let awards = sqlx::query_as::<_,AwardSummary>(
            "SELECT id,expediente_id AS expedient_id,CAST(JSON_LENGTH(snapshot_json,'$.items') AS UNSIGNED) AS item_count FROM licitaciones_adjudicaciones WHERE expediente_id=? ORDER BY id DESC LIMIT 26 OFFSET ?",
        ).bind(id).bind((page - 1)*25).fetch_all(&mut *tx).await?;
        tx.commit().await?;
        Ok(awards)
    }

    pub async fn award(&self, actor: u64, id: u64, award: u64) -> Result<Award, CoreError> {
        let mut tx = self.begin(actor, "ver").await?;
        load(&mut tx, id).await?;
        let result = award_load(&mut tx, id, award).await?;
        tx.commit().await?;
        Ok(result)
    }

    pub async fn register_award(
        &self,
        actor: u64,
        id: u64,
        expected: u64,
        key: &str,
        input: AwardInput,
    ) -> Result<Award, CoreError> {
        input.validate()?;
        let key = key_hash(key)?;
        let hash = Sha256::digest(
            serde_json::to_vec(&(id, expected, &input)).map_err(|_| CoreError::Invalid)?,
        )
        .to_vec();
        let mut tx = self.begin(actor, "editar").await?;
        self.admin
            .authorize_in_transaction(&mut tx, actor, "licitaciones", "expedientes", "ver")
            .await?;
        let exp = load(&mut tx, id).await?;
        let previous: Option<(Vec<u8>, u64)> = sqlx::query_as(
            "SELECT request_hash,adjudicacion_id FROM licitaciones_adjudicacion_recibos WHERE actor_id=? AND key_hash=?",
        ).bind(actor).bind(&key).fetch_optional(&mut *tx).await?;
        if let Some((previous_hash, award)) = previous {
            if previous_hash != hash {
                return Err(CoreError::Conflict);
            }
            let result = award_load(&mut tx, id, award).await?;
            tx.commit().await?;
            return Ok(result);
        }
        if exp.version != expected {
            return Err(CoreError::PreconditionFailed);
        }
        if exp.state != "enviada" {
            return Err(CoreError::Conflict);
        }
        let mut used_query = QueryBuilder::<MySql>::new(
            "SELECT lote,item_numero,CAST(SUM(cantidad) AS CHAR) FROM licitaciones_adjudicacion_items WHERE expediente_id=",
        );
        used_query
            .push_bind(id)
            .push(" AND (lote,item_numero) IN (");
        let mut keys = used_query.separated(",");
        for item in &input.items {
            keys.push("(")
                .push_bind_unseparated(item.lot_number)
                .push_unseparated(",")
                .push_bind_unseparated(item.item_number)
                .push_unseparated(")");
        }
        used_query.push(") GROUP BY lote,item_numero");
        let used: Vec<(u32, u32, String)> = used_query.build_query_as().fetch_all(&mut *tx).await?;
        let used: BTreeMap<_, _> = used
            .into_iter()
            .map(|(lot, item, qty)| ((lot, item), qty))
            .collect();
        let mut lots = BTreeMap::<u32, Vec<LotItem>>::new();
        let mut items = Vec::with_capacity(input.items.len());
        for item in &input.items {
            config(&exp, item.lot_number)?;
            if !lots.contains_key(&item.lot_number) {
                let raw: serde_json::Value = sqlx::query_scalar(
                    "SELECT items_json FROM licitaciones_lotes WHERE expediente_id=? AND numero=?",
                )
                .bind(id)
                .bind(item.lot_number)
                .fetch_optional(&mut *tx)
                .await?
                .ok_or(CoreError::Invalid)?;
                lots.insert(
                    item.lot_number,
                    serde_json::from_value(raw).map_err(|_| CoreError::Invalid)?,
                );
            }
            let offered = lots[&item.lot_number]
                .get(item.item_number as usize - 1)
                .ok_or(CoreError::Invalid)?;
            let previous = used
                .get(&(item.lot_number, item.item_number))
                .map(|qty| scaled(qty, 12))
                .transpose()?
                .unwrap_or(0);
            let quantity = scaled(&item.quantity, 12)?;
            if previous.checked_add(quantity).ok_or(CoreError::Invalid)?
                > scaled(&offered.cantidad, 12)?
            {
                return Err(CoreError::Conflict);
            }
            items.push(AwardItem {
                lot_number: item.lot_number,
                item_number: item.item_number,
                description: offered.descripcion.clone(),
                quantity: item.quantity.clone(),
                unit: offered.unidad.clone(),
            });
        }
        let snapshot = AwardSnapshot {
            expedient_name: exp.name,
            expedient_version: exp.version,
            items,
        };
        let award = sqlx::query(
            "INSERT INTO licitaciones_adjudicaciones(expediente_id,snapshot_json,created_by) VALUES (?,?,?)",
        ).bind(id).bind(serde_json::to_value(&snapshot).map_err(|_| CoreError::Invalid)?).bind(actor)
            .execute(&mut *tx).await?.last_insert_id();
        for item in &input.items {
            sqlx::query("INSERT INTO licitaciones_adjudicacion_items(adjudicacion_id,expediente_id,lote,item_numero,cantidad) VALUES (?,?,?,?,?)")
                .bind(award).bind(id).bind(item.lot_number).bind(item.item_number).bind(&item.quantity).execute(&mut *tx).await?;
        }
        sqlx::query("INSERT INTO licitaciones_adjudicacion_recibos(actor_id,key_hash,request_hash,adjudicacion_id) VALUES (?,?,?,?)")
            .bind(actor).bind(key).bind(hash).bind(award).execute(&mut *tx).await?;
        CoreAdmin::record_domain_event(
            &mut tx,
            actor,
            "licitaciones",
            "award.register",
            &award.to_string(),
            serde_json::json!({"expedient_id":id,"expedient_version":exp.version}),
        )
        .await?;
        tx.commit().await?;
        Ok(Award {
            id: award,
            expedient_id: id,
            version: 1,
            snapshot,
        })
    }

    /// Owner port for the document coordinator. Locks follow Tender -> Invoice.
    pub async fn award_source_in_transaction(
        &self,
        tx: &mut Transaction<'_, MySql>,
        actor: u64,
        id: u64,
        award: u64,
        capability: &str,
    ) -> Result<Award, CoreError> {
        let enabled: Option<bool> = sqlx::query_scalar("SELECT (included_in_release AND environment_allowed AND enabled) FROM module_registry WHERE module_id='licitaciones' FOR UPDATE")
            .fetch_optional(&mut **tx).await?;
        if enabled != Some(true) {
            return Err(CoreError::NotFound);
        }
        for capability in ["ver", capability] {
            self.admin
                .authorize_in_transaction(tx, actor, "licitaciones", "expedientes", capability)
                .await?;
        }
        if load(tx, id).await?.state != "enviada" {
            return Err(CoreError::Conflict);
        }
        award_load(tx, id, award).await
    }
}

#[derive(Serialize)]
pub struct AvailableAwardItem {
    pub item_number: u32,
    pub description: String,
    pub offered_quantity: String,
    pub remaining_quantity: String,
    pub unit: String,
}
#[derive(Serialize)]
pub struct AvailableAwardLot {
    pub lot_number: u32,
    pub version: u64,
    pub items: Vec<AvailableAwardItem>,
}
impl TenderService {
    pub async fn available_award_lot(
        &self,
        actor: u64,
        id: u64,
        number: u32,
    ) -> Result<AvailableAwardLot, CoreError> {
        let mut tx = self.begin(actor, "ver").await?;
        let exp = load(&mut tx, id).await?;
        if exp.state != "enviada" {
            return Err(CoreError::Conflict);
        }
        config(&exp, number)?;
        let raw: Option<serde_json::Value> = sqlx::query_scalar(
            "SELECT items_json FROM licitaciones_lotes WHERE expediente_id=? AND numero=?",
        )
        .bind(id)
        .bind(number)
        .fetch_optional(&mut *tx)
        .await?;
        let offered: Vec<LotItem> = raw
            .map(serde_json::from_value)
            .transpose()
            .map_err(|_| CoreError::Invalid)?
            .unwrap_or_default();
        let used:Vec<(u32,String)> = sqlx::query_as("SELECT item_numero,CAST(SUM(cantidad) AS CHAR) FROM licitaciones_adjudicacion_items WHERE expediente_id=? AND lote=? GROUP BY item_numero")
            .bind(id).bind(number).fetch_all(&mut *tx).await?;
        let used: BTreeMap<_, _> = used.into_iter().collect();
        let mut items = Vec::with_capacity(offered.len());
        for (index, item) in offered.into_iter().enumerate() {
            let item_number = index as u32 + 1;
            let quantity = scaled(&item.cantidad, 12)?;
            let previous = used
                .get(&item_number)
                .map(|value| scaled(value, 12))
                .transpose()?
                .unwrap_or(0);
            let remaining = quantity
                .checked_sub(previous)
                .filter(|value| *value >= 0)
                .ok_or(CoreError::Conflict)?;
            let fraction = format!("{:012}", remaining % 1_000_000_000_000_i128);
            let fraction = fraction.trim_end_matches('0');
            let remaining_quantity = if fraction.is_empty() {
                (remaining / 1_000_000_000_000_i128).to_string()
            } else {
                format!("{}.{}", remaining / 1_000_000_000_000_i128, fraction)
            };
            items.push(AvailableAwardItem {
                item_number,
                description: item.descripcion,
                offered_quantity: item.cantidad,
                remaining_quantity,
                unit: item.unidad,
            });
        }
        tx.commit().await?;
        Ok(AvailableAwardLot {
            lot_number: number,
            version: exp.version,
            items,
        })
    }
}
