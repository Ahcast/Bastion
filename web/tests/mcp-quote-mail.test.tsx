import { after, test } from 'node:test';
import assert from 'node:assert/strict';
import { Window } from 'happy-dom';
import { ApiClient } from '../src/api.js';
import McpQuoteMail from '../src/pages/McpQuoteMail.js';
import { takeMailConfirmation } from '../src/mcpNavigation.js';

const window = new Window();
for (const name of ['document', 'HTMLElement', 'Event', 'MouseEvent', 'navigator'] as const) Object.defineProperty(globalThis, name, { configurable: true, value: window[name] });
Object.assign(globalThis, { window, IS_REACT_ACT_ENVIRONMENT: true });
const { act } = await import('react');
const { createRoot } = await import('react-dom/client');
after(() => window.happyDOM.abort());

test('mail confirmation removes the fragment and rejects query parameters', () => {
  let cleaned = '';
  const route = { pathname: '/mcp/quote-email', search: '', hash: '#'+ 'A'.repeat(43) };
  assert.equal(takeMailConfirmation(true, route, value => { cleaned = value; }), 'A'.repeat(43));
  assert.equal(cleaned, '/mcp/quote-email');
  assert.equal(takeMailConfirmation(true, { ...route, search: '?approve=true' }, () => {}), '');
});

test('only the explicit human decision requests queueing and preserves the reviewed handle', async () => {
  for (const choice of ['approve', 'deny']) {
    const calls: { path: string; body: Record<string, unknown> }[] = [];
    const api = new ApiClient('https://api.example.test', async (url, init = {}) => {
      const path = new URL(String(url)).pathname;
      calls.push({ path, body: JSON.parse(String(init.body)) as Record<string, unknown> });
      return Response.json(path.endsWith('/review') ? {
        status: 'awaiting_user_confirmation', quote_id: 7, quote_number: 'COT-7', version: 3,
        client_name: '<script>client</script>', attachments: ['condiciones.pdf'], expires_at: '2099-11-11T21:45:00Z',
        input: { to: 'customer@example.test', cc: '', bcc: 'copy@example.test', subject: 'Cotización', message: '<img src=x>', scheduled_at: '2099-11-11T15:30:00-06:00' },
      } : { status: choice === 'approve' ? 'queued' : 'denied' });
    });
    const container = document.createElement('div'); document.body.append(container); const root = createRoot(container);
    try {
      await act(async () => root.render(<McpQuoteMail api={api} handle={'A'.repeat(43)} />));
      assert.equal(calls.length, 0);
      await act(async () => container.querySelector<HTMLButtonElement>('button')!.click());
      assert.equal(calls.length, 1);
      assert.ok(container.textContent?.includes('copy@example.test'));
      assert.ok(container.textContent?.includes('condiciones.pdf'));
      assert.equal(container.querySelector('script'), null);
      assert.equal(container.querySelector('img'), null);
      const button = [...container.querySelectorAll('button')].find(item => item.textContent === (choice === 'approve' ? 'Confirmar y programar envío' : 'Rechazar'))!;
      await act(async () => button.click());
      assert.deepEqual(calls[1], { path: '/oauth/quote-email/decision', body: { handle: 'A'.repeat(43), decision: choice } });
      assert.equal(container.querySelector('button'), null);
    } finally { await act(async () => root.unmount()); container.remove(); }
  }
});
