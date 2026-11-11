use std::io::{self, Write};

use bastion_core::CoreError;
use bastion_cotizaciones::{QuoteFilter, QuoteInput, QuoteService, QuoteStatus};
use bastion_facturas::{InvoiceFilter, InvoiceInput, InvoiceService};
use bastion_licitaciones::{GlobalVariablesInput, LotInput, NameInput, PageQuery, TenderService};
use bastion_mcp::AuthenticatedAccess;
use bastion_ordenes_compra::{OrderFilter, OrderInput, OrderService, ProductStatus};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[path = "additional_reads.rs"]
mod additional_reads;
use additional_reads::AdditionalReads;

const MAX_RESPONSE_BYTES: usize = 262_144;
const MAX_SAFE_ID: u64 = 9_007_199_254_740_991;

#[derive(Debug, PartialEq, Eq)]
pub enum ReadToolError {
    Invalid,
    Denied,
    NotFound,
    Limited,
    Unavailable,
    TooLarge,
    Conflict,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ListArguments {
    search: Option<String>,
    page: Option<u32>,
    page_size: Option<u32>,
}

impl ListArguments {
    fn bounds(&self) -> Result<(u32, u32), ReadToolError> {
        let page = self.page.unwrap_or(1);
        let size = self.page_size.unwrap_or(25);
        if !(1..=10000).contains(&page)
            || !(1..=25).contains(&size)
            || self
                .search
                .as_ref()
                .is_some_and(|s| s.chars().count() > 200)
        {
            return Err(ReadToolError::Invalid);
        }
        Ok((page, size))
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DetailArguments {
    id: u64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EmptyArguments {}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateArguments<T> {
    idempotency_key: String,
    data: T,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UpdateArguments<T> {
    id: u64,
    version: u64,
    data: T,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TenderCreateArguments {
    idempotency_key: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TenderNameArguments {
    id: u64,
    version: u64,
    name: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TenderVariablesArguments {
    id: u64,
    version: u64,
    variables: Vec<bastion_licitaciones::GlobalVariable>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TenderLotArguments {
    id: u64,
    numero: u32,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TenderLotWriteArguments {
    id: u64,
    version: u64,
    numero: u32,
    items: Vec<bastion_licitaciones::LotItem>,
}

/// Typed draft adapter over existing domain services. The caller must provide
/// freshly authenticated OAuth access, enforce per-tool concurrency/time and
/// quotas, audit all outcomes, and never register it before end-to-end tests.
pub struct DraftReadExecutor {
    quotes: QuoteService,
    invoices: InvoiceService,
    orders: OrderService,
    tenders: Option<TenderService>,
    additional: Option<AdditionalReads>,
    quote_mail: Option<crate::QuoteMailConfirmation>,
}

impl DraftReadExecutor {
    pub fn new(quotes: QuoteService, invoices: InvoiceService, orders: OrderService) -> Self {
        Self {
            quotes,
            invoices,
            orders,
            tenders: None,
            additional: None,
            quote_mail: None,
        }
    }

    pub fn with_tenders(mut self, tenders: TenderService) -> Self {
        self.tenders = Some(tenders);
        self
    }

    pub fn with_quote_mail(mut self, mail: crate::QuoteMailConfirmation) -> Self {
        self.quote_mail = Some(mail);
        self
    }

    pub async fn call(
        &self,
        access: &AuthenticatedAccess,
        tool: &str,
        arguments: Value,
    ) -> Result<Value, ReadToolError> {
        let (module, operation) = tool.split_once('.').ok_or(ReadToolError::Invalid)?;
        if module == "cotizaciones" && matches!(operation, "preparar_envio" | "consultar_envio") {
            let service = self.quote_mail.as_ref().ok_or(ReadToolError::Unavailable)?;
            if operation == "consultar_envio" {
                #[derive(Deserialize)]
                #[serde(deny_unknown_fields)]
                struct Status {
                    handle: String,
                }
                let args: Status =
                    serde_json::from_value(arguments).map_err(|_| ReadToolError::Invalid)?;
                return bounded(
                    &service
                        .status(access, &args.handle)
                        .await
                        .map_err(map_error)?,
                );
            }
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Prepare {
                id: u64,
                version: u64,
                data: bastion_cotizaciones::QuoteEmailInput,
            }
            let args: Prepare =
                serde_json::from_value(arguments).map_err(|_| ReadToolError::Invalid)?;
            valid_version(args.id, args.version)?;
            return bounded(
                &service
                    .prepare(access, args.id, args.version, args.data)
                    .await
                    .map_err(map_error)?,
            );
        }
        if matches!(
            module,
            "procesos" | "clientes" | "proveedores" | "gastos" | "colaboracion" | "reportes"
        ) {
            return self
                .call_additional(access, module, operation, arguments)
                .await;
        }
        if !matches!(
            module,
            "cotizaciones" | "facturas" | "ordenes_compra" | "licitaciones"
        ) {
            return Err(ReadToolError::Invalid);
        }
        let reading = matches!(operation, "listar" | "consultar")
            || (module == "ordenes_compra" && operation == "listar_destinos")
            || (module == "licitaciones" && matches!(operation, "listar_lotes" | "consultar_lote"));
        if !access.scopes.contains(&format!(
            "{module}:{}",
            if reading { "read" } else { "write" }
        )) {
            return Err(ReadToolError::Denied);
        }
        if !reading {
            if !access.scopes.contains(&format!("{module}:read")) {
                return Err(ReadToolError::Denied);
            }
            return self.write(access, module, operation, arguments).await;
        }
        if module == "ordenes_compra" && operation == "listar_destinos" {
            let _: EmptyArguments =
                serde_json::from_value(arguments).map_err(|_| ReadToolError::Invalid)?;
            self.orders
                .consume_request(access.user_id)
                .await
                .map_err(map_error)?;
            return bounded(
                &self
                    .orders
                    .destinations(access.user_id)
                    .await
                    .map_err(map_error)?,
            );
        }
        if module == "licitaciones" && operation == "listar_lotes" {
            let args: DetailArguments =
                serde_json::from_value(arguments).map_err(|_| ReadToolError::Invalid)?;
            valid_version(args.id, 1)?;
            let service = self.tenders.as_ref().ok_or(ReadToolError::Unavailable)?;
            service
                .consume_request(access.user_id)
                .await
                .map_err(map_error)?;
            return bounded(
                &service
                    .lot_summaries(access.user_id, args.id)
                    .await
                    .map_err(map_error)?,
            );
        }
        if module == "licitaciones" && operation == "consultar_lote" {
            let args: TenderLotArguments =
                serde_json::from_value(arguments).map_err(|_| ReadToolError::Invalid)?;
            valid_version(args.id, 1)?;
            if args.numero == 0 {
                return Err(ReadToolError::Invalid);
            }
            let service = self.tenders.as_ref().ok_or(ReadToolError::Unavailable)?;
            service
                .consume_request(access.user_id)
                .await
                .map_err(map_error)?;
            return bounded(
                &service
                    .lot(access.user_id, args.id, args.numero)
                    .await
                    .map_err(map_error)?,
            );
        }
        if operation == "listar" {
            let args: ListArguments =
                serde_json::from_value(arguments).map_err(|_| ReadToolError::Invalid)?;
            let (page, size) = args.bounds()?;
            match module {
                "cotizaciones" => {
                    self.quotes
                        .consume_request(access.user_id)
                        .await
                        .map_err(map_error)?;
                    let filter = QuoteFilter {
                        q: args.search,
                        page: Some(page),
                        page_size: Some(size),
                    };
                    bounded(
                        &self
                            .quotes
                            .list(access.user_id, &filter)
                            .await
                            .map_err(map_error)?,
                    )
                }
                "facturas" => {
                    self.invoices
                        .consume_request(access.user_id)
                        .await
                        .map_err(map_error)?;
                    let filter = InvoiceFilter {
                        search: args.search,
                        page: Some(page),
                        page_size: Some(size),
                    };
                    bounded(
                        &self
                            .invoices
                            .list(access.user_id, &filter)
                            .await
                            .map_err(map_error)?,
                    )
                }
                "ordenes_compra" => {
                    self.orders
                        .consume_request(access.user_id)
                        .await
                        .map_err(map_error)?;
                    let filter = OrderFilter {
                        search: args.search,
                        page: Some(page),
                        page_size: Some(size),
                        ..Default::default()
                    };
                    bounded(
                        &self
                            .orders
                            .list(access.user_id, &filter)
                            .await
                            .map_err(map_error)?,
                    )
                }
                "licitaciones" => {
                    if args.search.is_some() {
                        return Err(ReadToolError::Invalid);
                    }
                    let service = self.tenders.as_ref().ok_or(ReadToolError::Unavailable)?;
                    service
                        .consume_request(access.user_id)
                        .await
                        .map_err(map_error)?;
                    bounded(
                        &service
                            .list(
                                access.user_id,
                                &PageQuery {
                                    page: Some(page),
                                    page_size: Some(size),
                                },
                            )
                            .await
                            .map_err(map_error)?,
                    )
                }
                _ => Err(ReadToolError::Invalid),
            }
        } else {
            let args: DetailArguments =
                serde_json::from_value(arguments).map_err(|_| ReadToolError::Invalid)?;
            if args.id == 0 || args.id > MAX_SAFE_ID {
                return Err(ReadToolError::Invalid);
            }
            match module {
                "cotizaciones" => {
                    self.quotes
                        .consume_request(access.user_id)
                        .await
                        .map_err(map_error)?;
                    bounded(
                        &self
                            .quotes
                            .detail(access.user_id, args.id)
                            .await
                            .map_err(map_error)?,
                    )
                }
                "facturas" => {
                    self.invoices
                        .consume_request(access.user_id)
                        .await
                        .map_err(map_error)?;
                    bounded(
                        &self
                            .invoices
                            .detail(access.user_id, args.id)
                            .await
                            .map_err(map_error)?,
                    )
                }
                "ordenes_compra" => {
                    self.orders
                        .consume_request(access.user_id)
                        .await
                        .map_err(map_error)?;
                    bounded(
                        &self
                            .orders
                            .detail(access.user_id, args.id)
                            .await
                            .map_err(map_error)?,
                    )
                }
                "licitaciones" => {
                    let service = self.tenders.as_ref().ok_or(ReadToolError::Unavailable)?;
                    service
                        .consume_request(access.user_id)
                        .await
                        .map_err(map_error)?;
                    bounded(
                        &service
                            .detail(access.user_id, args.id)
                            .await
                            .map_err(map_error)?,
                    )
                }
                _ => Err(ReadToolError::Invalid),
            }
        }
    }

    async fn write(
        &self,
        access: &AuthenticatedAccess,
        module: &str,
        operation: &str,
        arguments: Value,
    ) -> Result<Value, ReadToolError> {
        let actor = access.user_id;
        match (module, operation) {
            ("cotizaciones", "crear_borrador") => {
                let args: CreateArguments<QuoteInput> =
                    serde_json::from_value(arguments).map_err(|_| ReadToolError::Invalid)?;
                if !matches!(args.data.status, QuoteStatus::Borrador) {
                    return Err(ReadToolError::Invalid);
                }
                self.quotes
                    .consume_request(actor)
                    .await
                    .map_err(map_error)?;
                bounded(
                    &self
                        .quotes
                        .create(actor, &args.idempotency_key, args.data)
                        .await
                        .map_err(map_error)?,
                )
            }
            ("cotizaciones", "editar_borrador") => {
                let args: UpdateArguments<QuoteInput> =
                    serde_json::from_value(arguments).map_err(|_| ReadToolError::Invalid)?;
                valid_version(args.id, args.version)?;
                if !matches!(args.data.status, QuoteStatus::Borrador) {
                    return Err(ReadToolError::Invalid);
                }
                self.quotes
                    .consume_request(actor)
                    .await
                    .map_err(map_error)?;
                let current = self
                    .quotes
                    .detail(actor, args.id)
                    .await
                    .map_err(map_error)?;
                if current.version != args.version || current.status != "Borrador" {
                    return Err(ReadToolError::Conflict);
                }
                bounded(
                    &self
                        .quotes
                        .replace(actor, args.id, args.version, args.data)
                        .await
                        .map_err(map_error)?,
                )
            }
            ("facturas", "crear_borrador") => {
                let args: CreateArguments<InvoiceInput> =
                    serde_json::from_value(arguments).map_err(|_| ReadToolError::Invalid)?;
                if args.data.initial_payment.is_some() {
                    return Err(ReadToolError::Invalid);
                }
                self.invoices
                    .consume_request(actor)
                    .await
                    .map_err(map_error)?;
                bounded(
                    &self
                        .invoices
                        .create(actor, &args.idempotency_key, args.data)
                        .await
                        .map_err(map_error)?,
                )
            }
            ("facturas", "editar_borrador") => {
                let args: UpdateArguments<InvoiceInput> =
                    serde_json::from_value(arguments).map_err(|_| ReadToolError::Invalid)?;
                valid_version(args.id, args.version)?;
                if args.data.initial_payment.is_some() {
                    return Err(ReadToolError::Invalid);
                }
                self.invoices
                    .consume_request(actor)
                    .await
                    .map_err(map_error)?;
                let current = self
                    .invoices
                    .detail(actor, args.id)
                    .await
                    .map_err(map_error)?;
                if current.version != args.version || current.status != "Borrador" {
                    return Err(ReadToolError::Conflict);
                }
                bounded(
                    &self
                        .invoices
                        .replace(actor, args.id, args.version, args.data)
                        .await
                        .map_err(map_error)?,
                )
            }
            ("ordenes_compra", "crear_pendiente") => {
                let args: CreateArguments<OrderInput> =
                    serde_json::from_value(arguments).map_err(|_| ReadToolError::Invalid)?;
                if !pending_order(&args.data) {
                    return Err(ReadToolError::Invalid);
                }
                self.orders
                    .consume_request(actor)
                    .await
                    .map_err(map_error)?;
                bounded(
                    &self
                        .orders
                        .create(actor, &args.idempotency_key, args.data)
                        .await
                        .map_err(map_error)?,
                )
            }
            ("ordenes_compra", "editar_pendiente") => {
                let args: UpdateArguments<OrderInput> =
                    serde_json::from_value(arguments).map_err(|_| ReadToolError::Invalid)?;
                valid_version(args.id, args.version)?;
                if !pending_order(&args.data) {
                    return Err(ReadToolError::Invalid);
                }
                self.orders
                    .consume_request(actor)
                    .await
                    .map_err(map_error)?;
                let current = self
                    .orders
                    .detail(actor, args.id)
                    .await
                    .map_err(map_error)?;
                if current.version != args.version
                    || current.status != "Pendiente"
                    || !pending_order(&current.data)
                {
                    return Err(ReadToolError::Conflict);
                }
                bounded(
                    &self
                        .orders
                        .replace(actor, args.id, args.version, args.data)
                        .await
                        .map_err(map_error)?,
                )
            }
            ("licitaciones", operation) => {
                let service = self.tenders.as_ref().ok_or(ReadToolError::Unavailable)?;
                service.consume_request(actor).await.map_err(map_error)?;
                match operation {
                    "crear_borrador" => {
                        let args: TenderCreateArguments = serde_json::from_value(arguments)
                            .map_err(|_| ReadToolError::Invalid)?;
                        bounded(
                            &service
                                .create(actor, &args.idempotency_key)
                                .await
                                .map_err(map_error)?,
                        )
                    }
                    "renombrar_borrador" => {
                        let args: TenderNameArguments = serde_json::from_value(arguments)
                            .map_err(|_| ReadToolError::Invalid)?;
                        valid_version(args.id, args.version)?;
                        bounded(
                            &service
                                .rename(actor, args.id, args.version, NameInput { name: args.name })
                                .await
                                .map_err(map_error)?,
                        )
                    }
                    "editar_variables" => {
                        let args: TenderVariablesArguments = serde_json::from_value(arguments)
                            .map_err(|_| ReadToolError::Invalid)?;
                        valid_version(args.id, args.version)?;
                        bounded(
                            &service
                                .save_global_variables(
                                    actor,
                                    args.id,
                                    args.version,
                                    GlobalVariablesInput {
                                        variables: args.variables,
                                    },
                                    false,
                                )
                                .await
                                .map_err(map_error)?,
                        )
                    }
                    "editar_lote" => {
                        let args: TenderLotWriteArguments = serde_json::from_value(arguments)
                            .map_err(|_| ReadToolError::Invalid)?;
                        valid_version(args.id, args.version)?;
                        if args.numero == 0 {
                            return Err(ReadToolError::Invalid);
                        }
                        bounded(
                            &service
                                .save_lot(
                                    actor,
                                    args.id,
                                    args.numero,
                                    args.version,
                                    LotInput { items: args.items },
                                )
                                .await
                                .map_err(map_error)?,
                        )
                    }
                    _ => Err(ReadToolError::Invalid),
                }
            }
            _ => Err(ReadToolError::Invalid),
        }
    }
}

fn valid_version(id: u64, version: u64) -> Result<(), ReadToolError> {
    if id == 0 || id > MAX_SAFE_ID || version == 0 || version > MAX_SAFE_ID {
        Err(ReadToolError::Invalid)
    } else {
        Ok(())
    }
}
fn pending_order(input: &OrderInput) -> bool {
    input.n_bill.trim().is_empty()
        && input.received_date.is_none()
        && input
            .products
            .iter()
            .all(|product| product.status == ProductStatus::Pendiente)
}

fn map_error(error: CoreError) -> ReadToolError {
    match error {
        CoreError::Invalid => ReadToolError::Invalid,
        CoreError::Unauthorized
        | CoreError::Forbidden
        | CoreError::ReauthenticationRequired
        | CoreError::SessionReplaced => ReadToolError::Denied,
        CoreError::NotFound => ReadToolError::NotFound,
        CoreError::Locked | CoreError::Overloaded => ReadToolError::Limited,
        CoreError::Conflict | CoreError::PreconditionFailed => ReadToolError::Conflict,
        _ => ReadToolError::Unavailable,
    }
}

struct LimitWriter(Vec<u8>);
impl Write for LimitWriter {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        if data.len() > MAX_RESPONSE_BYTES.saturating_sub(self.0.len()) {
            return Err(io::Error::other("response limit"));
        }
        self.0.extend_from_slice(data);
        Ok(data.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
fn bounded<T: Serialize>(data: &T) -> Result<Value, ReadToolError> {
    let mut output = LimitWriter(Vec::new());
    serde_json::to_writer(&mut output, data).map_err(|_| ReadToolError::TooLarge)?;
    serde_json::from_slice(&output.0).map_err(|_| ReadToolError::Unavailable)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_serialization_rejects_large_responses() {
        assert_eq!(
            bounded(&"a".repeat(MAX_RESPONSE_BYTES)),
            Err(ReadToolError::TooLarge)
        );
        assert!(bounded(&"small").is_ok());
    }
}
