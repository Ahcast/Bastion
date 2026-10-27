import { useState } from 'react';
import type { FormEvent } from 'react';
import { message } from '../api.js';
import { reportCan } from '../navigation.js';
import { dashboardDate } from '../dashboard.js';
import { Heading, Loading, Notice, Table, useCore, useLoad } from '../ui.js';
import type { Access } from '../types.js';

type ReportKind = 'ordenes' | 'facturas' | 'gastos';
type ReportTab = { id: ReportKind; label: string };
const TABS: ReportTab[] = [
  { id: 'ordenes', label: 'Órdenes de compra' },
  { id: 'facturas', label: 'Facturas' },
  { id: 'gastos', label: 'Gastos' },
];
interface ReportResponse { items: ReportRow[]; totals: ReportTotals | ExpenseTotals[]; page: number; page_size: number; has_more: boolean }
type ReportRow = OrderRow | InvoiceRow | ExpenseRow;
type ReportTotals = OrderTotals | InvoiceTotals;
interface OrderRow { id: number; n_orders: string; n_bill: string; client_name: string; order_date: string; status: string; subtotal: string; isv: string; total: string; products_investment: string; other_expenses: string; investment_total: string; profit: string }
interface OrderTotals { records: number; subtotal: string; isv: string; total: string; products_investment: string; other_expenses: string; investment_total: string; profit: string }
interface InvoiceRow { id: number; client_name: string; order_number: string; invoice_date: string; status: string; total: string; paid_total: string; balance: string }
interface InvoiceTotals { records: number; total: string; paid_total: string; balance: string }
interface ExpenseRow { id: number; invoice_number: string; expense_date: string; provider: string; currency: string; taxable_amount: string; exempt_amount: string; isv_15: string; total: string }
interface ExpenseTotals { currency: string; records: number; taxable_amount: string; exempt_amount: string; isv_15: string; total: string }

function today(): string {
  const date = new Date();
  return `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, '0')}-${String(date.getDate()).padStart(2, '0')}`;
}
function firstOfMonth(value: string) { return `${value.slice(0, 7)}-01`; }
function queryFor(kind: ReportKind, from: string, to: string, page: number, pageSize: number, status: string, search: string) {
  const query = new URLSearchParams({ date_from: from, date_to: to, page: String(page), page_size: String(pageSize) });
  if (kind === 'ordenes' && status) query.set('status', status);
  if (search) query.set('search', search);
  return query.toString();
}
function numberOfDays(from: string, to: string) {
  return (Date.parse(`${to}T00:00:00Z`) - Date.parse(`${from}T00:00:00Z`)) / 86_400_000;
}
function money(value: string, currency = 'HNL') {
  if (!/^-?\d{1,30}(\.\d{1,2})?$/.test(value)) return '—';
  const negative = value.startsWith('-');
  const [whole, fraction = ''] = (negative ? value.slice(1) : value).split('.');
  const amount = BigInt(whole!) * 100n + BigInt(fraction.padEnd(2, '0'));
  return `${currency} ${negative ? '-' : ''}${(amount / 100n).toString().replace(/\B(?=(\d{3})+(?!\d))/g, ',')}.${(amount % 100n).toString().padStart(2, '0')}`;
}
function stats(entries: [string, string][]) {
  return <div className="report-stats">{entries.map(([label, value]) => <div key={label}><span>{label}</span><strong>{value}</strong></div>)}</div>;
}
function availableTabs(access: Access) { return TABS.filter(tab => reportCan(access, tab.id)); }

export default function Reportes() {
  const { api, access } = useCore();
  const tabs = availableTabs(access);
  const [kind, setKind] = useState<ReportKind>(() => tabs[0]?.id ?? 'ordenes');
  const initialToday = today();
  const [fromDraft, setFromDraft] = useState(firstOfMonth(initialToday));
  const [toDraft, setToDraft] = useState(initialToday);
  const [from, setFrom] = useState(firstOfMonth(initialToday));
  const [to, setTo] = useState(initialToday);
  const [statusDraft, setStatusDraft] = useState('');
  const [status, setStatus] = useState('');
  const [searchDraft, setSearchDraft] = useState('');
  const [search, setSearch] = useState('');
  const [page, setPage] = useState(1);
  const [filterError, setFilterError] = useState('');
  const [exportError, setExportError] = useState('');
  const [exporting, setExporting] = useState(false);
  const tab = tabs.find(item => item.id === kind) ?? tabs[0];
  const requestKey = tab ? queryFor(tab.id, from, to, page, 25, status, search) : '';
  const visibleKind = tab?.id ?? kind;
  const result = useLoad<{ kind: ReportKind; query: string; report: ReportResponse }>(signal => {
    if (!tab) return Promise.resolve({ kind, query: requestKey, report: { items: [], totals: [], page: 1, page_size: 25, has_more: false } });
    const query = queryFor(tab.id, from, to, page, 25, status, search);
    return api.request<ReportResponse>(`/api/v1/reportes/${tab.id}?${query}`, { signal }).then(report => ({ kind: tab.id, query: requestKey, report }));
  }, [api, tab?.id, from, to, status, search, page]);

  const loaded = result.data;
  const data = loaded && loaded.kind === tab?.id && loaded.query === requestKey ? loaded.report : null;
  if (!tabs.length) return <Notice>No tienes acceso a reportes de los dominios disponibles.</Notice>;

  const apply = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    setFilterError('');
    if (!fromDraft || !toDraft || fromDraft > toDraft) { setFilterError('Indica un rango de fechas válido.'); return; }
    if (numberOfDays(fromDraft, toDraft) > 365) { setFilterError('El rango máximo de consulta es de 366 días.'); return; }
    if (searchDraft.length > 200) { setFilterError('La búsqueda admite hasta 200 caracteres.'); return; }
    setFrom(fromDraft); setTo(toDraft); setStatus(statusDraft); setSearch(searchDraft.trim()); setPage(1);
  };
  const exportReport = async () => {
    if (!tab || exporting) return;
    setExportError(''); setExporting(true);
    try {
      const query = queryFor(tab.id, from, to, 1, 1000, status, search);
      const blob = await api.download(`/api/v1/reportes/${tab.id}/export?${query}`);
      const url = URL.createObjectURL(blob), link = document.createElement('a');
      link.href = url; link.download = `reporte-${tab.id}.xls`; document.body.append(link); link.click(); link.remove();
      setTimeout(() => URL.revokeObjectURL(url), 10_000);
    } catch (error) {
      setExportError(message(error));
    } finally { setExporting(false); }
  };
  const selectTab = (next: ReportKind) => {
    setKind(next); setPage(1); setStatus(''); setStatusDraft(''); setExportError('');
  };

  return <><Heading title="Reportes" description="Consulta órdenes, borradores de factura y gastos por fecha." action={<button disabled={!data || exporting} onClick={() => void exportReport()}>{exporting ? 'Preparando Excel…' : 'Exportar Excel'}</button>} />
    <nav className="report-tabs" aria-label="Tipo de reporte">{tabs.map(item => <button key={item.id} aria-pressed={visibleKind === item.id} onClick={() => selectTab(item.id)}>{item.label}</button>)}</nav>
    <form className="card report-filters" onSubmit={apply}>
      <label>Fecha inicio<input type="date" required value={fromDraft} onChange={event => setFromDraft(event.target.value)} /></label>
      <label>Fecha fin<input type="date" required value={toDraft} onChange={event => setToDraft(event.target.value)} /></label>
      <label>Búsqueda<input type="search" maxLength={200} value={searchDraft} onChange={event => setSearchDraft(event.target.value)} placeholder="Cliente, orden, factura o proveedor" /></label>
      {visibleKind === 'ordenes' && <label>Estado<select value={statusDraft} onChange={event => setStatusDraft(event.target.value)}><option value="">Todos</option><option value="Pendiente">Pendiente</option><option value="Listo">Listo</option><option value="Entregado">Entregado</option></select></label>}
      <button className="primary" type="submit">Consultar</button>
    </form>
    <Notice>{filterError || result.error || exportError}</Notice>
    <p className="report-note">Rango máximo: 366 días. La exportación Excel admite hasta 1,000 registros por archivo. Los totales de Gastos se presentan separados por moneda.</p>
    {visibleKind === 'facturas' && <p className="report-note">En esta etapa se reportan borradores; los pagos todavía no están habilitados.</p>}
    {data && <>
      {visibleKind === 'ordenes' && <OrderSummary data={data.totals as OrderTotals} />}
      {visibleKind === 'facturas' && <InvoiceSummary data={data.totals as InvoiceTotals} />}
      {visibleKind === 'gastos' && <ExpenseSummary data={data.totals as ExpenseTotals[]} />}
      {data.items.length ? <ReportTable kind={visibleKind} rows={data.items} /> : <div className="empty report-empty">No hay registros en el período seleccionado.</div>}
      <nav className="report-pagination" aria-label="Paginación del reporte"><button disabled={page === 1} onClick={() => setPage(value => value - 1)}>Anterior</button><span role="status">Página {data.page} · {visibleKind === 'gastos' ? (data.totals as ExpenseTotals[]).reduce((sum, item) => sum + item.records, 0) : (data.totals as ReportTotals).records} registros</span><button disabled={!data.has_more} onClick={() => setPage(value => value + 1)}>Siguiente</button></nav>
    </>}
    {!data && !result.error && <Loading />}
  </>;
}

function OrderSummary({ data }: { data: OrderTotals }) {
  return stats([
    ['Órdenes', String(data.records)], ['Venta total', money(data.total)], ['ISV', money(data.isv)],
    ['Inversión productos', money(data.products_investment)], ['Otros gastos', money(data.other_expenses)],
    ['Inversión total', money(data.investment_total)], ['Utilidad', money(data.profit)],
  ]);
}
function InvoiceSummary({ data }: { data: InvoiceTotals }) {
  return stats([['Borradores', String(data.records)], ['Venta', money(data.total)], ['Pagado', money(data.paid_total)], ['Saldo', money(data.balance)]]);
}
function ExpenseSummary({ data }: { data: ExpenseTotals[] }) {
  return <div className="report-stats">{data.length ? data.map(item => <div className="report-currency" key={item.currency}><span>{item.currency} · {item.records} registros</span><strong>{money(item.total, item.currency)}</strong><small>Gravado {money(item.taxable_amount, item.currency)} · Exento {money(item.exempt_amount, item.currency)} · ISV {money(item.isv_15, item.currency)}</small></div>) : <div><span>Gastos</span><strong>0</strong></div>}</div>;
}
function ReportTable({ kind, rows }: { kind: ReportKind; rows: ReportRow[] }) {
  if (kind === 'ordenes') {
    return <Table columns={['N.º orden', 'N.º factura', 'Cliente', 'Fecha', 'Estado', 'Subtotal HNL', 'ISV HNL', 'Total HNL', 'Inversión productos HNL', 'Otros gastos HNL', 'Inversión total HNL', 'Utilidad HNL']} rows={(rows as OrderRow[]).map(row => [row.n_orders, row.n_bill || '—', row.client_name, dashboardDate(row.order_date), row.status, money(row.subtotal), money(row.isv), money(row.total), money(row.products_investment), money(row.other_expenses), money(row.investment_total), money(row.profit)])} />;
  }
  if (kind === 'facturas') {
    return <Table columns={['ID borrador', 'Cliente', 'N.º orden', 'Fecha', 'Estado', 'Venta HNL', 'Pagado HNL', 'Saldo HNL']} rows={(rows as InvoiceRow[]).map(row => [row.id, row.client_name, row.order_number, dashboardDate(row.invoice_date), row.status, money(row.total), money(row.paid_total), money(row.balance)])} />;
  }
  return <Table columns={['Factura', 'Fecha', 'Proveedor', 'Moneda', 'Base gravada', 'Base exenta', 'ISV 15%', 'Total']} rows={(rows as ExpenseRow[]).map(row => [row.invoice_number, dashboardDate(row.expense_date), row.provider, row.currency, money(row.taxable_amount, row.currency), money(row.exempt_amount, row.currency), money(row.isv_15, row.currency), money(row.total, row.currency)])} />;
}
