use std::io::{self, Write};

use bastion_core::CoreError;
use bastion_cotizaciones::{QuoteFilter, QuoteService};
use bastion_facturas::{InvoiceFilter, InvoiceService};
use bastion_mcp::AuthenticatedAccess;
use bastion_ordenes_compra::{OrderFilter, OrderService};
use serde::{Deserialize, Serialize};
use serde_json::Value;

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
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ListArguments {
    search: Option<String>,
    page: Option<u32>,
    page_size: Option<u32>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DetailArguments {
    id: u64,
}

/// Typed draft adapter over existing domain services. The caller must provide
/// freshly authenticated OAuth access, enforce per-tool concurrency/time and
/// quotas, audit all outcomes, and never register it before end-to-end tests.
pub struct DraftReadExecutor {
    quotes: QuoteService,
    invoices: InvoiceService,
    orders: OrderService,
}

impl DraftReadExecutor {
    pub fn new(quotes: QuoteService, invoices: InvoiceService, orders: OrderService) -> Self {
        Self {
            quotes,
            invoices,
            orders,
        }
    }

    pub async fn call(
        &self,
        access: &AuthenticatedAccess,
        tool: &str,
        arguments: Value,
    ) -> Result<Value, ReadToolError> {
        let (module, operation) = tool.split_once('.').ok_or(ReadToolError::Invalid)?;
        if !matches!(module, "cotizaciones" | "facturas" | "ordenes_compra")
            || !matches!(operation, "listar" | "consultar")
        {
            return Err(ReadToolError::Invalid);
        }
        if !access.scopes.contains(&format!("{module}:read")) {
            return Err(ReadToolError::Denied);
        }
        if operation == "listar" {
            let args: ListArguments =
                serde_json::from_value(arguments).map_err(|_| ReadToolError::Invalid)?;
            let page = args.page.unwrap_or(1);
            let size = args.page_size.unwrap_or(25);
            if !(1..=10000).contains(&page)
                || !(1..=25).contains(&size)
                || args
                    .search
                    .as_ref()
                    .is_some_and(|s| s.chars().count() > 200)
            {
                return Err(ReadToolError::Invalid);
            }
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
                _ => Err(ReadToolError::Invalid),
            }
        }
    }
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
