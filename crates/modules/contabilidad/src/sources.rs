use crate::*;
use sqlx::{MySql, Transaction};

impl AccountingService {
    pub(crate) async fn source(
        &self,
        tx: &mut Transaction<'_, MySql>,
        actor: u64,
        module: &str,
        id: u64,
    ) -> Result<SourceSnapshot, CoreError> {
        if id == 0 {
            return Err(CoreError::Invalid);
        }
        match module {
            "gastos" => {
                let row =
                    bastion_gastos::ExpenseService::new(self.database.clone(), self.admin.clone())
                        .accounting_snapshot(tx, actor, id)
                        .await?;
                if row.data.currency != "HNL" {
                    return Err(CoreError::Conflict);
                }
                Ok(SourceSnapshot {
                    reference: SourceRef {
                        module: module.into(),
                        id,
                        version: row.version,
                    },
                    label: format!("{} · {}", row.data.invoice_number, row.data.provider),
                    date: row.data.expense_date,
                    amount: row.total,
                    tax: row.isv_15,
                })
            }
            "facturas" => {
                let row = bastion_facturas::InvoiceService::new(
                    self.database.clone(),
                    self.admin.clone(),
                )
                .accounting_snapshot(tx, actor, id)
                .await?;
                if row.status == "Borrador" || row.invoice_number.is_none() || row.currency != "HNL"
                {
                    return Err(CoreError::Conflict);
                }
                let tax = cents(&row.calculation.totals.isv_15)?
                    .checked_add(cents(&row.calculation.totals.isv_18)?)
                    .ok_or(CoreError::Invalid)?;
                Ok(SourceSnapshot {
                    reference: SourceRef {
                        module: module.into(),
                        id,
                        version: row.version,
                    },
                    label: format!(
                        "{} · {}",
                        row.invoice_number.unwrap_or_default(),
                        row.data.client_name
                    ),
                    date: row.data.invoice_date,
                    amount: row.calculation.totals.total,
                    tax: money(tax),
                })
            }
            "ordenes_compra" => {
                let row = bastion_ordenes_compra::OrderService::new(
                    self.database.clone(),
                    self.admin.clone(),
                )
                .accounting_snapshot(tx, actor, id)
                .await?;
                if row.currency != "HNL"
                    || row.data.received_date.is_none()
                    || row.data.products.is_empty()
                    || row.data.products.iter().any(|p| {
                        !matches!(p.status, bastion_ordenes_compra::ProductStatus::Comprado)
                    })
                {
                    return Err(CoreError::Conflict);
                }
                Ok(SourceSnapshot {
                    reference: SourceRef {
                        module: module.into(),
                        id,
                        version: row.version,
                    },
                    label: row.data.n_orders,
                    date: row.data.received_date.ok_or(CoreError::Invalid)?,
                    amount: row.calculation.totals.investment_total,
                    tax: "0.00".into(),
                })
            }
            _ => Err(CoreError::Invalid),
        }
    }
    pub async fn proposal(&self, actor: u64, module: &str, id: u64) -> Result<Proposal, CoreError> {
        let mut tx = self.begin(actor, "ver").await?;
        let source = self.source(&mut tx, actor, module, id).await?;
        let total = cents(&source.amount)?;
        if total == 0 {
            return Err(CoreError::Invalid);
        }
        let line = |account: &str, debit: i64, credit: i64| LineInput {
            account: account.into(),
            debit: money(debit),
            credit: money(credit),
        };
        let (lines, notice) = match module {
            "facturas" => {
                let tax = cents(&source.tax)?;
                let base = total
                    .checked_sub(tax)
                    .filter(|n| *n > 0)
                    .ok_or(CoreError::Invalid)?;
                let mut lines = vec![line("1104", total, 0), line("4101", 0, base)];
                if tax > 0 {
                    lines.push(line("2107", 0, tax));
                }
                (
                    lines,
                    "Reconoce la venta emitida. Revisa mercadería o servicios. Los cobros y el costo de venta se registran por separado; no se duplican automáticamente.",
                )
            }
            "gastos" => (
                vec![line("6115", total, 0), line("2101", 0, total)],
                "Selecciona la cuenta del gasto y su contrapartida real. El importe incluye ISV; sólo sepáralo como crédito fiscal si su elegibilidad y soporte están revisados.",
            ),
            _ => (
                vec![line("1105", total, 0), line("2101", 0, total)],
                "Inversión de la orden recibida: confirma facturas, costo de casillero y pagos reales. No vuelvas a registrar estos mismos costos desde Gastos. No se presume crédito fiscal ni se modifica inventario físico.",
            ),
        };
        let entry = EntryInput {
            date: source.date,
            description: format!(
                "{} #{}",
                match module {
                    "gastos" => "Gasto",
                    "facturas" => "Venta",
                    _ => "Compra recibida",
                },
                id
            ),
            reference: source.label.chars().take(120).collect(),
            kind: "origen".into(),
            source: Some(source.reference.clone()),
            lines,
        };
        entry.validate()?;
        tx.commit().await?;
        Ok(Proposal {
            entry,
            source,
            notice: notice.into(),
        })
    }
}
