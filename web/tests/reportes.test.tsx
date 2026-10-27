import { after, test } from 'node:test';
import assert from 'node:assert/strict';
import { ApiClient, ApiError } from '../src/api.js';
import { mainNavigation, reportCan, settingsSections } from '../src/navigation.js';
import type { Access } from '../src/types.js';
import { Window } from 'happy-dom';
import { CoreContext } from '../src/ui.js';
import Reportes from '../src/pages/Reportes.js';

test('reports require both active modules and the matching domain read scopes', () => {
  const access: Access = {
    is_system_administrator: false,
    modules: [{ module_id: 'reportes', display_name: 'Reportes' }, { module_id: 'gastos', display_name: 'Gastos' }],
    permissions: [{ module_id: 'reportes', scope_key: 'gastos', capability: 'ver' }, { module_id: 'gastos', scope_key: 'registros', capability: 'ver' }],
  };
  assert.equal(reportCan(access, 'gastos'), true);
  assert.equal(reportCan(access, 'ordenes'), false);
  assert.equal(mainNavigation(access).some(page => page.id === 'reportes'), true);
  assert.equal(settingsSections(access).some(page => page.id === 'reportes'), true);
  for (const context of [
    { ...access, modules: access.modules.slice(0,1) },
    { ...access, modules: access.modules.slice(1) },
    { ...access, permissions: access.permissions.slice(0,1) },
    { ...access, permissions: access.permissions.slice(1) },
  ]) {
    assert.equal(reportCan(context, 'gastos'), false);
    assert.equal(mainNavigation(context).some(page => page.id === 'reportes'), false);
  }
});

test('Excel downloads preserve cookies, browser fetch context, errors and authentication hooks', async () => {
  let expired = 0;
  const transport: typeof fetch = async function (this: unknown, _url, init) {
    assert.equal(this, undefined);
    assert.equal(init?.credentials, 'include');
    assert.equal(init?.cache, 'no-store');
    return new Response('<Workbook/>', { headers: { 'Content-Type': 'application/vnd.ms-excel' } });
  };
  const api = new ApiClient('https://example.test', transport);
  assert.equal(await (await api.download('/api/v1/reportes/gastos/export')).text(), '<Workbook/>');
  const denied = new ApiClient('https://example.test', async () => Response.json({ title: 'Export Limit Reached', detail: 'Reduce el período.' }, { status: 413 }));
  await assert.rejects(denied.download('/export'), error => error instanceof ApiError && error.status === 413 && error.message === 'Reduce el período.');
  const stale = new ApiClient('https://example.test', async () => new Response(null, { status: 401 }));
  stale.hooks.unauthorized = () => { expired++; };
  await assert.rejects(stale.download('/export'), error => error instanceof ApiError && error.status === 401);
  assert.equal(expired, 1);
});

const window = new Window();
for (const name of ['document', 'HTMLElement', 'HTMLInputElement', 'Event', 'MouseEvent', 'navigator'] as const) {
  Object.defineProperty(globalThis, name, { configurable: true, value: window[name] });
}
Object.assign(globalThis, { window, IS_REACT_ACT_ENVIRONMENT: true });
const { act } = await import('react');
const { createRoot } = await import('react-dom/client');
after(() => window.happyDOM.abort());

test('switching report tabs waits for its own response and keeps decimal totals exact', async () => {
  const access: Access = {
    is_system_administrator: true,
    modules: ['reportes', 'ordenes_compra', 'gastos'].map(module_id => ({ module_id, display_name: module_id })),
    permissions: [],
  };
  let resolveExpense: ((response: Response) => void) | undefined;
  const api = new ApiClient('https://example.test', async url => {
    if (String(url).includes('/gastos?')) return new Promise<Response>(resolve => { resolveExpense = resolve; });
    return Response.json({ items: [], page: 1, page_size: 25, has_more: false, totals: {
      records: 1, total: '123456789012345.67', subtotal: '0.00', isv: '0.00',
      products_investment: '0.00', other_expenses: '0.00', investment_total: '0.00', profit: '0.00',
    } });
  });
  const host = document.createElement('div');
  document.body.append(host);
  const root = createRoot(host);
  await act(async () => {
    root.render(<CoreContext.Provider value={{ api, access, notify: () => {} }}><Reportes /></CoreContext.Provider>);
    await new Promise(resolve => setTimeout(resolve, 20));
  });
  assert.ok(host.textContent?.includes('HNL 123,456,789,012,345.67'));
  const tab = [...host.querySelectorAll<HTMLButtonElement>('.report-tabs button')].find(button => button.textContent === 'Gastos')!;
  await act(async () => { tab.click(); });
  assert.equal([...host.querySelectorAll<HTMLButtonElement>('button')].find(button => button.textContent === 'Exportar Excel')?.disabled, true);
  assert.ok(!host.textContent?.includes('123,456,789,012,345.67'));
  assert.ok(resolveExpense);
  await act(async () => {
    resolveExpense!(Response.json({ items: [], page: 1, page_size: 25, has_more: false, totals: [
      { currency: 'HNL', records: 1, taxable_amount: '100.01', exempt_amount: '20.01', isv_15: '15.00', total: '135.02' },
    ] }));
  });
  assert.ok(host.textContent?.includes('HNL 135.02'));
  await act(async () => root.unmount());
  host.remove();
});
