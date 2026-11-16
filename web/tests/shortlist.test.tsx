import { after, test } from 'node:test';
import assert from 'node:assert/strict';
import { Window } from 'happy-dom';
import { act } from 'react';
import { ApiClient } from '../src/api.js';
import { CoreContext } from '../src/ui.js';
import { ShortlistProvider } from '../src/components/ShortlistAlerts.js';
import LicitacionesProcesos from '../src/components/LicitacionesProcesos.js';
import type { ShortlistItem, TenderShortlist } from '../src/shortlist.js';
import { sortShortlist } from '../src/shortlist.js';
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

async function workspace(userId: number, permitted = true, initialItems?: ShortlistItem[]) {
  let payload: TenderShortlist = { availability: 'available', checked_at: '2026-11-06T18:00:00Z',
    generated_at: '2026-11-06T18:00:00Z', scan_status: 'parcial', items: [item, { ...item, id: 'd'.repeat(64),
      expediente: 'LPN sin prioridad', coincidencias_interes: '', evidencia_interes: '' }] };
  if (initialItems) payload.items = initialItems;
  const calls: string[] = [];
  const api = new ApiClient('https://api.example.test', async url => {
    calls.push(String(url)); return Response.json(payload);
  });
  const container = document.createElement('div'); document.body.append(container);
  const root = createRoot(container);
  await act(async () => root.render(<CoreContext.Provider value={{ api, access: permitted ? access : { ...access, permissions: [] }, notify: () => {} }}>
    <ShortlistProvider userId={userId}><LicitacionesProcesos /></ShortlistProvider>
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
    assert.ok(app.calls[0]?.endsWith('/api/v1/procesos/licitaciones'));
    for (const text of ['Unidad de Gestión', 'UCP-BID-JICA/ENEE', 'Extracción parcial', 'Nueva', 'Por revisar']) {
      assert.ok(app.container.textContent?.includes(text), text);
    }
    assert.equal(app.container.querySelector('.shortlist-stats strong')?.textContent, '1');
    assert.deepEqual(Array.from(app.container.querySelectorAll('.shortlist-code')).map(el => el.textContent), ['HOP6-ENEE', 'LPN sin prioridad']);
    assert.equal(app.container.querySelector('a')?.getAttribute('rel'), 'noreferrer');
  } finally { await app.dispose(); }
});

test('reviews survive reopening per user and changes reopen the alert', async () => {
  let app = await workspace(22);
  await app.click('Marcar revisada');
  assert.equal(app.container.querySelector('.shortlist-stats strong')?.textContent, '0');
  await app.dispose();
  app = await workspace(22);
  try {
    assert.ok(app.container.textContent?.includes('Revisada'));
    app.update({ ...app.payload(), items: [{ ...item, version: 'c'.repeat(64), lugar_recepcion: 'Nueva oficina UCP' }] });
    await app.click('Actualizar');
    assert.ok(app.container.textContent?.includes('Actualizada'));
    assert.ok(app.container.textContent?.includes('Nueva oficina UCP'));
    assert.equal(app.container.querySelector('.shortlist-stats strong')?.textContent, '1');
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
    assert.ok(app.container.textContent?.includes('Todavía no hay una extracción de licitaciones publicada'));
    assert.equal(app.container.querySelector('.shortlist-stats'), null);
  } finally { await app.dispose(); }
  app = await workspace(34, false);
  try { assert.equal(app.calls.length, 0); assert.equal(app.container.querySelector('.shortlist-stats'), null); }
  finally { await app.dispose(); }
  assert.deepEqual(mainNavigation(access).map(entry => entry.id), ['inicio', 'procesos', 'ajustes']);
});

test('background refresh uses a one minute interval', async context => {
  const delays: number[] = [];
  context.mock.method(globalThis, 'setInterval', (_callback: () => void, delay: number) => { delays.push(delay); return 1; });
  context.mock.method(globalThis, 'clearInterval', () => {});
  const app = await workspace(44);
  try { assert.deepEqual(delays, [60_000]); }
  finally { await app.dispose(); }
});

test('shortlist ordering keeps pending scores last, accepts zero, preserves ties and never mutates the feed', () => {
  const scored = (id: string, puntaje: number | null, fecha_recepcion: string): ShortlistItem => ({ ...item, id,
    fecha_recepcion, clasificacion: puntaje === null ? null : { puntaje, rubro: 'Equipos', resumen: 'Resumen', items: [] } });
  const values = [scored('pending', null, '2026-11-19T18:00:00Z'), scored('zero', 0, '2026-11-19T10:00:00-06:00'),
    scored('high', 95, '2026-11-20T18:00:00Z'), scored('tied', 95, '2026-11-19T18:00:00Z')];
  const original = [...values];
  const ids = (order: Parameters<typeof sortShortlist>[1]) => sortShortlist(values, order).map(value => value.id);
  assert.deepEqual(ids('score_desc'), ['high', 'tied', 'zero', 'pending']);
  assert.deepEqual(ids('score_asc'), ['zero', 'high', 'tied', 'pending']);
  assert.deepEqual(ids('date_asc'), ['zero', 'pending', 'tied', 'high']);
  assert.deepEqual(ids('date_desc'), ['high', 'pending', 'tied', 'zero']);
  assert.deepEqual(ids('priority'), original.map(value => value.id));
  assert.deepEqual(values, original);
});

test('the ordering selector sorts all filtered tenders before pagination and survives refresh and review', async () => {
  const values: ShortlistItem[] = Array.from({ length: 30 }, (_, index) => ({ ...item,
    id: index.toString(16).padStart(64, '0'), expediente: `EXP-${index}`,
    fecha_recepcion: new Date(Date.UTC(2026, 10, 7 + index)).toISOString(),
    clasificacion: index === 1 ? null : { puntaje: index * 3, rubro: 'Equipos', resumen: 'Resumen', items: [] },
  }));
  const app = await workspace(55, true, values);
  try {
    const selector = Array.from(app.container.querySelectorAll('select')).find(el => el.parentElement?.textContent?.startsWith('Ordenar por'))!;
    assert.ok(selector);
    const codes = () => Array.from(app.container.querySelectorAll('.shortlist-code')).map(el => el.textContent);
    const select = async (value: string) => { await act(async () => { selector.value = value; selector.dispatchEvent(new Event('change', { bubbles: true })); }); };
    assert.equal(codes().length, 25);
    assert.equal(codes()[0], 'EXP-0');
    await select('score_desc');
    assert.equal(codes()[0], 'EXP-29'); // Originally outside the first page.
    await app.click('Mostrar más');
    assert.equal(codes().length, 30);
    assert.equal(codes().at(-1), 'EXP-1');
    await select('score_asc');
    assert.equal(codes().length, 25);
    assert.equal(codes()[0], 'EXP-0');
    await select('date_asc');
    assert.equal(codes()[0], 'EXP-0');
    await select('date_desc');
    assert.equal(codes()[0], 'EXP-29');
    app.update({ ...app.payload(), items: [...values].reverse() });
    await app.click('Actualizar');
    assert.equal(selector.value, 'date_desc');
    assert.equal(codes()[0], 'EXP-29');
    const search = app.container.querySelector<HTMLInputElement>('input[type="search"]')!;
    await act(async () => { Object.getOwnPropertyDescriptor(window.HTMLInputElement.prototype, 'value')!.set!.call(search, 'EXP-29'); search.dispatchEvent(new Event('input', { bubbles: true })); });
    assert.deepEqual(codes(), ['EXP-29']);
    await app.click('Marcar resultados revisados');
    assert.equal(selector.value, 'date_desc');
    assert.deepEqual(codes(), ['EXP-29']);
  } finally { await app.dispose(); }
});
