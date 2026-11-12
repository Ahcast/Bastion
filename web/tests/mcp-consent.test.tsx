import { after, test } from 'node:test';
import assert from 'node:assert/strict';
import { Window } from 'happy-dom';
import { ApiClient } from '../src/api.js';
import McpConsent from '../src/pages/McpConsent.js';

const window = new Window();
for (const name of ['document', 'HTMLElement', 'Event', 'MouseEvent', 'navigator'] as const) Object.defineProperty(globalThis, name, { configurable: true, value: window[name] });
Object.assign(globalThis, { window, IS_REACT_ACT_ENVIRONMENT: true });
const { act } = await import('react');
const { createRoot } = await import('react-dom/client');
const inspection = { status: 'ready', client_id: 'https://client.example/metadata.json', client_name: 'Client', scopes: ['facturas:read'], redirect_uri: 'https://client.example/callback' };
after(() => window.happyDOM.abort());

test('new client enrollment requires a separate explicit approval request before consent', async () => {
  const calls: { path: string; body: unknown }[] = [];
  let approved = false;
  const api = new ApiClient('https://api.example.test', async (url, init = {}) => {
    const path = new URL(String(url)).pathname;
    calls.push({ path, body: JSON.parse(String(init.body)) });
    if (path.endsWith('/inspect-authorization')) return Response.json({ ...inspection, status: approved ? 'ready' : 'approval_required' });
    if (path.endsWith('/begin')) return Response.json({ handle: 'A'.repeat(43), client_name: 'Client', scopes: ['facturas:read'], expires_at: '2099-01-01T00:00:00Z' });
    return Response.json({ status: 'pending' });
  });
  const container = document.createElement('div'); document.body.append(container); const root = createRoot(container);
  try {
    await act(async () => root.render(<McpConsent api={api} authorizationQuery="request" />));
    const button = (text: string) => [...container.querySelectorAll('button')].find(item => item.textContent === text)!;
    await act(async () => button('Revisar solicitud').click());
    assert.equal(calls.length, 1);
    assert.ok(container.textContent?.includes('aprobación administrativa'));
    assert.equal(button('Autorizar'), undefined);
    await act(async () => button('Solicitar aprobación').click());
    assert.deepEqual(calls[1], { path: '/oauth/clients/request', body: { client_id: inspection.client_id, scopes: inspection.scopes } });
    assert.ok(container.textContent?.includes('Solicitud enviada'));
    approved = true;
    await act(async () => button('Volver a verificar').click());
    assert.equal(calls.filter(call => call.path.endsWith('/begin')).length, 1);
    assert.ok(button('Autorizar'));
    assert.equal(calls.some(call => call.path.endsWith('/decision')), false);
  } finally { await act(async () => root.unmount()); container.remove(); }
});

test('consent requires an explicit review and decision and renders client text safely', async () => {
  for (const decision of ['approve', 'deny'] as const) {
    const calls: { path: string; body: Record<string, unknown> }[] = [];
    const api = new ApiClient('https://api.example.test', async (url, init = {}) => {
      const path = new URL(String(url)).pathname;
      calls.push({ path, body: JSON.parse(String(init.body)) as Record<string, unknown> });
      return Response.json(path.endsWith('/inspect-authorization') ? inspection : path.endsWith('/begin')
        ? { handle: 'A'.repeat(43), client_name: '<img src=x onerror=alert(1)>', scopes: ['cotizaciones:read', 'facturas:read', 'ordenes_compra:read', 'licitaciones:read', 'facturas:write'], expires_at: '2099-01-01T00:00:00Z' }
        : { redirect_to: 'https://client.example.test/callback?state=ok' });
    });
    let redirected = '';
    const container = document.createElement('div'); document.body.append(container); const root = createRoot(container);
    try {
      await act(async () => root.render(<McpConsent api={api} authorizationQuery="response_type=code" navigate={url => { redirected = url; }} />));
      assert.equal(calls.length, 0);
      await act(async () => container.querySelector<HTMLButtonElement>('button')!.click());
      assert.equal(calls.length, 2);
      assert.equal(calls[0]?.path, '/oauth/clients/inspect-authorization');
      assert.equal(calls[1]?.path, '/oauth/consent/begin');
      assert.ok(container.textContent?.includes('<img src=x onerror=alert(1)>'));
      assert.equal(container.querySelector('img'), null);
      assert.ok(container.textContent?.includes('Consultar órdenes de compra'));
      assert.ok(container.textContent?.includes('Consultar licitaciones'));
      assert.ok(container.textContent?.includes('Crear y editar facturas en borrador'));
      const buttons = [...container.querySelectorAll('button')];
      const button = buttons.find(item => item.textContent === (decision === 'approve' ? 'Autorizar' : 'Rechazar'))!;
      await act(async () => button.click());
      assert.equal(calls[2]?.body.decision, decision);
      assert.equal(redirected, 'https://client.example.test/callback?state=ok');
    } finally { await act(async () => root.unmount()); container.remove(); }
  }
});

test('consent never navigates to an unsafe response URL', async () => {
  const api = new ApiClient('https://api.example.test', async url => Response.json(new URL(String(url)).pathname.endsWith('/inspect-authorization') ? inspection : new URL(String(url)).pathname.endsWith('/begin')
    ? { handle: 'A'.repeat(43), client_name: 'Client', scopes: ['facturas:read'], expires_at: '2099-01-01T00:00:00Z' }
    : { redirect_to: 'javascript:alert(1)' }));
  let redirected = false;
  const container = document.createElement('div'); document.body.append(container); const root = createRoot(container);
  try {
    await act(async () => root.render(<McpConsent api={api} authorizationQuery="request" navigate={() => { redirected = true; }} />));
    await act(async () => container.querySelector<HTMLButtonElement>('button')!.click());
    await act(async () => [...container.querySelectorAll('button')].find(item => item.textContent === 'Autorizar')!.click());
    assert.equal(redirected, false);
    assert.ok(container.querySelector('[role="alert"]'));
  } finally { await act(async () => root.unmount()); container.remove(); }
});

test('consent permits exact localhost but rejects similar remote hostnames', async () => {
  for (const [target, allowed] of [['http://localhost:8787/callback?code=test', true], ['http://localhost.example/callback', false], ['http://localhost./callback', false]] as const) {
    const api = new ApiClient('https://api.example.test', async url => Response.json(new URL(String(url)).pathname.endsWith('/inspect-authorization') ? inspection : new URL(String(url)).pathname.endsWith('/begin')
      ? { handle: 'A'.repeat(43), client_name: 'Client', scopes: ['facturas:read'], expires_at: '2099-01-01T00:00:00Z' }
      : { redirect_to: target }));
    let redirected = '';
    const container = document.createElement('div'); document.body.append(container); const root = createRoot(container);
    try {
      await act(async () => root.render(<McpConsent api={api} authorizationQuery="request" navigate={url => { redirected = url; }} />));
      await act(async () => container.querySelector<HTMLButtonElement>('button')!.click());
      await act(async () => [...container.querySelectorAll('button')].find(item => item.textContent === 'Autorizar')!.click());
      assert.equal(redirected, allowed ? target : '');
      if (!allowed) assert.ok(container.querySelector('[role="alert"]'));
    } finally { await act(async () => root.unmount()); container.remove(); }
  }
});
