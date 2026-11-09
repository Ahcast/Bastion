use bastion_clientes::{ClientFilter, ClientService};
use bastion_colaboracion::{CollaborationService, FileFilter};
use bastion_core::{CoreAdmin, Database};
use bastion_facturas::InvoiceReportFilter;
use bastion_gastos::ExpenseReportFilter;
use bastion_gastos::{ExpenseFilter, ExpenseService};
use bastion_ordenes_compra::OrderReportFilter;
use bastion_procesos::{ProcessFilter, ProcessService};
use bastion_proveedores::{SupplierFilter, SupplierService};
use chrono::NaiveDate;

use super::{
    DetailArguments, DraftReadExecutor, ListArguments, ReadToolError, bounded, map_error,
    valid_version,
};
use crate::business_registry::REPORTS;
use bastion_mcp::AuthenticatedAccess;
use bastion_reportes::ReportService;
use serde::Deserialize;
use serde_json::Value;

pub(super) struct AdditionalReads {
    clientes: ClientService,
    proveedores: SupplierService,
    gastos: ExpenseService,
    procesos: ProcessService,
    colaboracion: CollaborationService,
    reports: ReportService,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ProcessDetail {
    id: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReportArguments {
    date_from: NaiveDate,
    date_to: NaiveDate,
    search: Option<String>,
    page: Option<u32>,
    page_size: Option<u32>,
}

impl DraftReadExecutor {
    pub fn with_additional_reads(mut self, db: Database, admin: CoreAdmin) -> Self {
        self.additional = Some(AdditionalReads {
            clientes: ClientService::new(db.clone(), admin.clone()),
            proveedores: SupplierService::new(db.clone(), admin.clone()),
            gastos: ExpenseService::new(db.clone(), admin.clone()),
            procesos: ProcessService::new(db.clone(), admin.clone()),
            colaboracion: CollaborationService::new(db.clone(), admin.clone()),
            reports: ReportService::new(db, admin),
        });
        self
    }

    pub(super) async fn call_additional(
        &self,
        access: &AuthenticatedAccess,
        module: &str,
        operation: &str,
        arguments: Value,
    ) -> Result<Value, ReadToolError> {
        if module == "reportes" {
            return self.call_report(access, operation, arguments).await;
        }
        if !matches!(operation, "listar" | "consultar")
            || (module == "colaboracion" && operation != "listar")
        {
            return Err(ReadToolError::Invalid);
        }
        if !access.scopes.contains(&format!("{module}:read")) {
            return Err(ReadToolError::Denied);
        }
        let services = self.additional.as_ref().ok_or(ReadToolError::Unavailable)?;
        let actor = access.user_id;
        if operation == "listar" {
            let args: ListArguments =
                serde_json::from_value(arguments).map_err(|_| ReadToolError::Invalid)?;
            let (page, size) = args.bounds()?;
            let page = Some(page);
            let page_size = Some(size);
            let search = args.search;
            match module {
                "clientes" => {
                    let service = &services.clientes;
                    let filter = ClientFilter {
                        search,
                        page,
                        page_size,
                    };
                    service.consume_request(actor).await.map_err(map_error)?;
                    bounded(&service.list(actor, &filter).await.map_err(map_error)?)
                }
                "proveedores" => {
                    let service = &services.proveedores;
                    let filter = SupplierFilter {
                        search,
                        page,
                        page_size,
                    };
                    service.consume_request(actor).await.map_err(map_error)?;
                    bounded(&service.list(actor, &filter).await.map_err(map_error)?)
                }
                "gastos" => {
                    let service = &services.gastos;
                    let filter = ExpenseFilter {
                        search,
                        page,
                        page_size,
                        date_from: None,
                        date_to: None,
                    };
                    service.consume_request(actor).await.map_err(map_error)?;
                    bounded(&service.list(actor, &filter).await.map_err(map_error)?)
                }
                "procesos" => {
                    let service = &services.procesos;
                    let filter = ProcessFilter {
                        q: search,
                        page,
                        page_size,
                        ..Default::default()
                    };
                    service.consume_request(actor).await.map_err(map_error)?;
                    bounded(&service.rows(actor, &filter).await.map_err(map_error)?)
                }
                "colaboracion" => {
                    let service = &services.colaboracion;
                    let filter = FileFilter {
                        search,
                        page,
                        page_size,
                        scope: Some("empresa".into()),
                        id_persona: None,
                        trash: false,
                    };
                    service.consume_request(actor).await.map_err(map_error)?;
                    bounded(&service.list(actor, &filter).await.map_err(map_error)?)
                }
                _ => Err(ReadToolError::Invalid),
            }
        } else if module == "procesos" {
            let args: ProcessDetail =
                serde_json::from_value(arguments).map_err(|_| ReadToolError::Invalid)?;
            if args.id.len() != 67 {
                return Err(ReadToolError::Invalid);
            }
            let service = &services.procesos;
            service.consume_request(actor).await.map_err(map_error)?;
            bounded(&service.detail(actor, &args.id).await.map_err(map_error)?)
        } else {
            let args: DetailArguments =
                serde_json::from_value(arguments).map_err(|_| ReadToolError::Invalid)?;
            valid_version(args.id, 1)?;
            match module {
                "clientes" => {
                    let service = &services.clientes;
                    service.consume_request(actor).await.map_err(map_error)?;
                    bounded(&service.detail(actor, args.id).await.map_err(map_error)?)
                }
                "proveedores" => {
                    let service = &services.proveedores;
                    service.consume_request(actor).await.map_err(map_error)?;
                    bounded(&service.detail(actor, args.id).await.map_err(map_error)?)
                }
                "gastos" => {
                    let service = &services.gastos;
                    service.consume_request(actor).await.map_err(map_error)?;
                    bounded(&service.detail(actor, args.id).await.map_err(map_error)?)
                }
                _ => Err(ReadToolError::Invalid),
            }
        }
    }

    async fn call_report(
        &self,
        access: &AuthenticatedAccess,
        report: &str,
        arguments: Value,
    ) -> Result<Value, ReadToolError> {
        if !REPORTS.iter().any(|(name, _, _)| *name == report) {
            return Err(ReadToolError::Invalid);
        }
        if !access.scopes.contains(&format!("reportes:{report}:read")) {
            return Err(ReadToolError::Denied);
        }
        let services = self.additional.as_ref().ok_or(ReadToolError::Unavailable)?;
        let args: ReportArguments =
            serde_json::from_value(arguments).map_err(|_| ReadToolError::Invalid)?;
        let date_from = args.date_from;
        let date_to = args.date_to;
        let list = ListArguments {
            page: args.page,
            page_size: args.page_size,
            search: args.search,
        };
        let (page, size) = list.bounds()?;
        let page = Some(page);
        let page_size = Some(size);
        let search = list.search;
        let actor = access.user_id;
        services
            .reports
            .check_access(actor, report)
            .await
            .map_err(map_error)?;
        services
            .reports
            .consume_request(actor)
            .await
            .map_err(map_error)?;
        match report {
            "ordenes" => {
                let filter = OrderReportFilter {
                    date_from,
                    date_to,
                    page,
                    page_size,
                    search,
                    status: None,
                };
                bounded(
                    &self
                        .orders
                        .report(actor, &filter)
                        .await
                        .map_err(map_error)?,
                )
            }
            "facturas" => {
                let filter = InvoiceReportFilter {
                    date_from,
                    date_to,
                    page,
                    page_size,
                    search,
                    status: None,
                };
                bounded(
                    &self
                        .invoices
                        .report(actor, &filter)
                        .await
                        .map_err(map_error)?,
                )
            }
            "gastos" => {
                let filter = ExpenseReportFilter {
                    date_from,
                    date_to,
                    page,
                    page_size,
                    search,
                };
                bounded(
                    &services
                        .gastos
                        .report(actor, &filter)
                        .await
                        .map_err(map_error)?,
                )
            }
            _ => Err(ReadToolError::Invalid),
        }
    }
}
