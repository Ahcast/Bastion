import { test } from 'node:test';
import assert from 'node:assert/strict';
import { moduleCan } from '../src/types.js';
import type { Access } from '../src/types.js';

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
