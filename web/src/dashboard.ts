export type DashboardPeriod = 'month' | 'year' | 'all';
export interface DashboardPoint { bucket: string; count: number; amount?: string }
interface DashboardBase<T, M> {
  window: { period: DashboardPeriod; from: string | null; until: string | null; today: string; timezone: string };
  as_of: string; metrics: M; series: DashboardPoint[]; chart_truncated: boolean;
  distribution: { label: string; count: number }[]; recent: T[];
}
export interface ProcessDashboardRow { proceso_id: string; expediente: string; entidad: string; resumen_corto: string; fecha_recepcion: string; marked: boolean }
export interface QuoteDashboardRow { id: number; quote_number: string | null; client_name: string; quote_date: string; status: string; total: string | null }
export type ProcessDashboard = DashboardBase<ProcessDashboardRow, { active: number; marked: number; entities: number; today: number }>;
export type QuoteDashboard = DashboardBase<QuoteDashboardRow, { total: number; amount: string; without_total: number }>;
export interface OrderDashboardRow { id: number; n_orders: string; client_name: string; order_date: string; status: string; total: string }
export interface InvoiceDashboardRow { id: number; order_number: string; client_name: string; invoice_date: string; status: string; total: string | null; balance: string | null }
export interface ExpenseDashboardRow { id: number; invoice_number: string; expense_date: string; provider: string; currency: string; total: string }
export type OrderDashboard = DashboardBase<OrderDashboardRow, { total: number; amount: string }>;
export type InvoiceDashboard = DashboardBase<InvoiceDashboardRow, { total: number; amount: string; without_total: number }>;
export type ExpenseDashboard = DashboardBase<ExpenseDashboardRow, { total: number; currencies: { currency: string; amount: string }[] }>;
export type ModuleDashboard = ProcessDashboard | QuoteDashboard | OrderDashboard | InvoiceDashboard | ExpenseDashboard;

// Keep exact decimal money supplied by the server, including amounts above JS's safe integer range.
export function cents(value: string | null | undefined): bigint | null {
  if (!value || !/^\d{1,65}(\.\d{1,2})?$/.test(value)) return null;
  const [whole, fraction = ''] = value.split('.');
  return BigInt(whole!) * 100n + BigInt(fraction.padEnd(2, '0'));
}
export function dashboardMoney(value: string | null | undefined): string {
  const amount = cents(value);
  return amount === null ? '—' : `L ${(amount / 100n).toString().replace(/\B(?=(\d{3})+(?!\d))/g, ',')}.${(amount % 100n).toString().padStart(2, '0')}`;
}
export function dashboardCurrency(value: string | null | undefined, currency: string): string {
  const amount = cents(value);
  return amount === null ? '—' : `${currency} ${(amount / 100n).toString().replace(/\B(?=(\d{3})+(?!\d))/g, ',')}.${(amount % 100n).toString().padStart(2, '0')}`;
}
export function chartPercent(value: bigint, maximum: bigint): number {
  return value <= 0n || maximum <= 0n ? 0 : Math.min(100, Math.max(0.5, Number(value * 10000n / maximum) / 100));
}
export function dashboardDate(value: string, time = false): string {
  const date = new Date(/^\d{4}-\d{2}-\d{2}$/.test(value) ? `${value}T12:00:00Z` : value);
  return Number.isNaN(date.getTime()) ? '—' : new Intl.DateTimeFormat('es-HN', { timeZone: 'America/Tegucigalpa', dateStyle: 'short', ...(time ? { timeStyle: 'short' as const } : {}) }).format(date);
}
