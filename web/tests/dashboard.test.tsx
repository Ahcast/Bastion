import { after, test } from 'node:test';
import assert from 'node:assert/strict';
import { Window } from 'happy-dom';
import { act } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import Dashboard, { areas, DashboardContent, DashboardChart } from '../src/pages/Dashboard.js';
import { cents, chartPercent, dashboardCurrency, dashboardMoney, dashboardDate } from '../src/dashboard.js';
import type { ExpenseDashboard, InvoiceDashboard, OrderDashboard, ProcessDashboard, QuoteDashboard } from '../src/dashboard.js';
import { CoreContext } from '../src/ui.js';
import { ApiClient } from '../src/api.js';
const window = new Window();
for (const name of ['document','HTMLElement','Event','MouseEvent','KeyboardEvent'] as const) Object.defineProperty(globalThis, name, { configurable: true, value: window[name] });
Object.assign(globalThis, { window, IS_REACT_ACT_ENVIRONMENT: true });
const { createRoot } = await import('react-dom/client');
after(() => window.happyDOM.abort());
const common = { window: { period: 'month' as const, from: '2026-10-01', until: '2026-11-01', today: '2026-10-23', timezone: 'America/Tegucigalpa' }, as_of: '2026-10-23T16:30:00Z', chart_truncated: false };
const processes: ProcessDashboard = { ...common, metrics: { active: 2, marked: 1, entities: 2, today: 1 }, distribution: [{ label: 'Hoy', count: 1 }, { label: 'Mañana', count: 1 }], series: [{ bucket: '2026-10-23', count: 1 }, { bucket: '2026-10-24', count: 1 }], recent: [{ proceso_id: 'p1', expediente: 'EXP-REAL', entidad: 'Entidad sintética', resumen_corto: 'Equipo de red', fecha_recepcion: '2026-10-23T22:00:00Z', marked: true }] };
const quotes: QuoteDashboard = { ...common, metrics: { total: 1, amount: '115.00', without_total: 0 }, distribution: [{ label: 'Borrador', count: 1 }], series: [{ bucket: '2026-10-23', count: 1, amount: '115.00' }], recent: [{ id: 1, quote_number: 'COT-000001', client_name: '<script>cliente</script>', quote_date: '2026-10-23', status: 'Borrador', total: '115.00' }] };
const orders: OrderDashboard = { ...common, metrics: { total: 1, amount: '200.00' }, distribution: [{ label: 'Pendiente', count: 1 }], series: [{ bucket: '2026-10-23', count: 1, amount: '200.00' }], recent: [{ id: 2, n_orders: 'OC-2', client_name: 'Cliente OC', order_date: '2026-10-23', status: 'Pendiente', total: '200.00' }] };
const invoices: InvoiceDashboard = { ...common, metrics: { total: 1, amount: '150.00', without_total: 0 }, distribution: [{ label: 'Borrador', count: 1 }], series: [{ bucket: '2026-10-23', count: 1, amount: '150.00' }], recent: [{ id: 3, order_number: 'FAC-3', client_name: 'Cliente factura', invoice_date: '2026-10-23', status: 'Borrador', total: '150.00', balance: '150.00' }] };
const expenses: ExpenseDashboard = { ...common, metrics: { total: 1, currencies: [{ currency: 'HNL', amount: '80.00' }] }, distribution: [{ label: 'Proveedor A', count: 1 }], series: [{ bucket: '2026-10-23', count: 1 }], recent: [{ id: 4, invoice_number: 'G-4', expense_date: '2026-10-23', provider: 'Proveedor A', currency: 'HNL', total: '80.00' }] };
test('dashboard money stays exact and Honduras dates never shift date-only documents', () => {
  assert.equal(cents('0.01'), 1n); assert.equal(cents('1.5'), 150n);
  assert.equal(dashboardMoney('9007199254740993.01'), 'L 9,007,199,254,740,993.01');
  assert.equal(dashboardMoney('0.00'), 'L 0.00'); assert.equal(dashboardMoney(null), '—');
  assert.equal(dashboardCurrency('9007199254740993.01', 'USD'), 'USD 9,007,199,254,740,993.01');
  assert.equal(cents('1e6'), null); assert.equal(cents('-1.00'), null);
  assert.equal(chartPercent(0n, 10n), 0); assert.equal(chartPercent(10n, 10n), 100);
  assert.equal(chartPercent(1n, 99999999999999999999n), .5);
  assert.equal(dashboardDate('2026-10-23'), dashboardDate('2026-10-23T12:00:00Z'));
});
test('real results, empty results, missing prices and escaped customer names are distinct', () => {
  const markup = renderToStaticMarkup(<DashboardContent area={areas[1]!} data={quotes} />);
  assert.ok(markup.includes('L 115.00')); assert.ok(markup.includes('&lt;script&gt;cliente&lt;/script&gt;')); assert.ok(!markup.includes('<script>'));
  const empty = renderToStaticMarkup(<DashboardContent area={areas[1]!} data={{ ...quotes, metrics: { total: 1, amount: '0.00', without_total: 1 }, recent: [], series: [] }} />);
  assert.ok(empty.includes('sin total calculado')); assert.ok(empty.includes('Sin registros en este período')); assert.ok(empty.includes('L 0.00'));
  const chart = renderToStaticMarkup(<DashboardChart title="Prueba" monetary={false} points={[{ bucket: '2026-10-23', count: 0 }]} />);
  assert.ok(chart.includes('Los valores registrados son cero'));
});
async function mount(allowed: string[] = ['procesos','cotizaciones'], failProcess = false) {
  const calls: { path: string; period: string | null; signal: AbortSignal | null | undefined }[] = [];
  const api = new ApiClient('https://example.test', async (input, options) => {
    const url = new URL(String(input)); calls.push({ path: url.pathname, period: url.searchParams.get('period'), signal: options?.signal });
    if (url.pathname.includes('/procesos/')) return failProcess ? Response.json({ detail: 'Resumen temporalmente no disponible' }, { status: 503 }) : Response.json(processes);
    if (url.pathname.includes('/cotizaciones/')) return Response.json(quotes);
    if (url.pathname.includes('/ordenes-compra/')) return Response.json(orders);
    if (url.pathname.includes('/facturas/')) return Response.json(invoices);
    return Response.json(expenses);
  });
  const container = document.createElement('div'); document.body.append(container); const root = createRoot(container); let opened = '';
  await act(async () => root.render(<CoreContext.Provider value={{ api, access: { is_system_administrator: false, modules: ['procesos','cotizaciones','ordenes_compra','facturas','gastos'].filter(module_id => allowed.includes(module_id)).map(module_id => ({ module_id, display_name: module_id })), permissions: allowed.map(module_id => ({ module_id, scope_key: module_id === 'procesos' ? 'consulta' : module_id === 'gastos' ? 'registros' : 'documentos', capability: 'ver' })) }, notify: () => {} }}><Dashboard openModule={page => { opened = page; }} /></CoreContext.Provider>));
  const click = async (element: Element | null) => { assert.ok(element); await act(async () => { element.dispatchEvent(new MouseEvent('click', { bubbles: true })); }); };
  return { calls, container, click, opened: () => opened, dispose: async () => { await act(async () => root.unmount()); container.remove(); } };
}
test('selection replaces charts and rows, range changes cancel old loads and refresh requests both summaries', async () => {
  const app = await mount(); try {
    assert.equal(app.calls.length, 2); assert.ok(app.container.textContent?.includes('EXP-REAL')); assert.ok(!app.container.textContent?.includes('COT-000001'));
    await app.click(app.container.querySelectorAll('[role="tab"]')[1]!);
    assert.ok(app.container.textContent?.includes('COT-000001')); assert.ok(!app.container.textContent?.includes('EXP-REAL'));
    await app.click(Array.from(app.container.querySelectorAll('button')).find(button => button.textContent?.startsWith('Ver cotizaciones'))!);
    assert.equal(app.opened(), 'cotizaciones');
    const select = app.container.querySelector('select')!;
    await act(async () => { select.value = 'all'; select.dispatchEvent(new Event('change', { bubbles: true })); });
    assert.equal(app.calls.length, 4); assert.equal(app.calls[0]!.signal?.aborted, true); assert.equal(app.calls[2]!.period, 'all');
    await app.click(Array.from(app.container.querySelectorAll('button')).find(button => button.textContent === 'Actualizar')!);
    assert.equal(app.calls.length, 6);
    await app.click(app.container.querySelectorAll('[role="tab"]')[4]!);
    assert.ok(app.container.textContent?.includes('Este módulo no está disponible')); assert.ok(!app.container.textContent?.includes('COT-000001'));
  } finally { await app.dispose(); }
});
test('las cinco áreas consumen sus propios resúmenes y muestran sus registros', async () => {
  const app = await mount(['procesos','cotizaciones','ordenes_compra','facturas','gastos']); try {
    assert.equal(app.calls.length, 5);
    const tabs = app.container.querySelectorAll('[role="tab"]');
    for (const [index, value] of [[1,'COT-000001'],[2,'OC-2'],[3,'FAC-3'],[4,'G-4']] as const) {
      await app.click(tabs[index]!);
      assert.ok(app.container.textContent?.includes(value));
    }
    assert.ok(app.container.textContent?.includes('Total HNL'));
  } finally { await app.dispose(); }
});
test('unauthorized modules make no requests and partial failure preserves the other module', async () => {
  const restricted = await mount(['cotizaciones']); try {
    assert.equal(restricted.calls.length, 1); assert.ok(restricted.calls[0]!.path.includes('cotizaciones'));
    await restricted.click(restricted.container.querySelectorAll('[role="tab"]')[0]!);
    assert.ok(restricted.container.textContent?.includes('Este módulo no está disponible')); assert.ok(!restricted.container.textContent?.includes('EXP-REAL'));
  } finally { await restricted.dispose(); }
  const partial = await mount(undefined, true); try {
    assert.ok(partial.container.querySelector('[role="alert"]')); assert.ok(partial.container.textContent?.includes('Reintentar'));
    await partial.click(partial.container.querySelectorAll('[role="tab"]')[1]!);
    assert.ok(partial.container.textContent?.includes('COT-000001')); assert.equal(partial.container.querySelector('[role="alert"]'), null);
  } finally { await partial.dispose(); }
});
