import { after, test } from 'node:test';
import assert from 'node:assert/strict';
import { Window } from 'happy-dom';
import { renderToStaticMarkup } from 'react-dom/server';
import { ApiClient } from '../src/api.js';
import { CoreContext } from '../src/ui.js';
import { NavigationGuardProvider } from '../src/navigationGuard.js';
import Gastos from '../src/pages/Gastos.js';
import { mainNavigation, settingsSections } from '../src/navigation.js';
import { emptyExpense, expenseInput } from '../src/gastos.js';
import type { Access } from '../src/types.js';

const access: Access = { is_system_administrator: false, modules: [{ module_id: 'gastos', display_name: 'Gastos' }], permissions: [{ module_id: 'gastos', scope_key: 'registros', capability: 'ver' }] };
const expense = { ...emptyExpense('2026-10-24'), invoice_number: 'F-001', provider: '<script>proveedor</script>', taxable_amount: '100.01', exempt_amount: '20.01', id: 7, version: 3, isv_15: '15.00', total: '135.02' };
const summary = { count: 100, taxable_amount: '100.01', exempt_amount: '20.01', isv_15: '15.00', total: '135.02' };

test('expense inputs exclude identity, version and calculated fields on every save', () => {
  assert.deepEqual(Object.keys(expenseInput(expense)).sort(), Object.keys(emptyExpense('2026-10-24')).sort());
  assert.equal('total' in expenseInput(expense), false);
});
test('expense navigation and writes require availability and separate permissions', () => {
  for (const [active, edit] of [[true, true], [true, false], [false, true]]) {
    const context = { ...access, modules: active ? access.modules : [], permissions: [...access.permissions, ...(edit ? [{ module_id: 'gastos', scope_key: 'registros', capability: 'editar' }] : [])] };
    const html = renderToStaticMarkup(<NavigationGuardProvider><CoreContext.Provider value={{ api: new ApiClient('https://example.test'), access: context, notify: () => {} }}><Gastos /></CoreContext.Provider></NavigationGuardProvider>);
    assert.equal(html.includes('>Nuevo gasto<'), active && edit);
    assert.equal(html.includes('No tienes acceso'), !active);
    assert.equal(mainNavigation(context).some(page => page.id === 'gastos'), active);
    assert.equal(settingsSections(context).some(page => page.id === 'gastos'), active);
  }
});
const window = new Window();
for (const name of ['document', 'HTMLElement', 'HTMLInputElement', 'HTMLTextAreaElement', 'FormData', 'Event', 'MouseEvent', 'navigator'] as const) Object.defineProperty(globalThis, name, { configurable: true, value: window[name] });
Object.assign(globalThis, { window, IS_REACT_ACT_ENVIRONMENT: true });
const { act } = await import('react');
const { createRoot } = await import('react-dom/client');
after(() => window.happyDOM.abort());

test('expense edits preserve drafts on 412, send six input fields and require delete confirmation', async () => {
  const calls: { url: URL; init: RequestInit }[] = [], notices: string[] = [];
  let stale = true, removed = false, puts = 0;
  const api = new ApiClient('https://example.test', async (url, init = {}) => {
    const parsed = new URL(String(url)); calls.push({ url: parsed, init });
    if (parsed.pathname.endsWith('/calculo')) return Response.json({ data: expenseInput(expense), isv_15: expense.isv_15, total: expense.total });
    if (init.method === 'PUT') {
      puts++; assert.equal(new Headers(init.headers).get('If-Match'), '\"3\"');
      assert.deepEqual(Object.keys(JSON.parse(String(init.body))).sort(), Object.keys(emptyExpense('2026-10-24')).sort());
      if (stale) return Response.json({ detail: 'El gasto cambió.' }, { status: 412 });
      return Response.json({ ...expense, version: 4 });
    }
    if (init.method === 'DELETE') { removed = true; return new Response(null, { status: 204 }); }
    if (parsed.pathname.endsWith('/7')) return Response.json(expense);
    return Response.json({ items: removed ? [] : [expense], page: Number(parsed.searchParams.get('page')), page_size: 20, has_more: parsed.searchParams.get('page') === '1', summary: removed ? { ...summary, count: 0 } : summary });
  });
  const container = document.createElement('div'); document.body.append(container); const root = createRoot(container);
  const context = { ...access, permissions: [...access.permissions, ...['editar', 'eliminar'].map(capability => ({ module_id: 'gastos', scope_key: 'registros', capability }))] };
  const click = async (text: string) => { const button = [...container.querySelectorAll('button')].find(b => b.textContent === text); assert.ok(button, text); await act(async () => button.click()); };
  try {
    await act(async () => root.render(<NavigationGuardProvider><CoreContext.Provider value={{ api, access: context, notify: text => notices.push(text) }}><Gastos /></CoreContext.Provider></NavigationGuardProvider>));
    assert.equal(container.querySelector('script'), null);
    assert.ok(container.querySelector('.expense-summary')?.textContent?.includes('100'));
    await click('Editar');
    assert.equal(container.querySelector<HTMLInputElement>('[name="invoice_number"]')?.maxLength, 50);
    await act(async () => container.querySelector('.expense-form')!.dispatchEvent(new Event('submit', { bubbles: true, cancelable: true })));
    assert.ok(container.textContent?.includes('Tus cambios se conservan'));
    assert.equal(container.querySelector<HTMLInputElement>('[name="provider"]')?.value, expense.provider);
    assert.ok(container.querySelector<HTMLFieldSetElement>('.expense-fields')?.disabled);
    await click('Abrir versión actual'); stale = false;
    await act(async () => container.querySelector('.expense-form')!.dispatchEvent(new Event('submit', { bubbles: true, cancelable: true })));
    assert.equal(puts, 2); assert.ok(notices.includes('Gasto guardado.'));
    await click('Cerrar');
    await click('Siguiente');
    assert.equal(calls.at(-1)?.url.searchParams.get('page'), '2');
    assert.equal(calls.at(-1)?.url.searchParams.get('page_size'), '20');
    Object.defineProperty(window, 'confirm', { configurable: true, value: () => false }); await click('Eliminar'); assert.equal(removed, false);
    Object.defineProperty(window, 'confirm', { configurable: true, value: () => true }); await click('Eliminar'); assert.equal(removed, true);
    assert.ok(notices.includes('Gasto eliminado.'));
  } finally { await act(async () => root.unmount()); container.remove(); }
});

test('read only expense users see totals and no mutation controls', async () => {
  const api = new ApiClient('https://example.test', async () => Response.json({ items: [expense], page: 1, page_size: 20, has_more: false, summary }));
  const container = document.createElement('div'); document.body.append(container); const root = createRoot(container);
  try {
    await act(async () => root.render(<NavigationGuardProvider><CoreContext.Provider value={{ api, access, notify: () => {} }}><Gastos /></CoreContext.Provider></NavigationGuardProvider>));
    const labels = [...container.querySelectorAll('button')].map(button => button.textContent);
    for (const label of ['Nuevo gasto', 'Editar', 'Eliminar']) assert.equal(labels.includes(label), false);
    assert.ok(container.textContent?.includes(expense.total));
  } finally { await act(async () => root.unmount()); container.remove(); }
});

