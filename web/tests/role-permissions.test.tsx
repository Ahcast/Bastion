import { test } from 'node:test';
import assert from 'node:assert/strict';
import { renderToStaticMarkup } from 'react-dom/server';
import { PermissionMatrix } from '../src/components/PermissionMatrix.js';
import { PERMISSION_MODULES, moduleIsConfigurable, permissionModuleCatalog } from '../src/permissionCatalog.js';
import { PERMISSION_COLUMNS, additionalPermissions, permissionSignature, permissionState, removedAdditionalPermissions, setModulePermission, setPermissionColumn, setPermissionRow } from '../src/rolePermissions.js';
import type { Module, Permission } from '../src/types.js';

const registry: Module[] = [{ module_id: 'core', display_name: 'Core', included_in_release: true, environment_allowed: true, enabled: true }, { module_id: 'procesos', display_name: 'Procesos', included_in_release: true, environment_allowed: true, enabled: true }, { module_id: 'cotizaciones', display_name: 'Cotizaciones', included_in_release: true, environment_allowed: false, enabled: false }];
const catalog = permissionModuleCatalog(registry);
const original: Permission[] = [
  { module_id: 'procesos', scope_key: 'consulta', capability: 'ver' },
  { module_id: 'procesos', scope_key: 'notas', capability: 'editar' },
  { module_id: 'procesos', scope_key: '*', capability: 'eliminar' },
  { module_id: 'core', scope_key: 'personas', capability: 'ver' },
  { module_id: 'legacy', scope_key: 'archive', capability: 'download' },
];

test('loading preserves every original tuple and identifies partial and additional grants', () => {
  const copy = structuredClone(original);
  assert.equal(permissionState(copy, 'procesos', 'ver'), 'mixed');
  assert.equal(permissionState(copy, 'procesos', 'editar'), 'mixed');
  assert.equal(permissionState(copy, 'procesos', 'eliminar'), true);
  assert.equal(permissionSignature(copy), permissionSignature([...original].reverse()));
  assert.deepEqual(additionalPermissions(copy, catalog), original.filter(permission => permission.capability !== 'eliminar'));
  assert.deepEqual(copy, original);
});
test('one Crear y editar control uses the existing editar capability and preserves Eliminar', () => {
  assert.deepEqual(PERMISSION_COLUMNS, [{ id: 'ver', label: 'Ver' }, { id: 'editar', label: 'Crear y editar' }, { id: 'eliminar', label: 'Eliminar' }]);
  const edited = setModulePermission(original, 'procesos', 'editar', true);
  assert.deepEqual(edited.slice(0, original.length), original);
  assert.equal(permissionState(edited, 'procesos', 'ver'), true);
  assert.equal(permissionState(edited, 'procesos', 'editar'), true);
  assert.equal(permissionState(edited, 'procesos', 'eliminar'), true);
  assert.ok(!edited.some(permission => permission.capability === 'crear'));
  assert.deepEqual(setModulePermission(edited, 'procesos', 'editar', true), edited);
  assert.deepEqual(removedAdditionalPermissions(original, edited, catalog), []);
});
test('clearing Ver revokes all module actions while preserving Core and unknown permissions', () => {
  const cleared = setModulePermission(original, 'procesos', 'ver', false);
  assert.deepEqual(cleared, original.slice(3));
  assert.deepEqual(removedAdditionalPermissions(original, cleared, catalog), original.slice(0, 2));
  const writable = setPermissionRow([], 'procesos', true);
  for (const column of PERMISSION_COLUMNS) assert.equal(permissionState(writable, 'procesos', column.id), true);
  assert.deepEqual(setPermissionRow(writable, 'procesos', false), []);
  const withoutEditing = setModulePermission(writable, 'procesos', 'editar', false);
  assert.equal(permissionState(withoutEditing, 'procesos', 'eliminar'), true);
  const readOnly = setModulePermission(withoutEditing, 'procesos', 'eliminar', false);
  assert.deepEqual(readOnly, [{ module_id: 'procesos', scope_key: '*', capability: 'ver' }]);
});
test('Eliminar is independent of Crear y editar and grants Ver without creating editar', () => {
  const deleting = setModulePermission([], 'procesos', 'eliminar', true);
  assert.deepEqual(deleting, [{ module_id: 'procesos', scope_key: '*', capability: 'ver' }, { module_id: 'procesos', scope_key: '*', capability: 'eliminar' }]);
  assert.equal(permissionState(deleting, 'procesos', 'editar'), false);
  const both = setModulePermission(deleting, 'procesos', 'editar', true);
  const withoutDeleting = setModulePermission(both, 'procesos', 'eliminar', false);
  assert.equal(permissionState(withoutDeleting, 'procesos', 'editar'), true);
  assert.equal(permissionState(withoutDeleting, 'procesos', 'ver'), true);
  const scopedDelete = { module_id: 'procesos', scope_key: 'archivo', capability: 'eliminar' };
  assert.equal(permissionState([scopedDelete], 'procesos', 'eliminar'), 'mixed');
  assert.deepEqual(removedAdditionalPermissions([scopedDelete], [], catalog), [scopedDelete]);
});
test('bulk controls only target server-approved modules and never grant Inicio or administration', () => {
  const available = catalog.filter(module => moduleIsConfigurable(module, registry)).map(module => module.id);
  assert.deepEqual(available, ['procesos']);
  const result = setPermissionColumn(original, available, 'editar', true);
  assert.deepEqual(result.filter(permission => permission.module_id !== 'procesos'), original.filter(permission => permission.module_id !== 'procesos'));
  assert.equal(permissionState(result, 'cotizaciones', 'ver'), false);
  assert.equal(result.filter(permission => permission.module_id === 'core').length, 1);
  assert.deepEqual(setPermissionColumn(original, [], 'ver', false), original);
  const deleteColumn = setPermissionColumn([], available, 'eliminar', true);
  assert.equal(permissionState(deleteColumn, 'procesos', 'eliminar'), true);
  assert.equal(permissionState(deleteColumn, 'procesos', 'editar'), false);
  assert.equal(permissionState(deleteColumn, 'procesos', 'ver'), true);
});
test('one catalog supplies the requested rows and automatically includes future registered modules', () => {
  assert.deepEqual(PERMISSION_MODULES.map(module => module.label), ['Inicio', 'Clientes', 'Proveedores', 'Órdenes de Compra', 'Cotizaciones', 'Facturas', 'Gastos', 'Reportes', 'Procesos', 'Licitaciones', 'Colaboración', 'Configuración']);
  const module: Module = { module_id: 'inventario', display_name: 'Inventario', included_in_release: true, environment_allowed: true, enabled: true };
  const expanded = permissionModuleCatalog([...registry, module]);
  assert.equal(expanded.at(-1)?.label, 'Inventario');
  assert.ok(moduleIsConfigurable(expanded.at(-1)!, [...registry, module]));
});
test('matrix renders mixed scope state, accessible bulk controls and protected implicit permissions', () => {
  const props = { catalog, registry, permissions: original, disabled: false, cell: () => {}, row: () => {}, column: () => {} };
  const markup = renderToStaticMarkup(<PermissionMatrix {...props} protectedRole={false} />);
  assert.match(markup, /aria-label="Crear y editar · Procesos"[^>]*aria-checked="mixed"/);
  assert.match(markup, /aria-label="Eliminar · Procesos"[^>]*aria-checked="true"/);
  assert.match(markup, /aria-label="Crear y editar · Cotizaciones"[^>]*disabled/);
  assert.match(markup, /aria-label="Ver · Configuración"[^>]*disabled/);
  assert.ok(markup.includes('Marcar todo en Ver'));
  assert.ok(markup.includes('Marcar todo en Eliminar'));
  const integrated = renderToStaticMarkup(<PermissionMatrix {...props} permissions={[]} protectedRole />);
  const inputs = integrated.match(/<input[^>]*>/g) ?? [];
  assert.ok(inputs.length > 30);
  assert.ok(inputs.every(input => input.includes('disabled')));
  assert.match(integrated, /aria-label="Crear y editar · Procesos"[^>]*aria-checked="true"/);
  assert.match(integrated, /aria-label="Eliminar · Procesos"[^>]*aria-checked="true"/);
});
