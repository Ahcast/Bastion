import { test } from 'node:test';
import assert from 'node:assert/strict';
import { moduleCan } from '../src/types.js';
import type { Access } from '../src/types.js';
import { ApiClient } from '../src/api.js';

test('disabled module stays inaccessible to administrators and stale permissions', () => {
  const access: Access = { is_system_administrator: true, modules: [], permissions: [{ module_id: 'procesos', scope_key: '*', capability: 'ver' }] };
  assert.equal(moduleCan(access, 'procesos', 'consulta', 'ver'), false);
  access.modules.push({ module_id: 'procesos', display_name: 'Procesos' });
  assert.equal(moduleCan(access, 'procesos', 'consulta', 'ver'), true);
  access.is_system_administrator = false;
  access.permissions[0]!.scope_key = 'exportacion';
  assert.equal(moduleCan(access, 'procesos', 'consulta', 'ver'), false);
  assert.equal(moduleCan(access, 'procesos', 'exportacion', 'ver'), true);
  assert.equal(moduleCan(access, 'procesos', 'exportacion', 'editar'), false);
});

test('shared note edits preserve If-Match across the CSRF refresh retry', async () => {
  const calls: RequestInit[] = [];
  const sequence = [new Response(JSON.stringify({title:'CSRF Validation Failed'}),{status:403}),new Response(JSON.stringify({csrf_token:'fresh'})),new Response(JSON.stringify({version:4,marked:true,note:'Compartida'}))];
  const transport: typeof fetch = async (_url, options) => {calls.push(options ?? {});return sequence.shift()!;};
  const api=new ApiClient('https://api.example.test',transport);
  await api.request('/api/v1/procesos/rows/synthetic/review',{method:'PATCH',body:{note:'Compartida'},ifMatch:3});
  assert.equal(new Headers(calls[0]?.headers).get('If-Match'),'"3"');
  assert.equal(new Headers(calls[2]?.headers).get('If-Match'),'"3"');
  assert.equal(new Headers(calls[2]?.headers).get('X-CSRF-Token'),'fresh');
});
