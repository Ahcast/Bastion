import type { QuoteOrigin } from './conversions.js';
export interface OrderProduct { line_id?: string; description: string; cantidad: number; sale_price: string; purchase_price: string; tracking: string; isv: boolean; status: 'Pendiente' | 'Comprado'; coments: string }
export interface OrderExpense { description: string; amount: string }
export interface OrderInput { n_orders: string; n_bill: string; order_date: string; received_date: string | null; client_name: string; client_rtn: string; id_destination: number | null; comments: string; products: OrderProduct[]; expenses: OrderExpense[] }
export interface OrderTotals { subtotal: string; subtotal_exonerado: string; tax: string; total: string; products_investment: string; other_expenses: string; investment_total: string; profit: string }
export interface OrderCalculation { data: OrderInput; status: 'Pendiente' | 'Listo' | 'Entregado'; totals: OrderTotals }
export interface PurchaseOrder { source_quote?: QuoteOrigin | null; id: number; version: number; status: OrderCalculation['status']; currency: 'HNL'; data: OrderInput; calculation: OrderCalculation }
export interface OrderSummary { id: number; version: number; n_orders: string; n_bill: string; client_name: string; order_date: string; status: OrderCalculation['status']; total: string }
export interface OrderPage { items: OrderSummary[]; page: number; page_size: number; has_more: boolean }
export interface Destination { id: number; version: number; destination: string }
export const newProduct = (): OrderProduct => ({ description: '', cantidad: 1, sale_price: '0', purchase_price: '0', tracking: '', isv: false, status: 'Pendiente', coments: '' });
export function newOrder(now = new Date()): OrderInput {
  const order_date = new Intl.DateTimeFormat('en-CA', { timeZone: 'America/Tegucigalpa', year: 'numeric', month: '2-digit', day: '2-digit' }).format(now);
  return { n_orders: '', n_bill: '', order_date, received_date: null, client_name: '', client_rtn: '', id_destination: null, comments: '', products: [newProduct()], expenses: [] };
}
