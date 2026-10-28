import { after, test } from 'node:test';
import assert from 'node:assert/strict';
import { Window } from 'happy-dom';
import { renderToStaticMarkup } from 'react-dom/server';
import { ApiClient } from '../src/api.js';
import { CoreContext } from '../src/ui.js';
import { NavigationGuardProvider } from '../src/navigationGuard.js';
import Licitaciones from '../src/pages/Licitaciones.js';
import FormLibrary from '../src/pages/FormLibrary.js';
import { mainNavigation, settingsSections } from '../src/navigation.js';
import { fieldValue } from '../src/tenders.js';
import type { Access } from '../src/types.js';
import type { Expedient } from '../src/tenders.js';

const window = new Window();
for (const name of ['document', 'HTMLElement', 'HTMLInputElement', 'HTMLTextAreaElement', 'Event', 'MouseEvent', 'navigator'] as const) Object.defineProperty(globalThis, name, { configurable: true, value: window[name] });
Object.assign(globalThis, { window, IS_REACT_ACT_ENVIRONMENT: true });
const { act } = await import('react');
const { createRoot } = await import('react-dom/client');
after(() => window.happyDOM.abort());
const access: Access = { is_system_administrator: false, modules: [{ module_id: 'licitaciones', display_name: 'Licitaciones' }], permissions: [{ module_id: 'licitaciones', scope_key: 'expedientes', capability: 'ver' }] };

test('tenders need an active module and read permission; library publication is admin only', () => {
  for (const context of [access, { ...access, modules: [] }, { ...access, permissions: [] }]) {
    const allowed = context.modules.length > 0 && context.permissions.length > 0;
    assert.equal(mainNavigation(context).some(p => p.id === 'licitaciones'), allowed);
    assert.equal(settingsSections(context).some(p => p.id === 'licitaciones'), allowed);
    const markup = renderToStaticMarkup(<NavigationGuardProvider><CoreContext.Provider value={{ api: new ApiClient('https://example.test'), access: context, notify: () => {} }}><Licitaciones /></CoreContext.Provider></NavigationGuardProvider>);
    assert.equal(markup.includes('No tienes acceso'), !allowed);
    assert.ok(!markup.includes('Nuevo expediente'));
  }
  const libraryAccess = { ...access, modules: [{ module_id: 'colaboracion', display_name: 'Colaboración' }], permissions: [{ module_id: 'colaboracion', scope_key: '*', capability: 'ver' }, { module_id: 'colaboracion', scope_key: '*', capability: 'editar' }] };
  for (const admin of [false, true]) {
    const markup = renderToStaticMarkup(<NavigationGuardProvider><CoreContext.Provider value={{ api: new ApiClient('https://example.test'), access: { ...libraryAccess, is_system_administrator: admin }, notify: () => {} }}><FormLibrary /></CoreContext.Provider></NavigationGuardProvider>);
    assert.equal(markup.includes('Nuevo formulario'), admin);
  }
  assert.equal(fieldValue('decimal', '9007199254740993.123456789012'), '9007199254740993.123456789012');
  assert.equal(fieldValue('boolean', 'false'), false);
  assert.equal(fieldValue('boolean', ''), '');
});

test('expedients start empty; values preserve drafts on 412 and navigation asks before discarding', async () => {
  const calls: { path: string; init: RequestInit }[] = [];
  const record: Expedient = { id: 7, name: '', version: 2, state: 'borrador', forms: [{ id: 8, source: { id: 3, revision: 1, hash: 'a'.repeat(64), definition: { title: '<img src=x onerror=evil()>', section: 'legal', variables: [{ key: 'dato', label: 'Dato sintético', kind: 'text', required: true }] } }, values: {}, missing_required: ['dato'] }] };
  let stale = true, created = false;
  const api = new ApiClient('https://example.test', async (url, init = {}) => {
    const path = new URL(String(url)).pathname;
    calls.push({ path, init });
    if (path.endsWith('/archivos')) return Response.json({ items: [], expedient_version: 2, retained_bytes: 0 });
    if (init.method === 'PUT') return stale ? Response.json({ title: 'Precondition Failed', detail: 'Cambió la versión.' }, { status: 412 }) : Response.json({ ...record, version: 3 });
    if (init.method === 'POST') { created = true; assert.deepEqual(JSON.parse(String(init.body)), {}); assert.ok(new Headers(init.headers).get('idempotency-key')); return Response.json({ ...record, forms: [] }); }
    if (path === '/api/v1/licitaciones') return Response.json({ items: [{ id: 7, version: 2, state: 'borrador' }], page: 1, page_size: 25, has_more: false });
    return Response.json(created ? { ...record, forms: [] } : record);
  });
  const context = { ...access, permissions: [...access.permissions, { module_id: 'licitaciones', scope_key: 'expedientes', capability: 'editar' }] };
  const container = document.createElement('div'); document.body.append(container); const root = createRoot(container);
  const click = async (text: string, scope: ParentNode = container) => { const button = [...scope.querySelectorAll('button')].find(b => b.textContent === text); assert.ok(button, text); await act(async () => button.click()); };
  try {
    await act(async () => root.render(<NavigationGuardProvider><CoreContext.Provider value={{ api, access: context, notify: () => {} }}><Licitaciones /></CoreContext.Provider></NavigationGuardProvider>));
    await click('Abrir expediente'); await click('Documentación legal'); await click('Abrir valores');
    assert.equal(container.querySelectorAll('img').length, 0);
    const textarea = container.querySelector('textarea')!; assert.ok(textarea);
    const setter = Object.getOwnPropertyDescriptor(window.HTMLTextAreaElement.prototype, 'value')!.set!;
    await act(async () => { setter.call(textarea, 'Texto privado'); textarea.dispatchEvent(new Event('input', { bubbles: true })); });
    await act(async () => container.querySelector('form')!.dispatchEvent(new Event('submit', { bubbles: true, cancelable: true })));
    assert.equal(textarea.value, 'Texto privado'); assert.ok(container.textContent?.includes('Cambió la versión'));
    const write = calls.find(c => c.init.method === 'PUT')!;
    assert.equal(new Headers(write.init.headers).get('if-match'), '"2"');
    assert.deepEqual(JSON.parse(String(write.init.body)), { values: { dato: 'Texto privado' } });
    await click('Volver al listado'); assert.ok(document.body.textContent?.includes('Cambios sin guardar')); await click('Seguir editando', document.body);
    stale = false; await act(async () => container.querySelector('form')!.dispatchEvent(new Event('submit', { bubbles: true, cancelable: true })));
    await click('Volver al listado'); await click('Nuevo expediente');
    assert.ok(container.textContent?.includes('0 formularios incorporados'));
    assert.ok(!container.textContent?.includes('Entidad convocante'));
  } finally { await act(async () => root.unmount()); container.remove(); }
});

test('new library schemas have no default variables and require explicit administrative approval', async () => {
  let writes = 0;
  const api = new ApiClient('https://example.test', async (_url, init = {}) => { if (init.method === 'POST') writes++; return Response.json({ items: [], page: 1, page_size: 25, has_more: false }); });
  const container = document.createElement('div'); document.body.append(container); const root = createRoot(container);
  try {
    await act(async () => root.render(<NavigationGuardProvider><CoreContext.Provider value={{ api, access: { ...access, modules: [{ module_id: 'colaboracion', display_name: 'Colaboración' }], is_system_administrator: true }, notify: () => {} }}><FormLibrary /></CoreContext.Provider></NavigationGuardProvider>));
    const button = [...container.querySelectorAll('button')].find(b => b.textContent === 'Nuevo formulario')!;
    await act(async () => button.click());
    const dialog = document.body.querySelector('[role="dialog"]')!;
    assert.equal(dialog.querySelectorAll('legend').length, 0);
    assert.equal([...dialog.querySelectorAll('button')].find(b => b.textContent === 'Aprobar y publicar')?.disabled, true);
    assert.equal(writes, 0);
  } finally { await act(async () => root.unmount()); container.remove(); }
});

test('a form editor can select a Typst file and add its variables', async () => {
  const api = new ApiClient('https://example.test', async (url) => Response.json({ items: new URL(String(url)).pathname === '/api/v1/colaboracion' ? [{ id: 7, filename: 'formulario.typ', mime: 'text/plain', size_bytes: 30 }] : [], page: 1, page_size: 25, has_more: false }));
  const container = document.createElement('div'); document.body.append(container); const root = createRoot(container);
  try {
    await act(async () => root.render(<NavigationGuardProvider><CoreContext.Provider value={{ api, access: { ...access, modules: [{ module_id: 'colaboracion', display_name: 'Colaboración' }], permissions: [{ module_id: 'colaboracion', scope_key: 'empresa', capability: 'ver' }], is_system_administrator: true }, notify: () => {} }}><FormLibrary /></CoreContext.Provider></NavigationGuardProvider>));
    await act(async () => [...container.querySelectorAll('button')].find(b => b.textContent === 'Nuevo formulario')!.click());
    const dialog = document.body.querySelector('[role="dialog"]')!;
    await act(async () => [...dialog.querySelectorAll('button')].find(b => b.textContent === 'Seleccionar plantilla')!.click());
    await act(async () => [...dialog.querySelectorAll('button')].find(b => b.textContent === 'Añadir variable')!.click());
    assert.ok(dialog.textContent?.includes('archivo #7')); assert.equal(dialog.querySelectorAll('legend').length, 1);
  } finally { await act(async () => root.unmount()); container.remove(); }
});

test('expedient names persist with a version, keep text on conflicts and update headings and lists', async () => {
  let record: Expedient = { id: 9, name: '', version: 1, state: 'borrador', forms: [] };
  let conflict = true;
  const writes: RequestInit[] = [];
  const api = new ApiClient('https://example.test', async (url, init = {}) => {
    const path = new URL(String(url)).pathname;
    if (init.method === 'PATCH') {
      writes.push(init);
      if (conflict) return Response.json({ title: 'Precondition Failed' }, { status: 412 });
      const { name } = JSON.parse(String(init.body)) as { name: string };
      record = { ...record, name: name.trim(), version: 2 }; return Response.json(record);
    }
    return Response.json(path === '/api/v1/licitaciones' ? { items: [record], page: 1, page_size: 25, has_more: false } : record);
  });
  const context = { ...access, permissions: [...access.permissions, { module_id: 'licitaciones', scope_key: 'expedientes', capability: 'editar' }] };
  const container = document.createElement('div'); document.body.append(container); const root = createRoot(container);
  const click = async (text: string, scope: ParentNode = container) => { const button = [...scope.querySelectorAll('button')].find(b => b.textContent === text); assert.ok(button,text); await act(async () => button.click()); };
  try {
    await act(async () => root.render(<NavigationGuardProvider><CoreContext.Provider value={{ api, access: context, notify: () => {} }}><Licitaciones /></CoreContext.Provider></NavigationGuardProvider>));
    assert.ok(container.textContent?.includes('Sin nombre'));
    await click('Abrir expediente');
    const input = container.querySelector<HTMLInputElement>('input[name="expedient_name"]')!;
    assert.ok(input); assert.equal(input.maxLength,255);
    const setter = Object.getOwnPropertyDescriptor(window.HTMLInputElement.prototype,'value')!.set!;
    await act(async () => { setter.call(input,'  Mi oferta <script>  '); input.dispatchEvent(new Event('input',{bubbles:true})); });
    await act(async () => container.querySelector('form')!.dispatchEvent(new Event('submit',{bubbles:true,cancelable:true})));
    assert.equal(input.value,'  Mi oferta <script>  '); assert.ok(container.textContent?.includes('Conservamos tu nombre'));
    assert.equal(new Headers(writes[0]!.headers).get('if-match'),'"1"');
    assert.deepEqual(JSON.parse(String(writes[0]!.body)),{name:'  Mi oferta <script>  '});
    await click('Volver al listado'); assert.ok(document.body.textContent?.includes('Cambios sin guardar')); await click('Seguir editando',document.body);
    conflict=false;
    await act(async () => container.querySelector('form')!.dispatchEvent(new Event('submit',{bubbles:true,cancelable:true})));
    assert.equal(container.querySelector('h1')?.textContent,'Mi oferta <script>'); assert.equal(container.querySelectorAll('script').length,0);
    await click('Volver al listado'); assert.ok(container.textContent?.includes('Mi oferta <script>')); assert.ok(container.textContent?.includes('#9'));
    await act(async () => root.render(<NavigationGuardProvider><CoreContext.Provider value={{ api, access, notify: () => {} }}><Licitaciones /></CoreContext.Provider></NavigationGuardProvider>));
    await click('Abrir expediente'); assert.equal(container.querySelector('input[name="expedient_name"]'),null);
  } finally { await act(async () => root.unmount()); container.remove(); }
});
