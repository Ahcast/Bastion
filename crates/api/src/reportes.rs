use super::*;
use bastion_facturas::{InvoiceReport, InvoiceReportFilter};
use bastion_gastos::{ExpenseReport, ExpenseReportFilter};
use bastion_ordenes_compra::{OrderReport, OrderReportFilter};
use bastion_reportes::{SpreadsheetCell, workbook};

const MAX_EXPORT_ROWS: u32 = 1000;

pub(super) fn routes(state: ApiState) -> Router<ApiState> {
    Router::new()
        .route("/api/v1/reportes/ordenes", get(orders))
        .route("/api/v1/reportes/ordenes/export", get(export_orders))
        .route("/api/v1/reportes/facturas", get(invoices))
        .route("/api/v1/reportes/facturas/export", get(export_invoices))
        .route("/api/v1/reportes/gastos", get(expenses))
        .route("/api/v1/reportes/gastos/export", get(export_expenses))
        .route_layer(middleware::from_fn_with_state(state, guard))
}

async fn guard(State(state): State<ApiState>, request: Request, next: Next) -> Response {
    match tokio::time::timeout(Duration::from_secs(30), async {
        let _job = state
            .report_jobs
            .clone()
            .try_acquire_owned()
            .map_err(|_| ApiError(CoreError::Overloaded))?;
        let user = actor(&state, request.headers()).await?;
        let scope = match request.uri().path().split('/').nth(4) {
            Some("ordenes") => "ordenes",
            Some("facturas") => "facturas",
            Some("gastos") => "gastos",
            _ => return Err(ApiError(CoreError::NotFound)),
        };
        state
            .reportes
            .check_access(user.id, scope)
            .await
            .map_err(ApiError)?;
        state
            .reportes
            .consume_request(user.id)
            .await
            .map_err(ApiError)?;
        Ok::<_, ApiError>(next.run(request).await)
    })
    .await
    {
        Ok(Ok(response)) => response,
        Ok(Err(error)) => error.into_response(),
        Err(_) => ApiError(CoreError::Overloaded).into_response(),
    }
}

async fn orders(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Query(filter): Query<OrderReportFilter>,
) -> Result<Json<OrderReport>, ApiError> {
    let user = actor(&state, &headers).await?;
    let report = state
        .ordenes_compra
        .report(user.id, &filter)
        .await
        .map_err(ApiError)?;
    state
        .reportes
        .record_report_event(
            user.id,
            "ordenes",
            "report.view",
            &filter.date_from.to_string(),
            &filter.date_to.to_string(),
            report.totals.records,
        )
        .await
        .map_err(ApiError)?;
    Ok(Json(report))
}

async fn export_orders(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Query(mut filter): Query<OrderReportFilter>,
) -> Result<Response, ApiError> {
    let user = actor(&state, &headers).await?;
    filter.bounds().map_err(ApiError)?;
    filter.page = Some(1);
    filter.page_size = Some(MAX_EXPORT_ROWS);
    let report = state
        .ordenes_compra
        .report(user.id, &filter)
        .await
        .map_err(ApiError)?;
    if report.has_more {
        return Ok(export_limit_problem());
    }
    let summary = [
        (
            "Registros",
            SpreadsheetCell::Number(report.totals.records.to_string()),
        ),
        (
            "Subtotal HNL",
            SpreadsheetCell::Number(report.totals.subtotal.clone()),
        ),
        (
            "ISV HNL",
            SpreadsheetCell::Number(report.totals.isv.clone()),
        ),
        (
            "Venta total HNL",
            SpreadsheetCell::Number(report.totals.total.clone()),
        ),
        (
            "Inversión en productos HNL",
            SpreadsheetCell::Number(report.totals.products_investment.clone()),
        ),
        (
            "Otros gastos HNL",
            SpreadsheetCell::Number(report.totals.other_expenses.clone()),
        ),
        (
            "Inversión total HNL",
            SpreadsheetCell::Number(report.totals.investment_total.clone()),
        ),
        (
            "Utilidad HNL",
            SpreadsheetCell::Number(report.totals.profit.clone()),
        ),
    ];
    let rows = report
        .items
        .iter()
        .map(|row| {
            vec![
                SpreadsheetCell::Text(row.id.to_string()),
                SpreadsheetCell::Text(row.n_orders.clone()),
                SpreadsheetCell::Text(row.n_bill.clone()),
                SpreadsheetCell::Text(row.client_name.clone()),
                SpreadsheetCell::Text(row.order_date.clone()),
                SpreadsheetCell::Text(row.status.clone()),
                SpreadsheetCell::Number(row.subtotal.clone()),
                SpreadsheetCell::Number(row.isv.clone()),
                SpreadsheetCell::Number(row.total.clone()),
                SpreadsheetCell::Number(row.products_investment.clone()),
                SpreadsheetCell::Number(row.other_expenses.clone()),
                SpreadsheetCell::Number(row.investment_total.clone()),
                SpreadsheetCell::Number(row.profit.clone()),
            ]
        })
        .collect::<Vec<_>>();
    state
        .reportes
        .record_report_event(
            user.id,
            "ordenes",
            "report.export",
            &filter.date_from.to_string(),
            &filter.date_to.to_string(),
            report.totals.records,
        )
        .await
        .map_err(ApiError)?;
    Ok(excel_response(
        "reporte-ordenes.xls",
        workbook(
            &summary,
            &[
                "ID",
                "N.º orden",
                "N.º factura",
                "Cliente",
                "Fecha",
                "Estado",
                "Subtotal HNL",
                "ISV HNL",
                "Venta total HNL",
                "Inversión productos HNL",
                "Otros gastos HNL",
                "Inversión total HNL",
                "Utilidad HNL",
            ],
            &rows,
        ),
    ))
}

async fn invoices(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Query(filter): Query<InvoiceReportFilter>,
) -> Result<Json<InvoiceReport>, ApiError> {
    let user = actor(&state, &headers).await?;
    let report = state
        .facturas
        .report(user.id, &filter)
        .await
        .map_err(ApiError)?;
    state
        .reportes
        .record_report_event(
            user.id,
            "facturas",
            "report.view",
            &filter.date_from.to_string(),
            &filter.date_to.to_string(),
            report.totals.records,
        )
        .await
        .map_err(ApiError)?;
    Ok(Json(report))
}

async fn export_invoices(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Query(mut filter): Query<InvoiceReportFilter>,
) -> Result<Response, ApiError> {
    let user = actor(&state, &headers).await?;
    filter.bounds().map_err(ApiError)?;
    filter.page = Some(1);
    filter.page_size = Some(MAX_EXPORT_ROWS);
    let report = state
        .facturas
        .report(user.id, &filter)
        .await
        .map_err(ApiError)?;
    if report.has_more {
        return Ok(export_limit_problem());
    }
    let summary = [
        (
            "Registros",
            SpreadsheetCell::Number(report.totals.records.to_string()),
        ),
        (
            "Venta HNL",
            SpreadsheetCell::Number(report.totals.total.clone()),
        ),
        (
            "Pagado HNL",
            SpreadsheetCell::Number(report.totals.paid_total.clone()),
        ),
        (
            "Saldo HNL",
            SpreadsheetCell::Number(report.totals.balance.clone()),
        ),
    ];
    let rows = report
        .items
        .iter()
        .map(|row| {
            vec![
                SpreadsheetCell::Text(row.id.to_string()),
                SpreadsheetCell::Text(row.client_name.clone()),
                SpreadsheetCell::Text(row.order_number.clone()),
                SpreadsheetCell::Text(row.invoice_date.clone()),
                SpreadsheetCell::Text(row.status.clone()),
                SpreadsheetCell::Number(row.total.clone()),
                SpreadsheetCell::Number(row.paid_total.clone()),
                SpreadsheetCell::Number(row.balance.clone()),
            ]
        })
        .collect::<Vec<_>>();
    state
        .reportes
        .record_report_event(
            user.id,
            "facturas",
            "report.export",
            &filter.date_from.to_string(),
            &filter.date_to.to_string(),
            report.totals.records,
        )
        .await
        .map_err(ApiError)?;
    Ok(excel_response(
        "reporte-facturas.xls",
        workbook(
            &summary,
            &[
                "ID",
                "Cliente",
                "N.º orden",
                "Fecha",
                "Estado",
                "Venta HNL",
                "Pagado HNL",
                "Saldo HNL",
            ],
            &rows,
        ),
    ))
}

async fn expenses(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Query(filter): Query<ExpenseReportFilter>,
) -> Result<Json<ExpenseReport>, ApiError> {
    let user = actor(&state, &headers).await?;
    let report = state
        .gastos
        .report(user.id, &filter)
        .await
        .map_err(ApiError)?;
    state
        .reportes
        .record_report_event(
            user.id,
            "gastos",
            "report.view",
            &filter.date_from.to_string(),
            &filter.date_to.to_string(),
            report.totals.iter().map(|totals| totals.records).sum(),
        )
        .await
        .map_err(ApiError)?;
    Ok(Json(report))
}

async fn export_expenses(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Query(mut filter): Query<ExpenseReportFilter>,
) -> Result<Response, ApiError> {
    let user = actor(&state, &headers).await?;
    filter.bounds().map_err(ApiError)?;
    filter.page = Some(1);
    filter.page_size = Some(MAX_EXPORT_ROWS);
    let report = state
        .gastos
        .report(user.id, &filter)
        .await
        .map_err(ApiError)?;
    if report.has_more {
        return Ok(export_limit_problem());
    }
    let mut summary: Vec<(String, SpreadsheetCell)> = Vec::new();
    for totals in &report.totals {
        summary.push((
            format!("{} · Registros", totals.currency),
            SpreadsheetCell::Number(totals.records.to_string()),
        ));
        summary.push((
            format!("{} · Base gravada", totals.currency),
            SpreadsheetCell::Number(totals.taxable_amount.clone()),
        ));
        summary.push((
            format!("{} · Base exenta", totals.currency),
            SpreadsheetCell::Number(totals.exempt_amount.clone()),
        ));
        summary.push((
            format!("{} · ISV 15%", totals.currency),
            SpreadsheetCell::Number(totals.isv_15.clone()),
        ));
        summary.push((
            format!("{} · Total", totals.currency),
            SpreadsheetCell::Number(totals.total.clone()),
        ));
    }
    let rows = report
        .items
        .iter()
        .map(|row| {
            vec![
                SpreadsheetCell::Text(row.id.to_string()),
                SpreadsheetCell::Text(row.invoice_number.clone()),
                SpreadsheetCell::Text(row.expense_date.clone()),
                SpreadsheetCell::Text(row.provider.clone()),
                SpreadsheetCell::Text(row.currency.clone()),
                SpreadsheetCell::Number(row.taxable_amount.clone()),
                SpreadsheetCell::Number(row.exempt_amount.clone()),
                SpreadsheetCell::Number(row.isv_15.clone()),
                SpreadsheetCell::Number(row.total.clone()),
            ]
        })
        .collect::<Vec<_>>();
    state
        .reportes
        .record_report_event(
            user.id,
            "gastos",
            "report.export",
            &filter.date_from.to_string(),
            &filter.date_to.to_string(),
            report.totals.iter().map(|totals| totals.records).sum(),
        )
        .await
        .map_err(ApiError)?;
    let xml = workbook(
        &summary,
        &[
            "ID",
            "Factura",
            "Fecha",
            "Proveedor",
            "Moneda",
            "Base gravada",
            "Base exenta",
            "ISV 15%",
            "Total",
        ],
        &rows,
    );
    Ok(excel_response("reporte-gastos.xls", xml))
}

fn excel_response(filename: &'static str, xml: String) -> Response {
    let mut response = xml.into_response();
    response.headers_mut().insert(
        axum::http::header::CONTENT_TYPE,
        HeaderValue::from_static("application/vnd.ms-excel; charset=utf-8"),
    );
    response.headers_mut().insert(
        axum::http::header::CONTENT_DISPOSITION,
        HeaderValue::from_static(match filename {
            "reporte-ordenes.xls" => "attachment; filename=\"reporte-ordenes.xls\"",
            "reporte-facturas.xls" => "attachment; filename=\"reporte-facturas.xls\"",
            _ => "attachment; filename=\"reporte-gastos.xls\"",
        }),
    );
    response
}

fn export_limit_problem() -> Response {
    problem(
        StatusCode::PAYLOAD_TOO_LARGE,
        "Export Limit Reached",
        "El período contiene más de 1,000 registros. Reduce el rango de fechas para exportar.",
    )
}

#[cfg(test)]
mod tests;
