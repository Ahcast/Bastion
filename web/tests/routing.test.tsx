import { after, test } from 'node:test';
import assert from 'node:assert/strict';
import { Window } from 'happy-dom';
import { act, useState } from 'react';
import { createRoot } from 'react-dom/client';
import { createMemoryRouter, RouterProvider } from 'react-router';
import { NavigationGuardProvider, useUnsavedChanges } from '../src/navigationGuard.js';
import { RoutingProvider, useAppRoute, useRouteRecord, useRouteState, enumRoute, pageRoute } from '../src/routing.js';
import { modulePaths, parseRoute, routeAllowed, routeCatalog, routeHref } from '../src/routes.js';
import type { Access, Page } from '../src/types.js';
import type { SettingsSection } from '../src/navigation.js';

const window = new Window({ url: 'https://app.example.test/' });
for (const name of ['document', 'HTMLElement', 'Event', 'MouseEvent', 'KeyboardEvent'] as const) Object.defineProperty(globalThis, name, { configurable: true, value: window[name] });
Object.assign(globalThis, { window, IS_REACT_ACT_ENVIRONMENT: true });
after(() => window.happyDOM.abort());

test('every module and settings screen has a round-trippable route', () => {
  for (const page of Object.keys(modulePaths) as Page[]) assert.equal(parseRoute(modulePaths[page])?.page, page);
  for (const [area, schema] of Object.entries(routeCatalog)) {
    const [page, section] = area.split('/') as [Page, SettingsSection | undefined];
    for (const vista of [undefined, ...(schema.views ?? [])]) {
      for (const registro of [undefined, ...(schema.record ? ['43'] : [])]) {
        const values: Record<string, string> = {};
        if (vista) values.vista = vista;
        if (registro) values.registro = registro;
        const route = { page, section, values };
        assert.deepEqual(parseRoute(routeHref(route)), route);
      }
    }
  }
  assert.deepEqual(parseRoute('/cotizaciones/42', '?modo=edit')?.values, { registro: '42', modo: 'edit' });
  for (const path of ['/unknown', '/ajustes/unknown', '/facturas/-1', '/facturas/%2Fetc', '/facturas/1/2', '/facturas/%E0%A4', '/ajustes/cuenta/evil']) assert.equal(parseRoute(path), null, path);
  for (const query of ['?token=secret', '?return=https://evil.test', '?modo=edit&modo=view', '?pagina=0', '?pagina=nuevo']) assert.equal(parseRoute('/facturas', query), null, query);
});

test('direct links retain module activation, scope, action and administrative permissions', () => {
  const access: Access = { is_system_administrator: false, modules: [{ module_id: 'facturas', display_name: 'Facturas' }], permissions: [{ module_id: 'facturas', scope_key: 'documentos', capability: 'ver' }] };
  const allowed = (path: string, search = '', who = access) => routeAllowed(parseRoute(path, search)!, who);
  assert.equal(allowed('/facturas/3'), true);
  assert.equal(allowed('/facturas/3', '?modo=edit'), false);
  assert.equal(allowed('/facturas/nuevo'), false);
  assert.equal(allowed('/facturas/3', '', { ...access, modules: [] }), false);
  assert.equal(allowed('/ajustes/roles'), false);
  assert.equal(allowed('/ajustes/respaldos'), false);
  assert.equal(allowed('/ajustes/cuenta/security'), true);
  assert.equal(allowed('/ajustes/mcp'), false);
  assert.equal(allowed('/cotizaciones/3'), false);
});

test('returning to page one overrides a previously remembered page', () => {
  const encoded = pageRoute.write(1);
  assert.equal(encoded, '1');
  assert.equal(pageRoute.read(encoded!, 5), 1);
});

type Row = { id: number } | 'new' | null;
async function fixture(initial = '/facturas', deferred = false) {
  const requests: { id: string; signal: AbortSignal; resolve: (value: Row) => void }[] = [];
  let dirty!: (value: boolean) => void, saving!: (value: boolean) => void;
  function Screen() {
    const routing = useAppRoute()!;
    const [changed, setChanged] = useState(false), [busy, setBusy] = useState(false);
    dirty = setChanged; saving = setBusy;
    useUnsavedChanges(changed, busy);
    const [record, setRecord] = useRouteRecord<Row>('registro', null, (id, signal) => new Promise(resolve => {
      requests.push({ id, signal, resolve }); if (!deferred) resolve({ id: Number(id) });
    }));
    const [mode, setMode] = useRouteState<'view' | 'edit'>('modo', 'view', enumRoute(['view', 'edit']));
    return <><output>{routing.route?.page}:{record === 'new' ? 'new' : record?.id ?? 'list'}:{mode}</output>
      <button onClick={() => { routing.patch('registro', '7'); setMode('edit'); }}>Open</button>
      <button onClick={() => routing.patch('registro', '8')}>Next</button>
      <button onClick={() => routing.patch('registro', 'nuevo')}>New</button>
      <button onClick={() => setRecord({ id: 9 })}>Saved</button>
      <button onClick={() => { routing.patch('registro', '11'); routing.go({ page: 'inicio', values: {} }); }}>Supersede</button>
    </>;
  }
  const router = createMemoryRouter([{ path: '*', element: <NavigationGuardProvider><RoutingProvider><Screen /></RoutingProvider></NavigationGuardProvider> }], { initialEntries: [initial] });
  const container = document.createElement('div'); document.body.append(container); const root = createRoot(container);
  await act(async () => root.render(<RouterProvider router={router} />));
  const click = async (text: string) => { const button = Array.from(document.querySelectorAll('button')).find(b => b.textContent === text); assert.ok(button, text); await act(async () => button.click()); };
  const navigate = async (delta: number) => { await act(async () => { await router.navigate(delta); }); };
  const dispose = async () => { await act(async () => root.unmount()); router.dispose(); container.remove(); };
  return { router, container, requests, click, navigate, dispose, dirty: async (value: boolean) => { await act(async () => dirty(value)); }, saving: async (value: boolean) => { await act(async () => saving(value)); } };
}

test('a click pushes one entry; Back, Forward and direct loading restore the record and mode', async () => {
  const f = await fixture();
  try {
    await f.click('Open'); assert.equal(f.router.state.location.pathname, '/facturas/7'); assert.equal(f.container.querySelector('output')?.textContent, 'facturas:7:edit');
    await f.navigate(-1); assert.equal(f.router.state.location.pathname, '/facturas'); assert.equal(f.container.querySelector('output')?.textContent, 'facturas:list:view');
    await f.navigate(1); assert.equal(f.container.querySelector('output')?.textContent, 'facturas:7:edit');
    assert.deepEqual(f.requests.map(r => r.id), ['7', '7']);
  } finally { await f.dispose(); }
  const direct = await fixture('/facturas/7?modo=edit');
  try { assert.equal(direct.container.querySelector('output')?.textContent, 'facturas:7:edit'); } finally { await direct.dispose(); }
});

test('Back respects unsaved cancellation, discard and pending writes; Forward remains usable', async () => {
  const f = await fixture();
  try {
    await f.click('Open'); await f.dirty(true); await f.navigate(-1);
    assert.ok(document.body.textContent?.includes('Cambios sin guardar'));
    await f.click('Seguir editando'); assert.equal(f.router.state.location.pathname, '/facturas/7');
    await f.navigate(-1); await f.click('Descartar y continuar'); assert.equal(f.router.state.location.pathname, '/facturas');
    await f.dirty(false); await f.navigate(1); assert.equal(f.router.state.location.pathname, '/facturas/7');
    await f.saving(true); await f.navigate(-1); assert.equal(f.router.state.location.pathname, '/facturas/7');
    await f.saving(false); await f.navigate(-1); assert.equal(f.router.state.location.pathname, '/facturas');
  } finally { await f.dispose(); }
});

test('record changes abort stale loads, and leaving a module supersedes queued URL updates', async () => {
  const f = await fixture('/facturas/7', true);
  try {
    await f.click('Next'); assert.equal(f.requests[0]!.signal.aborted, true);
    await act(async () => { f.requests[0]!.resolve({ id: 7 }); f.requests[1]!.resolve({ id: 8 }); });
    assert.equal(f.container.querySelector('output')?.textContent, 'facturas:8:view');
    await f.click('Supersede'); assert.equal(f.router.state.location.pathname, '/');
  } finally { await f.dispose(); }
});

test('saving a new record replaces the new URL so Back cannot recreate it', async () => {
  const f = await fixture();
  try {
    await f.click('New'); assert.equal(f.container.querySelector('output')?.textContent, 'facturas:new:view');
    await f.click('Saved'); assert.equal(f.router.state.location.pathname, '/facturas/9');
    await f.navigate(-1); assert.equal(f.router.state.location.pathname, '/facturas');
    await f.navigate(1); assert.equal(f.container.querySelector('output')?.textContent, 'facturas:9:view');
  } finally { await f.dispose(); }
});
