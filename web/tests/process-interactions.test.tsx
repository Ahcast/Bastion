import { after, test } from 'node:test';
import assert from 'node:assert/strict';
import { registerHooks } from 'node:module';
import { Window } from 'happy-dom';
import { act } from 'react';
import { createRoot } from 'react-dom/client';
import { ApiClient } from '../src/api.js';
import { CoreContext } from '../src/ui.js';
import type { ProcessPage, ProcessRecord } from '../src/processes.js';

const css = registerHooks({ resolve(specifier, context, next) { return specifier.endsWith('.css') ? { url: 'data:text/javascript,', shortCircuit: true } : next(specifier, context); } });
const { default: Procesos } = await import('../src/pages/Procesos.js');
css.deregister();
const window = new Window();
for (const name of ['document', 'HTMLElement', 'FormData', 'Event', 'MouseEvent'] as const) Object.defineProperty(globalThis, name, { configurable: true, value: window[name] });
Object.assign(globalThis, { window, IS_REACT_ACT_ENVIRONMENT: true });
Object.defineProperty(document, 'visibilityState', { configurable: true, value: 'visible' });
after(() => window.happyDOM.abort());
const sample: ProcessRecord = { proceso_id: 'one', expediente: 'EXP-001', entidad: 'Salud', fecha_recepcion: '2099-10-23T05:59:59Z', puntaje: 87, rubro: 'Equipos', resumen_corto: 'Laboratorio', source: { descripcion: 'Detalle completo' }, marked: false, note: 'Nota inicial', version: 1 };
const tick = async () => { await act(async () => { await new Promise(resolve => setTimeout(resolve, 0)); }); };

async function workspace(expires = false) {
  const calls: { path: string; method: string; match: string | null }[] = [];
  let record = { ...sample }, fail = 0, listCalls = 0;
  let release: (() => void) | undefined;
  const notices: string[] = [];
  const api = new ApiClient('https://api.example.test', async (input, init) => {
    const path = new URL(String(input)).pathname, method = init?.method ?? 'GET';
    calls.push({ path, method, match: new Headers(init?.headers).get('If-Match') });
    let data: unknown;
    if (method === 'PATCH') {
      if (fail) { const status = fail; fail = 0; return Response.json({ title: 'Conflict', detail: 'Cambio de otro usuario' }, { status }); }
      assert.equal(new Headers(init?.headers).get('If-Match'), `"${record.version}"`);
      record = { ...record, ...JSON.parse(String(init?.body)), version: record.version + 1 };
      data = { marked: record.marked, note: record.note, version: record.version };
    } else if (path.endsWith('/status')) data = null;
    else if (path.endsWith('/entities')) data = { entities: ['Salud'] };
    else if (path.endsWith('/rows')) {
      listCalls++;
      const snapshot = { ...record };
      if (expires && listCalls > 1) await new Promise<void>(resolve => { release = resolve; });
      const items = expires ? (listCalls === 1 ? [{ ...sample, proceso_id: 'expired', fecha_recepcion: '2000-01-01T00:00:00Z' }, snapshot] : [snapshot]) : (new URL(String(input)).searchParams.has('marked') && !snapshot.marked ? [] : [snapshot]);
      data = { items: items.map(row => ({ ...row, source: {} })), total: items.length, server_time: new Date().toISOString(), page: 1, page_size: 25, has_more: false } satisfies ProcessPage;
    } else data = { ...record };
    return Response.json(data);
  });
  const container = document.createElement('div'); document.body.append(container);
  const root = createRoot(container);
  await act(async () => root.render(<CoreContext.Provider value={{ api, access: { is_system_administrator: true, modules: [{ module_id: 'procesos', display_name: 'Procesos' }], permissions: [] }, notify: text => notices.push(text) }}><Procesos /></CoreContext.Provider>));
  const click = async (element: Element | null) => { assert.ok(element); await act(async () => { element.dispatchEvent(new MouseEvent('click', { bubbles: true })); }); };
  const mark = () => container.querySelector<HTMLInputElement>('tbody input[type="checkbox"]')!;
  const open = () => click(container.querySelector('tbody button[aria-expanded]'));
  const dispose = async () => { release?.(); await act(async () => root.unmount()); container.remove(); };
  return { container, calls, notices, click, mark, open, dispose, release: () => release?.(), listCalls: () => listCalls, conflict: () => { record = { ...record, marked: true, version: record.version + 1 }; fail = 412; }, failure: () => { fail = 500; } };
}

test('returning to the browser tab and marking retain the detail and note draft without list requests', async () => {
  const app = await workspace();
  try {
    await app.open();
    const detail = app.container.querySelector('.process-detail-row');
    const note = app.container.querySelector('textarea')!; note.value = 'Borrador sin guardar';
    const count = app.calls.length;
    await act(async () => { document.dispatchEvent(new Event('visibilitychange')); });
    assert.equal(app.calls.length, count);
    await app.click(app.mark());
    assert.equal(app.mark().checked, true);
    assert.equal(app.container.querySelector('.process-detail-row'), detail);
    assert.equal(app.container.querySelector('textarea'), note);
    assert.equal(note.value, 'Borrador sin guardar');
    assert.equal(app.listCalls(), 1);
    await act(async () => { note.form!.dispatchEvent(new Event('submit', { bubbles: true, cancelable: true })); });
    assert.equal(app.calls.filter(call => call.method === 'PATCH').at(-1)?.match, '"2"');
    assert.equal(app.listCalls(), 1);
    assert.equal(note.value, 'Borrador sin guardar');
    assert.ok(app.notices.includes('Nota compartida guardada.'));
  } finally { await app.dispose(); }
});

test('failed marks retain the draft; version conflicts refresh only the affected review before an explicit retry', async () => {
  const app = await workspace();
  try {
    await app.open(); const note = app.container.querySelector('textarea')!; note.value = 'Mi borrador';
    app.failure(); await app.click(app.mark());
    assert.equal(app.mark().checked, false); assert.equal(app.listCalls(), 1);
    app.conflict(); await app.click(app.mark());
    assert.equal(app.mark().checked, true); assert.equal(app.listCalls(), 1);
    assert.equal(app.container.querySelector('textarea'), note); assert.equal(note.value, 'Mi borrador');
    await act(async () => { note.form!.dispatchEvent(new Event('submit', { bubbles: true, cancelable: true })); });
    assert.equal(app.calls.filter(call => call.method === 'PATCH').at(-1)?.match, '"2"');
  } finally { await app.dispose(); }
});

test('unmarking in Solo marcados updates the filtered count and list', async () => {
  const app = await workspace();
  try {
    await app.click(app.mark());
    const filter = Array.from(app.container.querySelectorAll('label')).find(label => label.textContent?.includes('Solo marcados'))!.querySelector('input');
    await app.click(filter);
    assert.equal(app.mark().checked, true);
    await app.click(app.mark());
    assert.equal(app.container.querySelectorAll('tbody tr').length, 0);
    assert.ok(app.container.textContent?.includes('0 resultados'));
  } finally { await app.dispose(); }
});

test('expiry removes expired rows and retains live rows and detail while replacement results load', async () => {
  const app = await workspace(true);
  try {
    await tick();
    assert.equal(app.listCalls(), 2);
    assert.ok(!app.container.textContent?.includes('Consultando…'));
    assert.equal(app.container.querySelectorAll('tbody tr').length, 1);
    const row = app.container.querySelector('tbody tr');
    await app.open(); const note = app.container.querySelector('textarea')!; note.value = 'Pendiente';
    await app.click(app.mark());
    app.release(); await tick();
    assert.equal(app.container.querySelector('tbody tr'), row);
    assert.equal(app.container.querySelector('textarea'), note);
    assert.equal(note.value, 'Pendiente');
    assert.equal(app.mark().checked, true, 'an older list response cannot undo a confirmed mark');
  } finally { await app.dispose(); }
});
