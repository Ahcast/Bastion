import { after, test } from 'node:test';
import assert from 'node:assert/strict';
import { Window } from 'happy-dom';
import { act } from 'react';
import { ApiClient } from '../src/api.js';
import { CoreContext } from '../src/ui.js';
import { NavigationGuardProvider } from '../src/navigationGuard.js';
import TenderAwards from '../src/components/TenderAwards.js';
import { invoiceFromAward } from '../src/tenderAwards.js';
import type { Award } from '../src/tenderAwards.js';
import type { Expedient } from '../src/tenders.js';

const window = new Window();
for (const name of ['document', 'HTMLElement', 'HTMLInputElement', 'FormData', 'Event', 'MouseEvent'] as const) {
  Object.defineProperty(globalThis, name, { configurable: true, value: window[name] });
}
Object.assign(globalThis, { window, IS_REACT_ACT_ENVIRONMENT: true });
const { createRoot } = await import('react-dom/client');
after(() => window.happyDOM.abort());

const record: Expedient = {
  id: 4, name: 'Tender', state: 'enviada', version: 8, forms: [],
  variables: [{ key: 'lotes', label: 'Lotes', kind: 'integer', value: '1' }],
};
const award: Award = {
  id: 7, expedient_id: 4, version: 1,
  snapshot: { expedient_name: 'Tender', expedient_version: 8,
    items: [{ lot_number: 1, item_number: 1, description: 'Equipment', quantity: '2.000000000000', unit: 'und' }] },
};

async function workspace(registered: boolean) {
  const calls: { path: string; method: string; key: string | null; body: unknown }[] = [];
  let fail = false;
  const api = new ApiClient('https://api.example.test', async (url, options) => {
    const path = new URL(String(url)).pathname;
    const method = options?.method ?? 'GET';
    calls.push({ path, method, key: new Headers(options?.headers).get('Idempotency-Key'),
      body: options?.body ? JSON.parse(String(options.body)) : null });
    if (path.endsWith('/adjudicaciones') && method === 'GET') {
      return Response.json({ items: registered ? [{ id: award.id, expedient_id: 4, item_count: 1 }] : [], page: 1, page_size: 25, has_more: false });
    }
    if (path.endsWith('/adjudicacion')) {
      return Response.json({ lot_number: 1, version: 8,
        items: [{ item_number: 1, description: 'Equipment', offered_quantity: '5', remaining_quantity: '3', unit: 'und' }] });
    }
    if (path.endsWith('/adjudicaciones') && method === 'POST') {
      if (fail) { fail = false; return Response.json({ detail: 'Retry' }, { status: 503 }); }
      return Response.json(award);
    }
    if (path.endsWith('/adjudicaciones/7')) return Response.json(award);
    if (path.endsWith('/factura')) return Response.json(null);
    throw new Error(`Unexpected request ${method} ${path}`);
  });
  const container = document.createElement('div');
  container.id = 'app';
  document.body.append(container);
  const root = createRoot(container);
  await act(async () => root.render(
    <CoreContext.Provider value={{ api, notify: () => {}, access: {
      is_system_administrator: true,
      modules: ['licitaciones', 'facturas', 'clientes'].map(module_id => ({ module_id, display_name: module_id })),
      permissions: [],
    } }}>
      <NavigationGuardProvider><TenderAwards record={record} /></NavigationGuardProvider>
    </CoreContext.Provider>,
  ));
  const click = async (element: Element | null | undefined) => {
    assert.ok(element);
    await act(async () => { element.dispatchEvent(new MouseEvent('click', { bubbles: true })); });
  };
  return {
    container, calls, click,
    button: (name: string) => Array.from(container.querySelectorAll('button')).find(button => button.textContent === name),
    failOnce: () => { fail = true; },
    dispose: async () => { await act(async () => root.unmount()); container.remove(); },
  };
}

test('review seeds exact awarded quantities and requires manual prices and client selection', () => {
  const input = invoiceFromAward(award);
  assert.equal(input.id_client, null);
  assert.equal(input.payment_term_type, 'credito');
  assert.equal(input.items[0]?.quantity, '2');
  assert.equal(input.items[0]?.unit_price, '0');
  assert.equal(input.initial_payment, undefined);
  const precise = structuredClone(award);
  precise.snapshot.items[0]!.quantity = '1.234';
  assert.throws(() => invoiceFromAward(precise), /dos decimales/);
});

test('opening a review preserves selection and creates no invoice before saving', async () => {
  const app = await workspace(true);
  try {
    await app.click(app.button('Preparar o abrir factura'));
    assert.ok(app.container.textContent?.includes('Elige un cliente del catálogo'));
    const quantity = app.container.querySelector<HTMLInputElement>('input[aria-label="Cantidad del ítem 1"]');
    assert.equal(quantity?.readOnly, true);
    assert.equal(quantity?.value, '2');
    assert.equal(app.container.querySelector<HTMLButtonElement>('button[aria-label="Quitar ítem 1"]')?.disabled, true);
    assert.equal(app.container.textContent?.includes('Registrar pago al guardar'), false);
    assert.equal(app.calls.filter(call => call.method === 'POST' && call.path === '/api/v1/facturas').length, 0);
    assert.equal(app.calls.filter(call => call.method === 'POST' && call.path.endsWith('/factura')).length, 0);
  } finally {
    await app.dispose();
  }
});

test('registering a selection preserves its idempotency key after an ambiguous failure', async () => {
  const app = await workspace(false);
  try {
    await app.click(app.button('Registrar adjudicación'));
    await app.click(app.container.querySelector('input[type="checkbox"]'));
    app.failOnce();
    const submit = async () => {
      const form = app.container.querySelector('form');
      assert.ok(form);
      await act(async () => { form.dispatchEvent(new Event('submit', { bubbles: true, cancelable: true })); });
    };
    await submit();
    assert.ok(app.container.textContent?.includes('Retry'));
    await submit();
    const saves = app.calls.filter(call => call.method === 'POST' && call.path.endsWith('/adjudicaciones'));
    assert.equal(saves.length, 2);
    assert.ok(saves[0]?.key);
    assert.equal(saves[0]?.key, saves[1]?.key);
    assert.deepEqual(saves[0]?.body, { items: [{ lot_number: 1, item_number: 1, quantity: '3' }] });
  } finally {
    await app.dispose();
  }
});
