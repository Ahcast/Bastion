import { after, test } from 'node:test';
import assert from 'node:assert/strict';
import { Window } from 'happy-dom';
import { act } from 'react';
import { ApiClient } from '../src/api.js';
import { CoreContext } from '../src/ui.js';
import { ShortlistAlert, ShortlistProvider } from '../src/components/ShortlistAlerts.js';
import ListaCorta from '../src/pages/ListaCorta.js';
import type { ShortlistItem, TenderShortlist } from '../src/shortlist.js';
import { mainNavigation } from '../src/navigation.js';
import type { Access } from '../src/types.js';

const window = new Window({ url: 'https://bastion.example.test' });
for (const name of ['document', 'HTMLElement', 'Event', 'MouseEvent'] as const) {
  Object.defineProperty(globalThis, name, { configurable: true, value: window[name] });
}
Object.assign(globalThis, { window, IS_REACT_ACT_ENVIRONMENT: true });
const { createRoot } = await import('react-dom/client');
after(() => window.happyDOM.abort());

const access: Access = { is_system_administrator: false,
  modules: [{ module_id: 'procesos', display_name: 'Procesos' }],
  permissions: [{ module_id: 'procesos', scope_key: 'consulta', capability: 'ver' }] };
const item: ShortlistItem = {
  id: 'a'.repeat(64), version: 'b'.repeat(64), first_seen_at: '2026-11-06T18:00:00Z',
  updated_at: '2026-11-06T18:00:00Z', expediente: 'HOP6-ENEE', entidad: 'ENEE',
  unidad_compra: 'Unidad de Gestión de Proyectos BID', modalidad: 'Licitación pública internacional',
  tipo_adquisicion: 'Servicios', etapa: 'Recepción de Ofertas', objeto: 'Rehabilitación de compuertas',
  tipo_fuente: 'Recursos externos', fuente: 'Japón', fecha_recepcion: '2026-11-30T20:00:00Z',
  lugar_recepcion: 'UCP-BID-JICA/ENEE', coincidencias_interes: 'BID | UCP | JICA', posibles_interes: '',
  evidencia_interes: 'JICA (lugar_recepcion) UCP-BID-JICA/ENEE',
  url: 'http://sicc.honducompras.gob.hn/HC/Procesos/ProcesoHistorico.aspx?id=1',
  extraido_en: '2026-11-06T18:00:00Z',
};

async function workspace(userId: number, permitted = true) {
  let payload: TenderShortlist = { availability: 'available', checked_at: '2026-11-06T18:00:00Z',
    generated_at: '2026-11-06T18:00:00Z', scan_status: 'parcial', items: [item] };
  const calls: string[] = [];
  const api = new ApiClient('https://api.example.test', async url => {
    calls.push(String(url)); return Response.json(payload);
  });
  const container = document.createElement('div'); document.body.append(container);
  const root = createRoot(container);
  await act(async () => root.render(<CoreContext.Provider value={{ api, access: permitted ? access : { ...access, permissions: [] }, notify: () => {} }}>
    <ShortlistProvider userId={userId}><ShortlistAlert open={() => {}} /><ListaCorta /></ShortlistProvider>
  </CoreContext.Provider>));
  const click = async (label: string) => {
    const button = Array.from(container.querySelectorAll('button')).find(button => button.textContent === label);
    assert.ok(button, label); await act(async () => button.click());
  };
  return { container, calls, click, update: (next: TenderShortlist) => { payload = next; }, payload: () => payload,
    dispose: async () => { await act(async () => root.unmount()); container.remove(); } };
}

test('shortlist shows purchase unit, reception, evidence and an alert for partial data', async () => {
  const app = await workspace(11);
  try {
    assert.equal(app.calls.length, 1);
    assert.ok(app.calls[0]?.endsWith('/api/v1/procesos/lista-corta'));
    for (const text of ['Unidad de Gestión', 'UCP-BID-JICA/ENEE', 'Extracción parcial', 'Nueva', 'Por revisar']) {
      assert.ok(app.container.textContent?.includes(text), text);
    }
    assert.ok(app.container.querySelector('button[aria-label*="1 por revisar"]'));
    assert.equal(app.container.querySelector('a')?.getAttribute('rel'), 'noreferrer');
  } finally { await app.dispose(); }
});

test('reviews survive reopening per user and changes reopen the alert', async () => {
  let app = await workspace(22);
  await app.click('Marcar revisada');
  assert.equal(app.container.querySelector('button[aria-label*="por revisar"]'), null);
  await app.dispose();
  app = await workspace(22);
  try {
    assert.ok(app.container.textContent?.includes('Revisada'));
    app.update({ ...app.payload(), items: [{ ...item, version: 'c'.repeat(64), lugar_recepcion: 'Nueva oficina UCP' }] });
    await app.click('Actualizar');
    assert.ok(app.container.textContent?.includes('Actualizada'));
    assert.ok(app.container.textContent?.includes('Nueva oficina UCP'));
    assert.ok(app.container.querySelector('button[aria-label*="1 por revisar"]'));
  } finally { await app.dispose(); }
  app = await workspace(23);
  try { assert.ok(app.container.textContent?.includes('Nueva')); }
  finally { await app.dispose(); }
});

test('missing sources and restricted users do not appear as a successful empty scan', async () => {
  let app = await workspace(33);
  try {
    app.update({ ...app.payload(), availability: 'missing', generated_at: null, scan_status: null, items: [] });
    await app.click('Actualizar');
    assert.ok(app.container.textContent?.includes('todavía no tiene una extracción publicada'));
    assert.equal(app.container.querySelector('.shortlist-stats'), null);
  } finally { await app.dispose(); }
  app = await workspace(34, false);
  try { assert.equal(app.calls.length, 0); assert.equal(app.container.querySelector('.shortlist-alert'), null); }
  finally { await app.dispose(); }
  assert.ok(mainNavigation(access).some(entry => entry.id === 'lista_corta'));
  assert.ok(!mainNavigation({ ...access, permissions: [] }).some(entry => entry.id === 'lista_corta'));
});

test('background refresh uses a one minute interval', async context => {
  const delays: number[] = [];
  context.mock.method(globalThis, 'setInterval', (_callback: () => void, delay: number) => { delays.push(delay); return 1; });
  context.mock.method(globalThis, 'clearInterval', () => {});
  const app = await workspace(44);
  try { assert.deepEqual(delays, [60_000]); }
  finally { await app.dispose(); }
});
