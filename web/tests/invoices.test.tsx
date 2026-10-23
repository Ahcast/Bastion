import { after, test } from 'node:test';
import assert from 'node:assert/strict';
import { Window } from 'happy-dom';
import { renderToStaticMarkup } from 'react-dom/server';
import { ApiClient } from '../src/api.js';
import { CoreContext } from '../src/ui.js';
import { NavigationGuardProvider } from '../src/navigationGuard.js';
import Facturas, { InvoiceEditor, InvoiceTotals } from '../src/pages/Facturas.js';
import { newInvoice } from '../src/invoices.js';
import type { InvoiceCalculation, InvoiceDraft } from '../src/invoices.js';
import type { Access } from '../src/types.js';
import { mainNavigation, settingsSections } from '../src/navigation.js';
import { InvoiceItems } from '../src/components/InvoiceItems.js';

const access: Access = { is_system_administrator: false, modules: [{ module_id: 'facturas', display_name: 'Facturas' }], permissions: [{ module_id: 'facturas', scope_key: 'documentos', capability: 'ver' }] };
const input = newInvoice(new Date('2026-10-23T02:00:00Z'));
input.payment_term_type = 'credito'; input.credit_days = 30;
const calculation: InvoiceCalculation = { data: input, items: [], totals: { subtotal: '100.00', importe_exonerado: '0.00', importe_exento: '0.00', importe_gravado_15: '100.00', importe_gravado_18: '0.00', descuentos_rebajas: '0.00', isv_15: '15.00', isv_18: '0.00', total: '115.00' }, product_investment: '120.00', other_expenses: '5.00', investment_total: '125.00', profit: '-10.00', paid_total: '0.00', balance: '115.00' };
const draft: InvoiceDraft = { id: 7, version: 3, status: 'Borrador', currency: 'HNL', invoice_number: null, data: input, calculation };
const editorAccess: Access = { ...access, permissions: [...access.permissions, { module_id: 'facturas', scope_key: 'documentos', capability: 'editar' }] };

test('invoice date follows Honduras calendar and no fiscal fields or payments are fabricated', () => {
  assert.equal(input.invoice_date, '2026-10-22');
  for (const field of ['invoice_number', 'status', 'total', 'payments', 'quote_id', 'client_request_id']) assert.ok(!(field in input));
  assert.equal(input.items[0]?.cost_price, '0');
  assert.ok(!('tracking' in input.items[0]!));
  assert.ok(!('coments' in input.items[0]!));
  assert.equal(newInvoice().payment_term_type, 'contado');
  assert.equal(newInvoice().credit_days, 0);
});

test('invoice navigation and creation follow activation and separate read/edit capabilities', () => {
  for (const [active, edit] of [[true, true], [true, false], [false, true]]) {
    const context = { ...access, modules: active ? access.modules : [], permissions: edit ? editorAccess.permissions : access.permissions };
    const html = renderToStaticMarkup(<CoreContext.Provider value={{ api: new ApiClient('https://example.test'), access: context, notify: () => {} }}><Facturas /></CoreContext.Provider>);
    assert.equal(html.includes('Nueva factura'), active && edit);
    assert.equal(html.includes('No tienes acceso'), !active);
    assert.equal(mainNavigation(context).some(page => page.id === 'facturas'), active);
    assert.equal(settingsSections(context).some(section => section.id === 'facturas'), active);
    assert.equal(html.includes('Ingresar pago'), false);
  }
});

test('invoice summary renders server decimals and negative profit without floating point arithmetic', () => {
  const html = renderToStaticMarkup(<InvoiceTotals calculation={calculation} />);
  assert.ok(html.includes('L -10.00'));
  assert.ok(html.includes('L 115.00'));
  assert.ok(html.includes('Borrador sin emisión'));
});

test('invoice rows do not assign a server total to a different item after blank rows are filtered', () => {
  const shortened = { ...calculation, data: { ...input, items: [] }, items: [{ total: '999.99' }] } as unknown as InvoiceCalculation;
  const html = renderToStaticMarkup(<InvoiceItems items={input.items} calculation={shortened} change={() => {}} remove={() => {}} add={() => {}} />);
  assert.ok(html.includes('Ítems de factura'));
  assert.ok(!html.includes('Tracking'));
  assert.ok(!html.includes('Comentarios'));
  assert.ok(!html.includes('L 999.99'));
  assert.ok(html.includes('Total del ítem 1'));
});

const window = new Window();
for (const name of ['document', 'HTMLElement', 'HTMLInputElement', 'HTMLTextAreaElement', 'FormData', 'Event', 'MouseEvent', 'KeyboardEvent', 'navigator'] as const) Object.defineProperty(globalThis, name, { configurable: true, value: window[name] });
Object.assign(globalThis, { window, IS_REACT_ACT_ENVIRONMENT: true });
const { act } = await import('react');
const { createRoot } = await import('react-dom/client');
after(() => window.happyDOM.abort());

test('invoice client combobox saves a catalog selection and clears its snapshot when changed to manual', async () => {
  const bodies: Record<string, unknown>[] = [];
  let catalogCalls = 0;
  const api = new ApiClient('https://example.test', async (url, init = {}) => {
    const path = new URL(String(url)).pathname;
    if (path.endsWith('/catalogo')) { catalogCalls++; return Response.json({ items: [{ id: 41, cliente_name: 'Cliente del catálogo', rtn: '08011900000000', email: 'client@example.test' }], page: 1, page_size: 25, has_more: false }); }
    if (init.method === 'PUT') { const data = JSON.parse(String(init.body)); bodies.push(data); return Response.json({ ...draft, version: 4, data, calculation: { ...calculation, data } }); }
    return Response.json(calculation);
  });
  const context = { ...editorAccess, modules: [...editorAccess.modules, { module_id: 'clientes', display_name: 'Clientes' }], permissions: [...editorAccess.permissions, { module_id: 'clientes', scope_key: 'catalogo', capability: 'ver' }] };
  const container = document.createElement('div'); document.body.append(container); const root = createRoot(container);
  try {
    await act(async () => root.render(<NavigationGuardProvider><CoreContext.Provider value={{ api, access: context, notify: () => {} }}><InvoiceEditor initial={draft} editable close={() => {}} /></CoreContext.Provider></NavigationGuardProvider>));
    assert.equal(catalogCalls, 0);
    assert.equal(container.querySelectorAll('[role="combobox"]').length, 1);
    await act(async () => container.querySelector<HTMLButtonElement>('[aria-label="Mostrar clientes"]')!.click());
    const field = container.querySelector<HTMLInputElement>('[role="combobox"]')!;
    for (const key of ['ArrowDown', 'ArrowDown', 'Enter']) await act(async () => field.dispatchEvent(new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true })));
    assert.equal(field.value, 'Cliente del catálogo');
    assert.equal(bodies.length, 0);
    await act(async () => container.querySelector('form')!.dispatchEvent(new Event('submit', { bubbles: true, cancelable: true })));
    assert.equal(bodies[0]?.id_client, 41);
    assert.equal(bodies[0]?.client_rtn, '08011900000000');
    assert.equal(bodies[0]?.client_email, 'client@example.test');
    await act(async () => { Object.getOwnPropertyDescriptor(window.HTMLInputElement.prototype, 'value')!.set!.call(field, 'Cliente manual'); field.dispatchEvent(new Event('input', { bubbles: true })); });
    await act(async () => container.querySelector('form')!.dispatchEvent(new Event('submit', { bubbles: true, cancelable: true })));
    assert.equal(bodies[1]?.id_client, null);
    assert.equal(bodies[1]?.client_name, 'Cliente manual');
    assert.equal(bodies[1]?.client_rtn, '');
    assert.equal(bodies[1]?.client_email, '');
  } finally { await act(async () => root.unmount()); container.remove(); }
});

test('invoice dropdown preserves the nine requested actions, permissions, keyboard and view/edit modes', async () => {
  const labels = ['Ver', 'Editar', 'Marcar como enviado', 'Ingresar pago', 'Clonar', 'PDF', 'Nota de entrega', 'Emitir Recibo', 'Eliminar'];
  for (const editable of [true, false]) {
    const calls: string[] = [];
    const api = new ApiClient('https://example.test', async (url, init = {}) => {
      calls.push(init.method ?? 'GET');
      return Response.json(new URL(String(url)).pathname.endsWith('/7') ? draft : { items: [{ ...draft, client_name: input.client_name, invoice_date: input.invoice_date, total: '115.00', balance: '115.00', item_count: 1 }], page: 1, page_size: 25, has_more: false });
    });
    const container = document.createElement('div'); document.body.append(container); const root = createRoot(container);
    const click = async (button: HTMLButtonElement | null | undefined) => { assert.ok(button); await act(async () => button.click()); };
    try {
      await act(async () => root.render(<NavigationGuardProvider><CoreContext.Provider value={{ api, access: editable ? editorAccess : access, notify: () => {} }}><Facturas /></CoreContext.Provider></NavigationGuardProvider>));
      const trigger = container.querySelector<HTMLButtonElement>('[aria-haspopup="menu"]')!;
      await click(trigger);
      let items = [...document.querySelectorAll<HTMLButtonElement>('[role="menuitem"]')];
      assert.deepEqual(items.map(item => item.childNodes[0]?.textContent), labels);
      assert.equal(items[0]?.disabled, false);
      assert.equal(items[1]?.disabled, !editable);
      assert.ok(items.slice(2).every(item => item.disabled && item.querySelector('small')?.textContent));
      assert.equal(document.activeElement, items[0]);
      await act(async () => document.querySelector('[role="menu"]')!.dispatchEvent(new KeyboardEvent('keydown', { key: 'End', bubbles: true })));
      assert.equal(document.activeElement, items[editable ? 1 : 0]);
      await act(async () => document.querySelector('[role="menu"]')!.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true })));
      assert.equal(document.querySelector('[role="menu"]'), null);
      assert.equal(document.activeElement, trigger);
      await click(trigger);
      items = [...document.querySelectorAll<HTMLButtonElement>('[role="menuitem"]')];
      await click(items[0]);
      assert.ok(container.querySelector<HTMLFieldSetElement>('.quote-form')?.disabled);
      assert.equal(document.querySelector('[role="menu"]'), null);
      await click([...container.querySelectorAll('button')].find(button => button.textContent === 'Volver'));
      await click(container.querySelector<HTMLButtonElement>('[aria-haspopup="menu"]'));
      items = [...document.querySelectorAll<HTMLButtonElement>('[role="menuitem"]')];
      await click(items[1]);
      assert.equal(Boolean(container.querySelector('.invoice-editor')), editable);
      if (editable) assert.equal(container.querySelector<HTMLFieldSetElement>('.quote-form')?.disabled, false);
      assert.ok(calls.every(method => method === 'GET'), 'opening menus and drafts never issues mutations');
    } finally { await act(async () => root.unmount()); container.remove(); }
  }
});

test('invoice editor retains changes on version conflict and confirms navigation before discarding', async () => {
  let writes = 0, closes = 0;
  const api = new ApiClient('https://example.test', async (_url, init = {}) => {
    if (init.method === 'PUT') {
      writes++;
      assert.equal(new Headers(init.headers).get('If-Match'), '"3"');
      assert.deepEqual(JSON.parse(String(init.body)), input);
      return Response.json({ detail: 'El borrador cambió.' }, { status: 412 });
    }
    return Response.json(draft);
  });
  const container = document.createElement('div'); document.body.append(container); const root = createRoot(container);
  const click = async (text: string) => { const button = [...document.querySelectorAll('button')].find(b => b.textContent?.trim().endsWith(text)); assert.ok(button, text); await act(async () => button.click()); };
  try {
    await act(async () => root.render(<NavigationGuardProvider><CoreContext.Provider value={{ api, access: editorAccess, notify: () => {} }}><InvoiceEditor initial={draft} editable close={() => closes++} /></CoreContext.Provider></NavigationGuardProvider>));
    await act(async () => container.querySelector('form')!.dispatchEvent(new Event('submit', { bubbles: true, cancelable: true })));
    assert.equal(writes, 1);
    assert.ok(container.textContent?.includes('Tus cambios se conservan'));
    assert.ok(container.querySelector<HTMLFieldSetElement>('fieldset')?.disabled);
    // Refreshing a stale, unmodified view uses the explicit current-version path.
    await click('Abrir versión actual');
    assert.ok(!container.querySelector<HTMLFieldSetElement>('fieldset')?.disabled);
    await click('Agregar gasto');
    await click('Volver');
    assert.equal(closes, 0);
    assert.ok(document.querySelector('[role="dialog"]')?.textContent?.includes('Cambios sin guardar'));
    await click('Seguir editando');
    assert.ok(container.textContent?.includes('Gasto 1'));
    await click('Volver');
    await click('Descartar y continuar');
    assert.equal(closes, 1);
  } finally { await act(async () => root.unmount()); container.remove(); }
});

test('ambiguous create failures retain the idempotency key and read-only mode cannot mutate', async () => {
  const keys: (string | null)[] = [];
  const api = new ApiClient('https://example.test', async (url, init = {}) => {
    if (new URL(String(url)).pathname === '/api/v1/facturas' && init.method === 'POST') {
      keys.push(new Headers(init.headers).get('Idempotency-Key'));
      if (keys.length === 1) throw new TypeError('synthetic network failure');
      return Response.json(draft);
    }
    return Response.json(calculation);
  });
  const container = document.createElement('div'); document.body.append(container); const root = createRoot(container);
  try {
    await act(async () => root.render(<NavigationGuardProvider><CoreContext.Provider value={{ api, access: editorAccess, notify: () => {} }}><InvoiceEditor initial={null} editable close={() => {}} /></CoreContext.Provider></NavigationGuardProvider>));
    assert.ok(container.textContent?.includes('Contado exige pago total'));
    assert.ok(container.querySelector<HTMLButtonElement>('button[type="submit"]')?.disabled);
    await act(async () => { const select = container.querySelector<HTMLSelectElement>('[name="payment_term_type"]')!; select.value = 'credito'; select.dispatchEvent(new Event('change', { bubbles: true })); });
    for (let i = 0; i < 2; i++) await act(async () => container.querySelector('form')!.dispatchEvent(new Event('submit', { bubbles: true, cancelable: true })));
    assert.equal(keys.length, 2); assert.ok(keys[0]); assert.equal(keys[0], keys[1]);
    await act(async () => root.render(<NavigationGuardProvider><CoreContext.Provider value={{ api, access, notify: () => {} }}><InvoiceEditor key="view" initial={draft} editable={false} close={() => {}} /></CoreContext.Provider></NavigationGuardProvider>));
    assert.ok(container.querySelector<HTMLFieldSetElement>('fieldset')?.disabled);
    assert.equal([...container.querySelectorAll('button')].some(b => b.textContent === 'Guardar borrador'), false);
  } finally { await act(async () => root.unmount()); container.remove(); }
});
