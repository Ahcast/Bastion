import { after, test } from 'node:test';
import assert from 'node:assert/strict';
import { registerHooks } from 'node:module';
import { Window } from 'happy-dom';
import { act } from 'react';
import { createMemoryRouter, RouterProvider } from 'react-router';
import { ApiClient } from '../src/api.js';
import { NavigationGuardProvider } from '../src/navigationGuard.js';
import { RoutingProvider } from '../src/routing.js';
import { newQuote } from '../src/quotes.js';
import type { Access } from '../src/types.js';

const window = new Window({ url: 'https://bastion.example.test/' });
for (const name of ['document', 'HTMLElement', 'FormData', 'Event', 'MouseEvent', 'location', 'history', 'KeyboardEvent'] as const) Object.defineProperty(globalThis, name, { configurable: true, value: window[name] });
Object.assign(globalThis, { window, matchMedia: window.matchMedia.bind(window), IS_REACT_ACT_ENVIRONMENT: true });
const css = registerHooks({ resolve(specifier, context, next) { return specifier.endsWith('.css') ? { url: 'data:text/javascript,', shortCircuit: true } : next(specifier, context); } });
const { default: App } = await import('../src/App.js');
const { createRoot } = await import('react-dom/client');
after(() => { css.deregister(); void window.happyDOM.abort(); });

async function app(path: string, permitted = true, signedIn = true) {
  const calls: { path: string; method: string }[] = [];
  const access: Access = { is_system_administrator: false, modules: permitted ? [{ module_id: 'cotizaciones', display_name: 'Cotizaciones' }] : [], permissions: [{ module_id: 'cotizaciones', scope_key: 'documentos', capability: 'ver' }] };
  const api = new ApiClient('https://api.example.test', async (input, init) => {
    const url = new URL(String(input)), path = url.pathname, method = init?.method ?? 'GET'; calls.push({ path, method });
    let data: unknown;
    if (path.endsWith('/csrf')) data = { csrf_token: 'fixture-csrf' };
    else if (path.endsWith('/bootstrap/status')) data = { setup_required: false };
    else if (path.endsWith('/auth/me')) {
      if (!signedIn) return Response.json({ title: 'Unauthorized' }, { status: 401 });
      data = { user: { id: 7, name: 'Operador', email: 'fixture@example.test', role: 'Ventas' } };
    } else if (path.endsWith('/auth/access')) data = access;
    else if (path.endsWith('/document-readiness')) data = { pdf: true, attachments: true, email: true };
    else if (path === '/api/v1/cotizaciones') data = { items: [], page: 1, page_size: 25, has_more: false };
    else if (path === '/api/v1/cotizaciones/7') {
      const data = { ...newQuote(), client_name: 'Cliente de prueba local' };
      return Response.json({ id: 7, version: 1, status: 'Borrador', currency: 'HNL', quote_number: 'COT-000007', data, calculation: { data, valid_until: '2099-01-01', items: [], totals: { subtotal: '0.00', total: '0.00' } } });
    } else if (path.endsWith('/attachments')) data = { items: [], has_more: false };
    else throw Error('Unexpected request ' + path);
    return Response.json(data);
  });
  const router = createMemoryRouter([{ path: '*', element: <NavigationGuardProvider><RoutingProvider><App api={api} /></RoutingProvider></NavigationGuardProvider> }], { initialEntries: [path] });
  const container = document.createElement('div'); document.body.append(container); const root = createRoot(container);
  await act(async () => root.render(<RouterProvider router={router} />));
  const until = async (check: () => boolean) => { for (let i = 0; i < 60 && !check(); i++) await act(async () => { await new Promise(r => setTimeout(r, 10)); }); assert.ok(check(), container.textContent ?? 'Empty screen'); };
  return { calls, container, router, until, dispose: async () => { await act(async () => root.unmount()); router.dispose(); container.remove(); } };
}

test('App opens an authorized quotation directly, and Back/Forward restore list/detail without writes', async () => {
  const f = await app('/cotizaciones/7');
  try {
    await f.until(() => Boolean(f.container.querySelector('.quote-form')));
    assert.equal(f.router.state.location.pathname, '/cotizaciones/7');
    assert.ok(f.calls.some(c => c.path === '/api/v1/cotizaciones/7'));
    await act(async () => { await f.router.navigate('/cotizaciones'); });
    await f.until(() => Boolean(f.container.querySelector('.quote-list')));
    await act(async () => { await f.router.navigate(-1); });
    await f.until(() => Boolean(f.container.querySelector('.quote-form')));
    await act(async () => { await f.router.navigate(1); });
    await f.until(() => Boolean(f.container.querySelector('.quote-list')));
    assert.ok(f.calls.every(c => c.method === 'GET'));
  } finally { await f.dispose(); }
});

test('App denies inactive modules and edit-only URLs before mounting their resource screens', async () => {
  for (const [path, active] of [['/cotizaciones/7', false], ['/cotizaciones/7?modo=edit', true], ['/ajustes/roles', true]] as const) {
    const f = await app(path, active);
    try {
      await f.until(() => Boolean(f.container.textContent?.includes('No tienes permiso')));
      assert.ok(!f.calls.some(c => c.path.startsWith('/api/v1/cotizaciones')));
    } finally { await f.dispose(); }
  }
});

test('unauthenticated deep links show login, retain the destination and never fetch document data', async () => {
  const f = await app('/cotizaciones/7', true, false);
  try {
    await f.until(() => Boolean(f.container.querySelector('input[type="password"]')));
    assert.equal(f.router.state.location.pathname, '/cotizaciones/7');
    assert.ok(!f.calls.some(c => c.path.startsWith('/api/v1/cotizaciones')));
  } finally { await f.dispose(); }
});
