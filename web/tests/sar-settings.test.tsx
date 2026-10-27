import { after, test } from 'node:test';
import assert from 'node:assert/strict';
import { Window } from 'happy-dom';
import { ApiClient } from '../src/api.js';
import { CoreContext } from '../src/ui.js';
import { NavigationGuardProvider } from '../src/navigationGuard.js';
import SarSettings from '../src/pages/SarSettings.js';
import type { SarRecord } from '../src/pages/SarSettings.js';
import type { Access } from '../src/types.js';

const window = new Window();
for (const name of ['document', 'HTMLElement', 'HTMLInputElement', 'HTMLTextAreaElement', 'FormData', 'Event', 'MouseEvent', 'KeyboardEvent', 'navigator'] as const) Object.defineProperty(globalThis, name, { configurable: true, value: window[name] });
Object.assign(globalThis, { window, IS_REACT_ACT_ENVIRONMENT: true });
const { act } = await import('react');
const { createRoot } = await import('react-dom/client');
after(() => window.happyDOM.abort());
const access: Access = { is_system_administrator: true, modules: [], permissions: [] };
const initial: SarRecord = { settings: { limit_range: '', limit_date: null, current_invoice: '', sar_text: '' }, version: 'a'.repeat(64), status: { days_remaining: null, invoices_remaining: null, range_expired: false, date_expired: false } };

test('SAR configuration is not fetched by an ordinary user, even with Core grants', async () => {
  let calls = 0;
  const api = new ApiClient('https://example.test', async () => { calls++; throw new Error('Must not query'); });
  const container = document.createElement('div'); document.body.append(container); const root = createRoot(container);
  try {
    await act(async () => root.render(<CoreContext.Provider value={{ api, access: { ...access, is_system_administrator: false, permissions: [{ module_id: 'core', scope_key: '*', capability: 'editar' }] }, notify: () => {} }}><SarSettings /></CoreContext.Provider>));
    assert.equal(calls, 0);
    assert.ok(container.textContent?.includes('Solo el administrador'));
    assert.equal(container.querySelector('form'), null);
  } finally { await act(async () => root.unmount()); container.remove(); }
});

test('SAR saves all four fields atomically, uses the server status and retains edits on a stale version', async () => {
  let record = initial, writes = 0, reads = 0;
  const notices: string[] = [];
  const api = new ApiClient('https://example.test', async (url, init = {}) => {
    assert.equal(new URL(String(url)).pathname, '/api/v1/settings/sar');
    if (init.method === 'PUT') {
      writes++;
      const body = JSON.parse(String(init.body));
      assert.deepEqual(Object.keys(body).sort(), ['settings', 'version']);
      assert.equal(body.version, record.version);
      if (writes === 2) return Response.json({ detail: 'Version changed' }, { status: 412 });
      if (writes === 3) { assert.equal(body.settings.sar_text, ' Texto de prueba '); return Response.json(record); }
      assert.deepEqual(body.settings, { limit_range: '100', limit_date: '2026-10-23', current_invoice: '90', sar_text: 'Texto de prueba' });
      record = { settings: body.settings, version: 'b'.repeat(64), status: { days_remaining: 0, invoices_remaining: 10, date_expired: false, range_expired: false } };
      return Response.json(record);
    }
    reads++; return Response.json(record);
  });
  const container = document.createElement('div'); document.body.append(container); const root = createRoot(container);
  const button = (text: string) => [...document.querySelectorAll<HTMLButtonElement>('button')].find(element => element.textContent === text)!;
  const change = async (name: string, value: string) => {
    const field = container.querySelector<HTMLInputElement | HTMLTextAreaElement>(`[name="${name}"]`)!;
    const prototype = field.tagName === 'TEXTAREA' ? window.HTMLTextAreaElement.prototype : window.HTMLInputElement.prototype;
    await act(async () => { Object.getOwnPropertyDescriptor(prototype, 'value')!.set!.call(field, value); field.dispatchEvent(new Event('input', { bubbles: true })); field.dispatchEvent(new Event('change', { bubbles: true })); });
  };
  const submit = async () => { await act(async () => container.querySelector('form')!.dispatchEvent(new Event('submit', { bubbles: true, cancelable: true }))); };
  try {
    await act(async () => root.render(<NavigationGuardProvider><CoreContext.Provider value={{ api, access, notify: text => notices.push(text) }}><SarSettings /></CoreContext.Provider></NavigationGuardProvider>));
    assert.ok(container.textContent?.includes('Sin numeración'));
    assert.equal(button('Guardar SAR').disabled, true);
    await change('limit_range', '100'); await change('limit_date', '2026-10-23'); await change('current_invoice', '90'); await change('sar_text', 'Texto de prueba');
    await submit();
    assert.equal(writes, 1);
    assert.equal(notices[0], 'Configuración SAR guardada.');
    assert.ok(container.querySelector('[aria-label="Estado SAR guardado"]')?.textContent?.includes('Facturas restantes: 10'));
    assert.ok(container.textContent?.includes('la emisión se confirma desde Facturas'));
    await change('current_invoice', '91'); await submit();
    assert.equal(writes, 2);
    assert.equal(container.querySelector<HTMLInputElement>('[name="current_invoice"]')?.value, '91');
    assert.ok(container.textContent?.includes('Tus cambios se conservan'));
    assert.equal(button('Guardar SAR').disabled, true);
    await act(async () => button('Recargar configuración SAR').click());
    assert.equal(reads, 1);
    assert.ok(document.querySelector('[role="dialog"]'));
    await act(async () => button('Seguir editando').click());
    assert.equal(container.querySelector<HTMLInputElement>('[name="current_invoice"]')?.value, '91');
    await act(async () => button('Recargar configuración SAR').click());
    await act(async () => button('Descartar y continuar').click());
    assert.equal(reads, 2);
    assert.equal(container.querySelector<HTMLInputElement>('[name="current_invoice"]')?.value, '90');
    await change('sar_text', 'Sin guardar');
    await act(async () => button('Cancelar cambios').click());
    assert.equal(container.querySelector<HTMLTextAreaElement>('[name="sar_text"]')?.value, 'Texto de prueba');
    assert.equal(writes, 2);
    await change('sar_text', ' Texto de prueba '); await submit();
    assert.equal(writes, 3);
    assert.equal(container.querySelector<HTMLTextAreaElement>('[name="sar_text"]')?.value, 'Texto de prueba');
    assert.equal(button('Guardar SAR').disabled, true);
  } finally { await act(async () => root.unmount()); container.remove(); }
});
