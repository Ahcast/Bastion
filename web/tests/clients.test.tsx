import { after, test } from 'node:test';
import assert from 'node:assert/strict';
import { Window } from 'happy-dom';
import { renderToStaticMarkup } from 'react-dom/server';
import { ApiClient } from '../src/api.js';
import { CoreContext } from '../src/ui.js';
import { NavigationGuardProvider } from '../src/navigationGuard.js';
import Clientes from '../src/pages/Clientes.js';
import { mainNavigation, settingsSections } from '../src/navigation.js';
import type { Access } from '../src/types.js';
import { emptyClient } from '../src/clients.js';

const access: Access = { is_system_administrator: false, modules: [{ module_id: 'clientes', display_name: 'Clientes' }], permissions: [{ module_id: 'clientes', scope_key: 'catalogo', capability: 'ver' }] };
test('client navigation, settings and writes follow active module and separate permissions', () => {
  for (const [active, edit] of [[true, true], [true, false], [false, true]]) {
    const permissions = [...access.permissions, ...(edit ? [{ module_id: 'clientes', scope_key: 'catalogo', capability: 'editar' }] : [])];
    const context = { ...access, modules: active ? access.modules : [], permissions };
    const html = renderToStaticMarkup(<NavigationGuardProvider><CoreContext.Provider value={{ api: new ApiClient('https://example.test'), access: context, notify: () => {} }}><Clientes /></CoreContext.Provider></NavigationGuardProvider>);
    assert.equal(html.includes('>Nuevo<'), active && edit);
    assert.equal(html.includes('No tienes acceso'), !active);
    assert.equal(mainNavigation(context).some(page => page.id === 'clientes'), active);
    assert.equal(settingsSections(context).some(page => page.id === 'clientes'), active);
    assert.equal(html.includes('Eliminar'), false);
  }
});

const window = new Window();
for (const name of ['document', 'HTMLElement', 'HTMLInputElement', 'HTMLTextAreaElement', 'FormData', 'Event', 'MouseEvent', 'navigator'] as const) Object.defineProperty(globalThis, name, { configurable: true, value: window[name] });
Object.assign(globalThis, { window, IS_REACT_ACT_ENVIRONMENT: true });
const { act } = await import('react');
const { createRoot } = await import('react-dom/client');
after(() => window.happyDOM.abort());

test('client edits send only seven fields, use versions, retain drafts on 412 and paginate', async () => {
  const calls: { url: URL; init: RequestInit }[] = [], notices: string[] = [];
  const client = { ...emptyClient(), id: 7, version: 3, cliente_name: 'Compañía José', email: 'test@example.test' };
  let stale = true;
  const api = new ApiClient('https://example.test', async (url, init = {}) => {
    const parsed = new URL(String(url)); calls.push({ url: parsed, init });
    if (init.method === 'PUT') {
      assert.equal(new Headers(init.headers).get('If-Match'), '"3"');
      assert.deepEqual(Object.keys(JSON.parse(String(init.body))).sort(), Object.keys(emptyClient()).sort());
      if (stale) return Response.json({ detail: 'El cliente cambió.' }, { status: 412 });
      return Response.json({ ...client, version: 4 });
    }
    if (parsed.pathname.endsWith('/7')) return Response.json(client);
    return Response.json({ items: [client], page: Number(parsed.searchParams.get('page')), page_size: 25, has_more: parsed.searchParams.get('page') === '1' });
  });
  const container = document.createElement('div'); document.body.append(container); const root = createRoot(container);
  const context = { ...access, permissions: [...access.permissions, { module_id: 'clientes', scope_key: 'catalogo', capability: 'editar' }] };
  const click = async (text: string) => { const button = [...container.querySelectorAll('button')].find(b => b.textContent === text); assert.ok(button, text); await act(async () => button.click()); };
  try {
    await act(async () => root.render(<NavigationGuardProvider><CoreContext.Provider value={{ api, access: context, notify: text => notices.push(text) }}><Clientes /></CoreContext.Provider></NavigationGuardProvider>));
    await click('Editar');
    assert.equal(container.querySelector<HTMLInputElement>('[name="cliente_name"]')?.value, client.cliente_name);
    const form = container.querySelector('.client-editor form')!;
    await act(async () => form.dispatchEvent(new Event('submit', { bubbles: true, cancelable: true })));
    assert.ok(container.textContent?.includes('Tus cambios se conservan'));
    assert.equal(container.querySelector<HTMLInputElement>('[name="cliente_name"]')?.value, client.cliente_name);
    assert.ok(container.querySelector<HTMLFieldSetElement>('.client-editor fieldset')?.disabled);
    // An unmodified form can reload the latest version without discarding input.
    await click('Abrir versión actual'); stale = false;
    await act(async () => container.querySelector('.client-editor form')!.dispatchEvent(new Event('submit', { bubbles: true, cancelable: true })));
    assert.ok(notices.includes('Cliente guardado.'));
    assert.equal(container.querySelector('.client-editor'), null);
    await click('Siguiente');
    assert.equal(calls.at(-1)?.url.searchParams.get('page'), '2');
    assert.ok([...container.querySelectorAll('button')].find(b => b.textContent === 'Siguiente')?.disabled);
    await click('Anterior');
    assert.equal(calls.at(-1)?.url.searchParams.get('page'), '1');
  } finally { await act(async () => root.unmount()); container.remove(); }
});
