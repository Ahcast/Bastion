import { test } from 'node:test';
import assert from 'node:assert/strict';
import { ApiClient, ApiError, apiOrigin } from '../src/api.js';
const response = (status: number, data: unknown) => new Response(status === 204 ? null : JSON.stringify(data), { status });
function fixture(sequence: (Response | Error)[]) {
  const calls: { url: string; options: RequestInit }[] = [];
  const transport: typeof fetch = async (url, options) => { calls.push({ url: String(url), options: options ?? {} }); const next = sequence.shift(); assert.ok(next, 'Unexpected request'); if (next instanceof Error) throw next; return next; };
  return { api: new ApiClient('https://api.example.test', transport), calls };
}
test('CSRF rejection refreshes and retries once with the same payload and cookies', async () => {
  const { api, calls } = fixture([response(403, { title: 'CSRF Validation Failed' }), response(200, { csrf_token: 'fresh' }), response(200, { status: 'totp_required' })]);
  const result = await api.request<{ status: string }>('/api/v1/auth/login', { method: 'POST', body: { email: 'synthetic@example.test', password: 'test-only' } });
  assert.equal(result.status, 'totp_required'); assert.equal(calls.length, 3);
  assert.equal(calls[1]?.url, 'https://api.example.test/api/v1/auth/csrf');
  assert.equal(new Headers(calls[2]?.options.headers).get('X-CSRF-Token'), 'fresh');
  assert.equal(calls[0]?.options.body, calls[2]?.options.body);
  for (const call of calls) { assert.equal(call.options.credentials, 'include'); assert.equal(call.options.cache, 'no-store'); }
});
test('repeated CSRF failure requests a new identity without retrying indefinitely', async () => {
  const { api, calls } = fixture([response(403, { title: 'CSRF Validation Failed' }), response(200, { csrf_token: 'fresh' }), response(403, { title: 'CSRF Validation Failed' })]);
  let expired = 0; api.hooks.unauthorized = () => { expired++; };
  await assert.rejects(api.request('/api/v1/account/profile', { method: 'PUT', body: {} }), /Inicia sesión de nuevo/);
  assert.equal(calls.length, 3); assert.equal(expired, 1);
});
test('network, server and Origin errors never retry a mutation', async () => {
  for (const failed of [new Error('offline'), response(500, { detail: 'Unavailable' }), response(403, { title: 'Origin Rejected' })]) {
    const { api, calls } = fixture([failed]); await assert.rejects(api.request('/api/v1/account/profile', { method: 'PUT', body: {} })); assert.equal(calls.length, 1);
  }
});
test('GET rejection never enters a CSRF refresh loop', async () => {
  const { api, calls } = fixture([response(403, { title: 'CSRF Validation Failed' })]);
  await assert.rejects(api.refreshCsrf()); assert.equal(calls.length, 1);
});
test('parallel refresh requests share one token read', async () => {
  const { api, calls } = fixture([response(200, { csrf_token: 'shared' })]);
  await Promise.all([api.refreshCsrf(), api.refreshCsrf()]); assert.equal(calls.length, 1);
});
test('401 ends the UI session and 428 requests TOTP reauthentication', async () => {
  const { api } = fixture([response(401, {}), response(428, {})]); let expired = 0, reauth = 0;
  api.hooks = { unauthorized: () => { expired++; }, reauthenticate: () => { reauth++; } };
  await assert.rejects(api.request('/api/v1/auth/me'), ApiError); await assert.rejects(api.request('/api/v1/account/profile'), ApiError);
  assert.equal(expired, 1); assert.equal(reauth, 1);
});
test('logout failure stays a failure; successful 204 needs no JSON', async () => {
  const { api } = fixture([response(503, { detail: 'Unavailable' }), response(204, null)]);
  await assert.rejects(api.request('/api/v1/auth/logout', { method: 'POST' }));
  assert.equal(await api.request('/api/v1/auth/logout', { method: 'POST' }), null);
});
test('API configuration accepts an HTTPS origin and rejects secrets, paths and insecure hosts', () => {
  assert.equal(apiOrigin('https://api.example.test/'), 'https://api.example.test');
  assert.equal(apiOrigin('http://127.0.0.1:8845'), 'http://127.0.0.1:8845');
  for (const invalid of [undefined, 'http://api.example.test', 'https://user:secret@api.example.test', 'https://api.example.test/path', 'https://api.example.test/?key=value']) assert.throws(() => apiOrigin(invalid));
});

test('browser fetch is invoked without binding the API client as its receiver', async () => {
  const transport: typeof fetch = async function (this: unknown) { assert.equal(this, undefined); return response(200, {}); };
  await new ApiClient('https://api.example.test', transport).request('/api/v1/auth/csrf');
});
