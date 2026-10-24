use bastion_core::CoreError;
use bastion_cotizaciones::{QuoteDraft, QuoteService, QuoteStatus};
use bastion_ordenes_compra::{
    OrderCalculation, OrderInput, OrderProduct, OrderService, ProductStatus, PurchaseOrder,
    SourceQuote,
};
use chrono::NaiveDate;
use serde::Serialize;
use sha2::{Digest, Sha256};

#[derive(Debug, Serialize)]
pub struct QuoteOrderSeed {
    pub source: SourceQuote,
    pub data: OrderInput,
    pub calculation: OrderCalculation,
}

pub fn seed_quote_order(source: &QuoteDraft, date: NaiveDate) -> Result<QuoteOrderSeed, CoreError> {
    if !matches!(source.data.status, QuoteStatus::Aprobada) {
        return Err(CoreError::Conflict);
    }
    let quote = bastion_cotizaciones::calculate(&source.data)?;
    if quote.totals.total != source.calculation.totals.total {
        return Err(CoreError::Conflict);
    }
    let products = quote
        .data
        .items
        .iter()
        .map(|item| OrderProduct {
            description: item.description.clone(),
            cantidad: item.quantity.clone(),
            sale_price: item.unit_price.clone(),
            purchase_price: item.unit_price.clone(),
            tracking: String::new(),
            isv: matches!(item.tax_type, bastion_cotizaciones::TaxType::Isv15),
            status: ProductStatus::Pendiente,
            coments: String::new(),
        })
        .collect();
    let data = OrderInput {
        approved_quote: Some(source.data.clone()),
        n_orders: if quote.data.order_number.chars().count() <= 50 {
            quote.data.order_number.clone()
        } else {
            String::new()
        },
        n_bill: String::new(),
        order_date: date,
        received_date: None,
        client_name: quote.data.client_name.clone(),
        client_rtn: quote.data.client_rtn.clone(),
        id_destination: None,
        comments: String::new(),
        products,
        expenses: Vec::new(),
    };
    let calculation = bastion_ordenes_compra::calculate(&data)?;
    Ok(QuoteOrderSeed {
        source: SourceQuote {
            id: source.id,
            version: source.version,
            quote_number: source.quote_number.clone(),
        },
        data: calculation.data.clone(),
        calculation,
    })
}

pub async fn prepare_quote_order(
    quotes: &QuoteService,
    orders: &OrderService,
    actor: u64,
    id: u64,
    expected: u64,
    date: NaiveDate,
) -> Result<QuoteOrderSeed, CoreError> {
    quotes.check_access(actor, "ver").await?;
    let mut tx = quotes.begin_order_conversion(actor).await?;
    orders.enter_conversion(&mut tx, actor).await?;
    let source = quotes
        .order_conversion_source(&mut tx, id, expected)
        .await?;
    let seed = seed_quote_order(&source, date)?;
    tx.commit().await?;
    Ok(seed)
}

pub async fn convert_quote_order(
    quotes: &QuoteService,
    orders: &OrderService,
    actor: u64,
    id: u64,
    expected: u64,
    key: &str,
    input: OrderInput,
) -> Result<PurchaseOrder, CoreError> {
    if !(16..=128).contains(&key.len())
        || !key
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        return Err(CoreError::Invalid);
    }
    let request = Sha256::digest(
        serde_json::to_vec(&(id, expected, &input)).map_err(|_| CoreError::Invalid)?,
    );
    let key = Sha256::digest(key.as_bytes());
    quotes.check_access(actor, "ver").await?;
    let mut tx = quotes.begin_order_conversion(actor).await?;
    orders.enter_conversion(&mut tx, actor).await?;
    if let Some(previous) = orders
        .conversion_receipt(&mut tx, actor, &key, &request)
        .await?
    {
        tx.commit().await?;
        return Ok(previous);
    }
    let source = quotes
        .order_conversion_source(&mut tx, id, expected)
        .await?;
    if serde_json::to_value(&input.approved_quote).map_err(|_| CoreError::Invalid)?
        != serde_json::to_value(Some(&source.data)).map_err(|_| CoreError::Invalid)?
    {
        return Err(CoreError::Conflict);
    }
    let calculation = bastion_ordenes_compra::calculate(&input)?;
    if calculation.totals.total != source.calculation.totals.total {
        return Err(CoreError::Conflict);
    }
    let origin = SourceQuote {
        id,
        version: expected,
        quote_number: source.quote_number.clone(),
    };
    let order = orders
        .persist_conversion(&mut tx, actor, &key, &request, input, origin)
        .await?;
    quotes
        .finish_order_conversion(&mut tx, actor, source, order.id)
        .await?;
    tx.commit().await?;
    Ok(order)
}
