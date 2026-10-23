import type { CalculatedItem, DiscountType, QuoteTotals } from './quotes.js';

export interface InvoiceItem { description: string; quantity: string; unit_price: string; cost_price: string; discount_type: DiscountType; discount_value: string; tax_type: 'none' | 'isv15' | 'isv18' }
export interface InvoiceExpense { description: string; amount: string }
export interface InvoiceInput { id_client?: number | null; client_name: string; client_rtn?: string; client_email?: string; order_number: string; invoice_date: string; payment_term_type: 'contado' | 'credito'; credit_days: number; discount_type: DiscountType; discount_value: string; purchase_order_exempt_number: string; registration_certificate_number: string; sag_registration_number: string; notes: string; public_notes: string; items: InvoiceItem[]; expenses: InvoiceExpense[] }
export interface InvoiceCalculation { data: InvoiceInput; items: CalculatedItem[]; totals: QuoteTotals; product_investment: string; other_expenses: string; investment_total: string; profit: string; paid_total: string; balance: string }
export interface InvoiceDraft { id: number; version: number; status: 'Borrador'; currency: 'HNL'; invoice_number: null; data: InvoiceInput; calculation: InvoiceCalculation }
export interface InvoiceSummary { id: number; version: number; client_name: string; order_number: string; invoice_date: string; status: 'Borrador'; total: string; balance: string; item_count: number }
export interface InvoicePage { items: InvoiceSummary[]; page: number; page_size: number; has_more: boolean }
export const newInvoiceItem = (): InvoiceItem => ({ description: '', quantity: '1', unit_price: '0', cost_price: '0', discount_type: 'amount', discount_value: '0', tax_type: 'isv15' });
export function newInvoice(now = new Date()): InvoiceInput {
  const invoice_date = new Intl.DateTimeFormat('en-CA', { timeZone: 'America/Tegucigalpa', year: 'numeric', month: '2-digit', day: '2-digit' }).format(now);
  return { id_client: null, client_name: '', client_rtn: '', client_email: '', order_number: '', invoice_date, payment_term_type: 'contado', credit_days: 0, discount_type: 'amount', discount_value: '0', purchase_order_exempt_number: '', registration_certificate_number: '', sag_registration_number: '', notes: '', public_notes: '', items: [newInvoiceItem()], expenses: [] };
}
