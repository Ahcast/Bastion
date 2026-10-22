export const quoteStatuses = ['Borrador', 'Enviada', 'Aprobada', 'Rechazada', 'Convertido'] as const;
export type QuoteStatus = typeof quoteStatuses[number];
export type DiscountType = 'amount' | 'percent';
export interface QuoteItem { description: string; quantity: string; unit_price: string; discount_type: DiscountType; discount_value: string; tax_type: 'none' | 'isv15' | 'isv18' }
export interface QuoteInput { client_name: string; order_number: string; quote_date: string; valid_days: number; status: QuoteStatus; purchase_order_exempt_number: string; registration_certificate_number: string; sag_registration_number: string; contact_email: string; prepared_by_name: string; notes: string; terms: string; discount_type: DiscountType; discount_value: string; items: QuoteItem[] }
export interface QuoteDraft { id: number; version: number; status: QuoteStatus; currency: 'HNL'; data: QuoteInput; quote_number: string | null; calculation: QuoteCalculation }
export interface QuoteSummary { id: number; version: number; client_name: string; order_number: string; quote_date: string; item_count: number; status: QuoteStatus; quote_number: string | null; valid_until: string | null; total: string | null }
export interface QuotePage { items: QuoteSummary[]; page: number; page_size: number; has_more: boolean }
export const newQuoteItem = (): QuoteItem => ({ description: '', quantity: '1', unit_price: '0', discount_type: 'amount', discount_value: '0', tax_type: 'isv15' });
export function newQuote(now = new Date()): QuoteInput {
  const date = new Intl.DateTimeFormat('en-CA', { timeZone: 'America/Tegucigalpa', year: 'numeric', month: '2-digit', day: '2-digit' }).format(now);
  return { client_name: '', order_number: '', quote_date: date, valid_days: 30, status: 'Borrador', purchase_order_exempt_number: '', registration_certificate_number: '', sag_registration_number: '', contact_email: '', prepared_by_name: '', notes: '', terms: 'Tiempo de entrega: ', discount_type: 'amount', discount_value: '0', items: [newQuoteItem()] };
}

export interface QuoteTotals { subtotal: string; importe_exonerado: string; importe_exento: string; importe_gravado_15: string; importe_gravado_18: string; descuentos_rebajas: string; isv_15: string; isv_18: string; total: string }
export interface CalculatedItem { subtotal: string; unit_discount: string; item_discount_total: string; global_line_discount: string; discount_amount: string; taxable_base: string; tax_type: QuoteItem['tax_type']; tax_amount: string; total: string }
export interface QuoteCalculation { data: QuoteInput; valid_until: string; items: CalculatedItem[]; totals: QuoteTotals }
