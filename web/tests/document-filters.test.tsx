import { after, test } from 'node:test';
import assert from 'node:assert/strict';
import { Window } from 'happy-dom';
import { ApiClient } from '../src/api.js';
import { CoreContext } from '../src/ui.js';
import { NavigationGuardProvider } from '../src/navigationGuard.js';
import OrdenesCompra from '../src/pages/OrdenesCompra.js';
import Facturas from '../src/pages/Facturas.js';
import { clearDocumentFilters } from '../src/documentFilters.js';
import type { Access } from '../src/types.js';

const window = new Window({ url: 'https://example.test' });
for (const name of ['document', 'HTMLElement', 'HTMLInputElement', 'HTMLSelectElement', 'Event', 'MouseEvent', 'KeyboardEvent', 'navigator'] as const) Object.defineProperty(globalThis, name, { configurable: true, value: window[name] });
Object.assign(globalThis, { window, IS_REACT_ACT_ENVIRONMENT: true });
const { act } = await import('react');
const { createRoot } = await import('react-dom/client');
after(() => window.happyDOM.abort());

test('each document list restores applied filters and page across module navigation and users, and clears explicitly', async () => {
  const access: Access = { is_system_administrator: true, modules: ['facturas', 'ordenes_compra'].map(module_id => ({ module_id, display_name: module_id })), permissions: [] };
  const calls: URL[] = [];
  const api = new ApiClient('https://example.test', async url => {
    const request = new URL(String(url)); calls.push(request);
    return Response.json({ items: [], has_more: true });
  });
  const container = document.createElement('div'); document.body.append(container);
  const root = createRoot(container);
  const mount = async (module: 'orders' | 'invoices', userId = 17) => {
    await act(async () => root.render(<NavigationGuardProvider><CoreContext.Provider value={{ api, access, userId, notify: () => {} }}>{module === 'orders' ? <OrdenesCompra key={userId} /> : <Facturas key={userId} />}</CoreContext.Provider></NavigationGuardProvider>));
  };
  const click = async (text: string) => {
    const button = [...container.querySelectorAll('button')].find(node => node.textContent === text); assert.ok(button, text);
    await act(async () => button.click());
  };
  const field = async (label: string, value: string) => {
    const control = [...container.querySelectorAll('label')].find(node => node.textContent?.startsWith(label))?.querySelector('input,select') as HTMLInputElement | HTMLSelectElement;
    assert.ok(control, label);
    await act(async () => {
      Object.getOwnPropertyDescriptor(control.tagName === 'SELECT' ? window.HTMLSelectElement.prototype : window.HTMLInputElement.prototype, 'value')!.set!.call(control, value);
      control.dispatchEvent(new Event(control.tagName === 'SELECT' ? 'change' : 'input', { bubbles: true }));
    });
  };
  const last = () => calls.filter(url => /\/(facturas|ordenes-compra)$/.test(url.pathname)).at(-1)!;
  try {
    clearDocumentFilters();
    for (const module of ['orders', 'invoices'] as const) {
      await mount(module); await click('Limpiar filtros');
      await field('Buscar', 'ZX-4400'); await field('Cliente', 'Cliente % _');
      await field('Fecha desde', '2026-11-01'); await field('Fecha hasta', '2026-11-16');
      const status = module === 'orders' ? 'Listo' : 'Enviado'; await field('Estado', status);
      assert.equal(last().searchParams.get('client'), null, 'draft edits do not apply');
      await act(async () => container.querySelector('form')!.dispatchEvent(new Event('submit', { bubbles: true, cancelable: true })));
      await click('Siguiente'); const expected = last().search;
      assert.equal(last().searchParams.get('client'), 'Cliente % _');
      assert.equal(last().searchParams.get('date_from'), '2026-11-01');
      assert.equal(last().searchParams.get('date_to'), '2026-11-16');
      assert.equal(last().searchParams.get('status'), status);
      assert.equal(last().searchParams.get('page'), '2');
      await field('Cliente', 'Borrador sin aplicar');
      await mount(module === 'orders' ? 'invoices' : 'orders'); await mount(module);
      assert.equal(last().search, expected);
      assert.equal(container.querySelector<HTMLInputElement>('input[placeholder="Nombre del cliente"]')?.value, 'Cliente % _');
      await mount(module, 99); assert.equal(last().searchParams.get('client'), null);
      await mount(module); assert.equal(last().search, expected);
      await click('Limpiar filtros'); assert.equal(last().searchParams.get('client'), null); assert.equal(last().searchParams.get('page'), '1');
      await mount(module === 'orders' ? 'invoices' : 'orders'); await mount(module); assert.equal(last().searchParams.get('status'), null);
    }
    clearDocumentFilters(); assert.equal(window.sessionStorage.length, 0);
  } finally { await act(async () => root.unmount()); container.remove(); }
});
