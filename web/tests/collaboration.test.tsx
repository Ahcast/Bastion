import { after, test } from 'node:test';
import assert from 'node:assert/strict';
import { Window } from 'happy-dom';
import { renderToStaticMarkup } from 'react-dom/server';
import { ApiClient } from '../src/api.js';
import { CoreContext } from '../src/ui.js';
import { NavigationGuardProvider } from '../src/navigationGuard.js';
import Colaboracion from '../src/pages/Colaboracion.js';
import CollaborationSettings from '../src/pages/CollaborationSettings.js';
import { contentBlob, fileBase64, MAX_FILE_BYTES } from '../src/collaboration.js';
import { mainNavigation, settingsSections } from '../src/navigation.js';
import type { Access } from '../src/types.js';

const window = new Window();
for (const name of ['document', 'HTMLElement', 'HTMLInputElement', 'FormData', 'Event', 'MouseEvent', 'navigator', 'FileReader', 'File'] as const) Object.defineProperty(globalThis, name, { configurable: true, value: window[name] });
Object.assign(globalThis, { window, IS_REACT_ACT_ENVIRONMENT: true });
const { act } = await import('react');
const { createRoot } = await import('react-dom/client');
after(() => window.happyDOM.abort());

const access: Access = { is_system_administrator: false, modules: [{ module_id: 'colaboracion', display_name: 'Colaboración' }], permissions: [{ module_id: 'colaboracion', scope_key: 'persona:7', capability: 'ver' }] };
test('collaboration navigation accepts a person-specific grant and requires an active module', () => {
  for (const active of [true, false]) {
    const context = { ...access, modules: active ? access.modules : [] };
    const html = renderToStaticMarkup(<NavigationGuardProvider><CoreContext.Provider value={{ api: new ApiClient('https://example.test'), access: context, notify: () => {} }}><Colaboracion /></CoreContext.Provider></NavigationGuardProvider>);
    assert.equal(mainNavigation(context).some(entry => entry.id === 'colaboracion'), active);
    assert.equal(settingsSections(context).some(entry => entry.id === 'colaboracion'), active);
    assert.equal(html.includes('No tienes acceso'), !active);
    assert.equal(html.includes('Guardar / subir'), false);
  }
});

test('trash allows restoration and permanent purge only after a separate confirmation', async () => {
  let trashed = false, purged = false;
  const calls: { path: string; method: string; body: unknown }[] = [];
  const record = { id: 18, scope: 'empresa', id_persona: null, title: 'Contrato', notes: '', filename: 'a.pdf', mime: 'application/pdf', size_bytes: 40, created_at: '2026-10-23T12:00:00Z', trashed_at: null, purge_after: '2026-11-22T12:00:00Z' };
  const policy = { max_file_mb: 50, trash_retention_days: 30, allowed_extensions: ['pdf'], blocked_extensions: ['exe'] };
  const api = new ApiClient('https://example.test', async (url, init = {}) => {
    const parsed = new URL(String(url)), method = init.method ?? 'GET';
    calls.push({ path: parsed.pathname, method, body: init.body ? JSON.parse(String(init.body)) : null });
    if (method === 'DELETE') { trashed = true; return new Response(null, { status: 204 }); }
    if (parsed.pathname.endsWith('/restore')) { trashed = false; return new Response(null, { status: 204 }); }
    if (parsed.pathname.endsWith('/purge')) { purged = true; return new Response(null, { status: 204 }); }
    if (parsed.pathname.endsWith('/settings')) return Response.json(policy);
    return Response.json({ items: !purged && (parsed.searchParams.get('trash') === 'true') === trashed ? [record] : [], page: 1, page_size: 25, has_more: false });
  });
  const container = document.createElement('div'); document.body.append(container); const root = createRoot(container);
  const context: Access = { ...access, permissions: ['ver', 'eliminar'].map(capability => ({ module_id: 'colaboracion', scope_key: '*', capability })) };
  const click = async (label: string, scope: ParentNode = container) => { const button = [...scope.querySelectorAll('button')].find(b => b.textContent === label); assert.ok(button, label); await act(async () => button.click()); };
  try {
    await act(async () => root.render(<NavigationGuardProvider><CoreContext.Provider value={{ api, access: context, notify: () => {} }}><Colaboracion /></CoreContext.Provider></NavigationGuardProvider>));
    await click('Eliminar'); await click('Mover a papelera', document.body); await click('Papelera');
    assert.equal([...container.querySelectorAll('button')].some(b => b.textContent === 'Descargar'), false);
    await click('Restaurar'); assert.equal(trashed, false);
    await click('Archivos'); await click('Eliminar'); await click('Mover a papelera', document.body); await click('Papelera');
    await click('Eliminar definitivamente');
    assert.equal(calls.filter(call => call.path.endsWith('/purge')).length, 0);
    await click('Confirmar borrado definitivo', document.body);
    assert.deepEqual(calls.find(call => call.path.endsWith('/purge'))?.body, { confirm_id: 18 });
    assert.ok(purged);
  } finally { await act(async () => root.unmount()); container.remove(); }
});

test('module settings save file limits, retention and normalized allow/block lists', async () => {
  let policy = { max_file_mb: 50, trash_retention_days: 30, allowed_extensions: ['pdf', 'zip'], blocked_extensions: ['exe'] };
  const api = new ApiClient('https://example.test', async (_url, init = {}) => {
    if (init.method === 'PUT') { policy = JSON.parse(String(init.body)); return new Response(null, { status: 204 }); }
    return Response.json(policy);
  });
  const container = document.createElement('div'); document.body.append(container); const root = createRoot(container);
  try {
    await act(async () => root.render(<NavigationGuardProvider><CoreContext.Provider value={{ api, access: { ...access, is_system_administrator: true }, notify: () => {} }}><CollaborationSettings /></CoreContext.Provider></NavigationGuardProvider>));
    const inputs = container.querySelectorAll<HTMLInputElement>('input');
    const setter = Object.getOwnPropertyDescriptor(window.HTMLInputElement.prototype, 'value')!.set!;
    await act(async () => { for (const [index, value] of [[0, '70'], [1, '15'], [2, '.PDF, zip, PDF']] as const) { setter.call(inputs[index], value); inputs[index]!.dispatchEvent(new Event('input', { bubbles: true })); } });
    await act(async () => container.querySelector('form')!.dispatchEvent(new Event('submit', { bubbles: true, cancelable: true })));
    assert.equal(policy.max_file_mb, 70); assert.equal(policy.trash_retention_days, 15); assert.deepEqual(policy.allowed_extensions, ['pdf', 'zip']);
  } finally { await act(async () => root.unmount()); container.remove(); }
});
test('active content cannot acquire a preview origin through its declared MIME', async () => {
  assert.throws(() => contentBlob({ filename: 'evil.svg', mime: 'image/svg+xml', content_base64: btoa('<svg onload="alert(1)">') }, true));
  const download = contentBlob({ filename: 'evil.html', mime: 'text/html', content_base64: btoa('<script>evil()</script>') }, false);
  assert.equal(download.type, 'application/octet-stream');
  assert.throws(() => contentBlob({ filename: 'a', mime: 'text/plain', content_base64: 'A'.repeat(Math.ceil(MAX_FILE_BYTES / 3) * 4 + 1) }, true));
  await assert.rejects(fileBase64({ size: 0 } as File));
  await assert.rejects(fileBase64({ size: MAX_FILE_BYTES + 1 } as File));
});



test('person folders filter requests and upload targets; deletion requires explicit confirmation and its own permission', async () => {
  const calls: { url: URL; init: RequestInit }[] = [];
  const record = { mime: 'text/plain', trashed_at: null, purge_after: null, id: 9, scope: 'persona', id_persona: 7, title: '<script>Privado</script>', notes: 'Contrato', filename: 'contrato.txt', size_bytes: 2, created_at: '2026-10-23T12:00:00Z' };
  const api = new ApiClient('https://example.test', async (url, init = {}) => {
    const parsed = new URL(String(url)); calls.push({ url: parsed, init });
    if (init.method === 'DELETE') return new Response(null, { status: 204 });
    if (init.method === 'POST') { const body = JSON.parse(String(init.body)); assert.equal(body.scope, 'persona'); assert.equal(body.id_persona, 7); assert.equal(body.title, 'Contrato nuevo'); assert.ok(new Headers(init.headers).get('idempotency-key')); return Response.json(record, { status: 201 }); }
    if (parsed.pathname.endsWith('/settings')) return Response.json({ max_file_mb: 50, trash_retention_days: 30, allowed_extensions: ['txt'], blocked_extensions: ['exe'] });
    if (parsed.pathname.endsWith('/personas')) return Response.json({ items: [{ id: 7, full_name: 'Ana Test' }], page: 1, page_size: 25, has_more: false });
    return Response.json({ items: [record], page: 1, page_size: 25, has_more: false });
  });
  const permissions = ['ver', 'editar', 'eliminar'].map(capability => ({ module_id: 'colaboracion', scope_key: 'persona:7', capability }));
  const container = document.createElement('div'); document.body.append(container); const root = createRoot(container);
  const render = async (canDelete: boolean) => act(async () => root.render(<NavigationGuardProvider><CoreContext.Provider value={{ api, access: { ...access, permissions: permissions.filter(p => canDelete || p.capability !== 'eliminar') }, notify: () => {} }}><Colaboracion /></CoreContext.Provider></NavigationGuardProvider>));
  const click = async (label: string, scope: ParentNode = container) => { const button = [...scope.querySelectorAll('button')].find(b => b.textContent === label); assert.ok(button, label); await act(async () => button.click()); };
  try {
    await render(false);
    assert.equal(container.querySelector('script'), null);
    assert.equal([...container.querySelectorAll('button')].some(b => b.textContent === 'Eliminar'), false);
    assert.ok(container.querySelector<HTMLButtonElement>('[aria-label="Carpetas"] button')?.disabled);
    await click('Ana Test');
    assert.ok(calls.some(call => call.url.searchParams.get('id_persona') === '7'));
    const title = container.querySelector<HTMLInputElement>('[name="title"]')!;
    await act(async () => { Object.getOwnPropertyDescriptor(window.HTMLInputElement.prototype, 'value')!.set!.call(title, 'Contrato nuevo'); title.dispatchEvent(new Event('input', { bubbles: true })); });
    const file = new window.File(['Hi'], 'contrato.txt', { type: 'text/plain' });
    const input = container.querySelector<HTMLInputElement>('[name="file"]')!;
    Object.defineProperty(input, 'files', { configurable: true, value: [file] });
    await act(async () => input.dispatchEvent(new Event('change', { bubbles: true })));
    await act(async () => { container.querySelector('.collaboration-upload form')!.dispatchEvent(new Event('submit', { bubbles: true, cancelable: true })); await window.happyDOM.whenAsyncComplete(); });
    assert.equal(calls.filter(c => c.init.method === 'POST').length, 1);
    await render(true); await click('Eliminar');
    assert.equal(calls.filter(c => c.init.method === 'DELETE').length, 0);
    await click('Mover a papelera', document.body);
    const deletion = calls.find(c => c.init.method === 'DELETE'); assert.ok(deletion);
    assert.deepEqual(JSON.parse(String(deletion.init.body)), { confirm_id: 9 });
  } finally { await act(async () => root.unmount()); container.remove(); }
});
