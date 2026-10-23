import { after, test } from 'node:test';
import assert from 'node:assert/strict';
import { registerHooks } from 'node:module';
import { Window } from 'happy-dom';
import { act } from 'react';
import { ApiClient } from '../src/api.js';
import { CoreContext } from '../src/ui.js';
import { NavigationGuardProvider } from '../src/navigationGuard.js';
import { quoteFromProcess, sourceQuantity } from '../src/processQuote.js';
import type { ProcessRecord } from '../src/processes.js';
import type { QuoteInput } from '../src/quotes.js';

const window = new Window({ url: 'https://bastion.example.test/' });
for (const name of ['document', 'HTMLElement', 'FormData', 'Event', 'MouseEvent', 'location', 'history'] as const) Object.defineProperty(globalThis, name, { configurable: true, value: window[name] });
Object.assign(globalThis, { window, matchMedia: window.matchMedia.bind(window), IS_REACT_ACT_ENVIRONMENT: true });
Object.defineProperty(globalThis, 'confirm', { configurable: true, value: () => { throw Error('Conversion must not ask for confirmation'); } });
const css = registerHooks({ resolve(specifier, context, next) { return specifier.endsWith('.css') ? { url: 'data:text/javascript,', shortCircuit: true } : next(specifier, context); } });
const { default: App } = await import('../src/App.js');
const { default: Procesos } = await import('../src/pages/Procesos.js');
const { default: Cotizaciones } = await import('../src/pages/Cotizaciones.js');
const { createRoot } = await import('react-dom/client');
after(() => { css.deregister(); void window.happyDOM.abort(); });

const sample: ProcessRecord = {
  proceso_id: 'scout-one', entidad: 'Hospital', expediente: 'EXP-001', fecha_recepcion: '2099-10-23T19:00:00Z',
  puntaje: 88, rubro: 'Equipos', resumen_corto: 'Taladros', marked: false, note: '', version: 1,
  source: { items_fuente: JSON.stringify([['1', '20', 'Taladro ABB XYZ-001, 220V', 'unidades'], ['2', '2.50', 'Cable <script>malicioso</script>', 'metros']]), items: '1. Taladro\n2. Cable', url: 'https://example.test/proceso?id=1' },
};
const withSource = (source: Record<string, string>) => ({ ...sample, source: { ...sample.source, ...source } });
const totals = { subtotal: '0.00', importe_exonerado: '0.00', importe_exento: '0.00', importe_gravado_15: '0.00', importe_gravado_18: '0.00', descuentos_rebajas: '0.00', isv_15: '0.00', isv_18: '0.00', total: '0.00' };

test('conversion preserves source rows, quantities, unit and full specification; uses manual client and no guessed prices', () => {
  const seed = quoteFromProcess(sample, new Date('2026-10-23T02:00:00Z'));
  assert.equal(seed.data.client_name, 'Hospital'); assert.equal(seed.data.id_client, null);
  assert.equal(seed.data.order_number, 'EXP-001'); assert.equal(seed.data.quote_date, '2026-10-22');
  assert.deepEqual(seed.data.items.map(item => item.quantity), ['20', '2.50']);
  assert.equal(seed.data.items[0]?.description, 'Taladro ABB XYZ-001, 220V · unidades');
  assert.equal(seed.data.items[1]?.description, 'Cable <script>malicioso</script> · metros');
  assert.ok(seed.data.items.every(item => item.unit_price === '0'));
  assert.equal(seed.data.status, 'Borrador'); assert.equal(seed.data.notes, '');
  assert.equal(seed.missingQuantities, 0);
});

test('quantities are parsed without floating point or guesses, including ambiguous comma separators', () => {
  for (const value of ['1', '0', '20.0001', '999999999999.9999']) assert.equal(sourceQuantity(value), value);
  assert.equal(sourceQuantity(' 2,50 '), '2.50');
  for (const value of ['1,000', '1,200.00', '20 unidades', '-2', '1e3', '2.12345', '1000000000000', '']) assert.equal(sourceQuantity(value), '');
  const seed = quoteFromProcess(withSource({ items_fuente: '[]', items: '1. 50 guantes\n2. Papel' }));
  assert.deepEqual(seed.data.items.map(item => item.quantity), ['', '']);
  assert.equal(seed.data.items[0]?.description, '50 guantes'); assert.equal(seed.missingQuantities, 2);
});

test('unrecognized or oversized source data fails explicitly without truncating or dropping rows', () => {
  for (const value of ['broken json', '{}', '[{"cantidad":"2"}]', '[["1","2"]]']) assert.throws(() => quoteFromProcess(withSource({ items_fuente: value })));
  assert.throws(() => quoteFromProcess(withSource({ items_fuente: JSON.stringify(Array.from({ length: 201 }, () => ['1', '2', 'X'])) })), /200/);
  assert.throws(() => quoteFromProcess(withSource({ items_fuente: JSON.stringify([['1', '2', 'X'.repeat(3001)]]) })), /3000/);
  assert.throws(() => quoteFromProcess({ ...sample, entidad: 'X'.repeat(256) }), /longitud/);
  assert.throws(() => quoteFromProcess({ ...sample, expediente: 'X'.repeat(101) }), /longitud/);
  assert.throws(() => quoteFromProcess(withSource({ items_fuente: '[]', items: '' })), /no contiene/);
});

test('only HTTP(S) original links without credentials are offered', () => {
  for (const url of ['javascript:alert(1)', 'data:text/html,hello', 'https://user:secret@example.test/', 'not a URL']) assert.equal(quoteFromProcess(withSource({ url })).sourceUrl, null);
  assert.equal(quoteFromProcess(sample).sourceUrl, sample.source.url);
});

async function workspace(options: { editable?: boolean; quotesActive?: boolean; expires?: boolean; standaloneQuote?: boolean; standaloneProcess?: boolean; missing?: boolean; hold?: boolean } = {}) {
  const record = options.missing ? withSource({ items_fuente: '[]' }) : sample;
  const modules = [{ module_id: 'procesos', display_name: 'Procesos' }, ...(options.quotesActive === false ? [] : [{ module_id: 'cotizaciones', display_name: 'Cotizaciones' }])];
  const access = { is_system_administrator: false, modules, permissions: [
    { module_id: 'procesos', scope_key: 'consulta', capability: 'ver' },
    { module_id: 'cotizaciones', scope_key: 'documentos', capability: 'ver' },
    ...(options.editable === false ? [] : [{ module_id: 'cotizaciones', scope_key: 'documentos', capability: 'editar' }]),
  ] };
  let detailCalls = 0, opened = 0;
  let release: (() => void) | undefined;
  const calls: { path: string; method: string; body: QuoteInput | null; key: string | null; csrf: string | null }[] = [];
  const api = new ApiClient('https://api.example.test', async (input, init) => {
    const url = new URL(String(input)), path = url.pathname, method = init?.method ?? 'GET';
    const body = init?.body ? JSON.parse(String(init.body)) as QuoteInput : null;
    const headers = new Headers(init?.headers);
    calls.push({ path, method, body, key: headers.get('Idempotency-Key'), csrf: headers.get('X-CSRF-Token') });
    let data: unknown;
    if (path.endsWith('/csrf')) data = { csrf_token: 'synthetic-csrf' };
    else if (path.endsWith('/bootstrap/status')) data = { setup_required: false };
    else if (path.endsWith('/auth/me')) data = { user: { id: 7, name: 'Usuario', email: 'user@example.test', role: 'Operador' } };
    else if (path.endsWith('/auth/access')) data = access;
    else if (path === '/api/v1/procesos/status') data = null;
    else if (path.endsWith('/entities')) data = { entities: ['Hospital'] };
    else if (path.endsWith('/rows')) data = { items: [{ ...record, source: {} }], total: 1, server_time: new Date().toISOString(), page: 1, page_size: 25, has_more: false };
    else if (path.endsWith('/rows/scout-one')) {
      detailCalls++;
      if (detailCalls > 1 && options.expires) return Response.json({ title: 'Not Found', detail: 'El proceso ya no está vigente.' }, { status: 404 });
      if (detailCalls > 1 && options.hold) await new Promise<void>(resolve => { release = resolve; });
      data = detailCalls > 1 ? { ...record, entidad: 'Hospital actualizado' } : record;
    } else if (path.endsWith('/document-readiness')) data = { pdf: true, company: true, email: true };
    else if (path.endsWith('/calculate')) data = { data: body, valid_until: '2026-11-22', items: [], totals };
    else if (path === '/api/v1/cotizaciones' && method === 'POST') data = { id: 19, version: 1, status: 'Borrador', currency: 'HNL', quote_number: 'COT-000019', data: body, calculation: { data: body, valid_until: '2026-11-22', items: [], totals } };
    else if (path === '/api/v1/cotizaciones') data = { items: [], page: 1, page_size: 25, has_more: false };
    else throw Error(`Unexpected ${method} ${path}`);
    return Response.json(data);
  });
  const container = document.createElement('div'); document.body.append(container);
  const root = createRoot(container);
  await act(async () => root.render(<NavigationGuardProvider>{options.standaloneQuote || options.standaloneProcess ? <CoreContext.Provider value={{ api, access, notify: () => {} }}>{options.standaloneQuote ? <Cotizaciones processSeed={quoteFromProcess(record)} /> : <Procesos openQuote={() => { opened++; }} />}</CoreContext.Provider> : <App api={api} />}</NavigationGuardProvider>));
  const until = async (find: () => Element | null) => { for (let i = 0; i < 80; i++) { if (find()) return find()!; await act(async () => { await new Promise(resolve => setTimeout(resolve, 15)); }); } assert.fail('Expected element did not appear'); };
  const button = (text: string) => Array.from(container.querySelectorAll('button')).find(button => button.textContent === text) ?? null;
  const click = async (element: Element) => { await act(async () => { element.dispatchEvent(new MouseEvent('click', { bubbles: true })); }); };
  const dispose = async () => { release?.(); await act(async () => root.unmount()); container.remove(); };
  return { container, calls, button, until, click, dispose, detailCalls: () => detailCalls, opened: () => opened, release: async () => { await act(async () => { release?.(); }); } };
}

test('real App navigation opens the prefilled editor without a modal or write; normal save retains quantities, CSRF and idempotency', async () => {
  const app = await workspace();
  try {
    await app.click(await app.until(() => app.container.querySelector('nav button:nth-child(2)')));
    await app.click(await app.until(() => app.container.querySelector('tbody button[aria-expanded]')));
    await app.click(await app.until(() => app.button('Convertir a cotización')));
    await app.until(() => app.container.querySelector('.quote-form'));
    assert.equal(app.detailCalls(), 2);
    const field = (name: string) => Array.from(app.container.querySelectorAll('label')).find(label => label.textContent?.startsWith(name))?.querySelector('input');
    assert.equal(app.container.querySelector<HTMLInputElement>('input[role="combobox"]')?.value, 'Hospital actualizado');
    assert.equal(field('Referencia de orden')?.value, 'EXP-001');
    assert.deepEqual(Array.from(app.container.querySelectorAll<HTMLInputElement>('input[aria-label^="Cantidad del ítem"]')).map(input => input.value), ['20', '2.50']);
    assert.equal(app.container.querySelector('[role="dialog"],[role="alertdialog"]'), null);
    const notice = app.container.querySelector('.quote-source-notice')!;
    assert.equal(notice.getAttribute('role'), 'status'); assert.equal(notice.getAttribute('aria-live'), 'polite');
    assert.ok(notice.textContent?.includes('Revisa siempre el proceso original')); assert.equal(notice.querySelector('button'), null);
    assert.equal(notice.querySelector('a')?.href, sample.source.url); assert.equal(app.container.querySelector('script'), null);
    assert.equal(app.calls.filter(call => call.path === '/api/v1/cotizaciones' && call.method === 'POST').length, 0);
    await act(async () => { app.container.querySelector('form')!.dispatchEvent(new Event('submit', { bubbles: true, cancelable: true })); });
    const saved = app.calls.find(call => call.path === '/api/v1/cotizaciones' && call.method === 'POST')!;
    assert.equal(saved.body?.client_name, 'Hospital actualizado'); assert.equal(saved.body?.order_number, 'EXP-001');
    assert.deepEqual(saved.body?.items.map(item => item.quantity), ['20', '2.50']);
    assert.ok(saved.key); assert.equal(saved.csrf, 'synthetic-csrf');
    assert.ok(app.container.querySelector('.quote-source-notice'), 'reminder stays after saving');
    await app.click(app.button('Volver')!);
    assert.equal(app.container.querySelector('.quote-source-notice'), null);
    await app.click(app.button('Nueva cotización')!);
    assert.equal(app.container.querySelector<HTMLInputElement>('input[role="combobox"]')?.value, '');
    assert.equal(app.container.querySelector('.quote-source-notice'), null, 'manual quote cannot inherit the process');
  } finally { await app.dispose(); }
});

test('missing quantities remain editable and suppress calculation requests until completed', async () => {
  const app = await workspace({ standaloneQuote: true, missing: true });
  try {
    assert.ok(app.container.textContent?.includes('2 ítems sin cantidad verificable'));
    const quantity = app.container.querySelector<HTMLInputElement>('input[aria-label="Cantidad del ítem 1"]')!;
    assert.equal(quantity.value, ''); assert.equal(quantity.required, true); assert.equal(quantity.disabled, false);
    assert.equal(app.container.querySelector('form')!.checkValidity(), false);
    await act(async () => { await new Promise(resolve => setTimeout(resolve, 500)); });
    assert.equal(app.calls.filter(call => call.path.endsWith('/calculate')).length, 0);
    assert.equal(app.container.querySelector('[role="dialog"]'), null);
  } finally { await app.dispose(); }
});

test('conversion requires an active destination and edit permission', async () => {
  for (const options of [{ editable: false }, { quotesActive: false }]) {
    const app = await workspace({ ...options, standaloneProcess: true });
    try {
      await app.click(await app.until(() => app.container.querySelector('tbody button[aria-expanded]')));
      const button = await app.until(() => app.button('Convertir a cotización')) as HTMLButtonElement;
      assert.equal(button.disabled, true); assert.equal(app.detailCalls(), 1); assert.equal(app.opened(), 0);
    } finally { await app.dispose(); }
  }
});

test('a process that expires before conversion leaves the detail open with a controlled error', async () => {
  const app = await workspace({ standaloneProcess: true, expires: true });
  try {
    await app.click(await app.until(() => app.container.querySelector('tbody button[aria-expanded]')));
    await app.click(await app.until(() => app.button('Convertir a cotización')));
    assert.equal(app.opened(), 0); assert.ok(app.container.textContent?.includes('ya no está vigente'));
    assert.ok(app.container.querySelector('.process-detail-row'));
  } finally { await app.dispose(); }
});

test('double click prepares only once and leaving the detail cancels the pending handoff', async () => {
  const app = await workspace({ standaloneProcess: true, hold: true });
  try {
    await app.click(await app.until(() => app.container.querySelector('tbody button[aria-expanded]')));
    const button = await app.until(() => app.button('Convertir a cotización'));
    await act(async () => { button.dispatchEvent(new MouseEvent('click', { bubbles: true })); button.dispatchEvent(new MouseEvent('click', { bubbles: true })); });
    assert.equal(app.detailCalls(), 2);
    await app.click(app.container.querySelector('tbody button[aria-expanded]')!);
    await app.release(); assert.equal(app.opened(), 0);
  } finally { await app.dispose(); }
});
