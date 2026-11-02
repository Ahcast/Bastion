import { after, test } from 'node:test';
import assert from 'node:assert/strict';
import { Window } from 'happy-dom';
import { ApiClient } from '../src/api.js';
import McpClients from '../src/pages/McpClients.js';

const window = new Window();
for (const name of ['document', 'HTMLElement', 'HTMLInputElement', 'Event', 'MouseEvent', 'navigator'] as const) Object.defineProperty(globalThis, name, { configurable: true, value: window[name] });
Object.assign(globalThis, { window, IS_REACT_ACT_ENVIRONMENT: true });
const { act } = await import('react');
const { createRoot } = await import('react-dom/client');
after(() => window.happyDOM.abort());

test('only an administrator sees the review queue and selects scopes explicitly', async () => {
  for (const admin of [false, true]) {
    const calls: { path: string; body: Record<string, unknown> }[] = [];
    const api = new ApiClient('https://api.example.test', async (url, init = {}) => {
      const path = new URL(String(url)).pathname;
      calls.push({ path, body: JSON.parse(String(init.body)) as Record<string, unknown> });
      if (path.endsWith('/pending')) return Response.json({ items: [{ id: 3, client_name: '<img src=x>', client_id: 'https://client.example/metadata', scopes: ['cotizaciones:read'], status: 'pending' }], next_before: null });
      if (path.endsWith('/mine')) return Response.json({ items: [{ id: 2, client_name: 'Otro cliente', client_id: 'https://other.example/metadata', scopes: ['cotizaciones:read', 'facturas:read'], approved_scopes: ['cotizaciones:read'], status: 'approved' }], next_before: null });
      if (path.endsWith('/admin/list')) return Response.json({ items: [], next_before: null });
      return Response.json({ status: 'reviewed' });
    });
    const container = document.createElement('div'); document.body.append(container); const root = createRoot(container);
    try {
      await act(async () => root.render(<McpClients api={api} isAdministrator={admin} />));
      assert.equal(calls.some(call => call.path.endsWith('/pending')), admin);
      assert.ok(container.textContent?.includes('Aprobados: Consultar cotizaciones'));
      assert.ok(container.textContent?.includes('Consultar licitaciones'));
      assert.ok(container.textContent?.includes('Crear y editar órdenes pendientes sin compras ni entregas'));
      assert.ok(container.textContent?.includes('Crear y editar facturas en borrador'));
      if (admin) {
        assert.ok(container.textContent?.includes('<img src=x>'));
        assert.equal(container.querySelector('img'), null);
        const approve = [...container.querySelectorAll('button')].find(button => button.textContent === 'Aprobar')!;
        await act(async () => approve.click());
        assert.equal(calls.some(call => call.path.endsWith('/review')), false);
        const checkbox = container.querySelector<HTMLInputElement>('article input[type="checkbox"]')!;
        await act(async () => checkbox.click());
        await act(async () => approve.click());
        const review = calls.find(call => call.path.endsWith('/review'))!;
        assert.equal(review.body.decision, 'approve');
        assert.deepEqual(review.body.scopes, ['cotizaciones:read']);
      } else assert.equal(container.querySelector('article'), null);
    } finally { await act(async () => root.unmount()); container.remove(); }
  }
});

test('administrators edit scope ceilings and explicitly confirm disabling a client', async () => {
  const calls: { path: string; body: Record<string, unknown> }[] = [];
  let scopes = ['cotizaciones:read'];
  let enabled = true;
  const api = new ApiClient('https://api.example.test', async (url, init = {}) => {
    const path = new URL(String(url)).pathname;
    const body = JSON.parse(String(init.body)) as Record<string, unknown>;
    calls.push({ path, body });
    if (path.endsWith('/admin/list')) return Response.json({ items: enabled ? [{ id: 9, client_id: 'https://fixture.example/client.json', client_name: 'Cliente de prueba', scopes }] : [], next_before: null });
    if (path.endsWith('/admin/scopes')) { scopes = body.scopes as string[]; return Response.json({ status: 'updated' }); }
    if (path.endsWith('/admin/disable')) { enabled = false; return Response.json({ status: 'disabled' }); }
    return Response.json({ items: [], next_before: null });
  });
  const container = document.createElement('div'); document.body.append(container); const root = createRoot(container);
  try {
    await act(async () => root.render(<McpClients api={api} isAdministrator />));
    const button = (name: string) => [...container.querySelectorAll('button')].find(item => item.textContent === name)!;
    await act(async () => button('Editar permisos').click());
    const editor = container.querySelector('section[aria-label^="Editar permisos"]')!;
    const checkbox = [...editor.querySelectorAll<HTMLInputElement>('input')].find(input => input.parentElement?.textContent?.includes('Consultar facturas'))!;
    await act(async () => checkbox.click());
    await act(async () => button('Guardar permisos').click());
    const update = calls.find(call => call.path.endsWith('/admin/scopes'))!;
    assert.deepEqual(update.body.expected_scopes, ['cotizaciones:read']);
    assert.deepEqual(update.body.scopes, ['cotizaciones:read', 'facturas:read']);
    await act(async () => button('Eliminar cliente').click());
    assert.equal(calls.some(call => call.path.endsWith('/admin/disable')), false);
    assert.ok(container.textContent?.includes('todos los usuarios'));
    await act(async () => button('Confirmar eliminación').click());
    assert.deepEqual(calls.find(call => call.path.endsWith('/admin/disable'))?.body, { id: 9, confirmed_client_id: 'https://fixture.example/client.json' });
    assert.ok(container.textContent?.includes('Sin clientes autorizados'));
  } finally { await act(async () => root.unmount()); container.remove(); }
});
