import type { Permission } from './types.js';
import type { PermissionModule } from './permissionCatalog.js';

export type PermissionColumn = 'ver' | 'crear' | 'editar';
export const PERMISSION_COLUMNS: readonly { id: PermissionColumn; label: string }[] = [
  { id: 'ver', label: 'Ver' }, { id: 'crear', label: 'Crear' }, { id: 'editar', label: 'Editar' },
];
export function permissionKey(permission: Permission): string {
  return JSON.stringify([permission.module_id, permission.scope_key, permission.capability]);
}
export function permissionSignature(permissions: readonly Permission[]): string {
  return JSON.stringify(permissions.map(permissionKey).sort());
}
export function capabilityFor(column: PermissionColumn): 'ver' | 'editar' { return column === 'ver' ? 'ver' : 'editar'; }
export function permissionState(permissions: readonly Permission[], module: string, column: PermissionColumn): boolean | 'mixed' {
  const matching = permissions.filter(permission => permission.module_id === module && permission.capability === capabilityFor(column));
  return matching.some(permission => permission.scope_key === '*') ? true : matching.length ? 'mixed' : false;
}
export function setModulePermission(permissions: readonly Permission[], module: string, column: PermissionColumn, enabled: boolean): Permission[] {
  const capability = capabilityFor(column);
  if (!enabled) return permissions.filter(permission => permission.module_id !== module || (permission.capability !== capability && !(capability === 'ver' && permission.capability === 'editar')));
  const result = [...permissions];
  for (const grant of capability === 'editar' ? ['ver', 'editar'] : ['ver']) {
    if (!result.some(permission => permission.module_id === module && permission.scope_key === '*' && permission.capability === grant)) {
      result.push({ module_id: module, scope_key: '*', capability: grant });
    }
  }
  return result;
}
export function setPermissionRow(permissions: readonly Permission[], module: string, enabled: boolean): Permission[] {
  return setModulePermission(permissions, module, enabled ? 'editar' : 'ver', enabled);
}
export function setPermissionColumn(permissions: readonly Permission[], modules: readonly string[], column: PermissionColumn, enabled: boolean): Permission[] {
  return modules.reduce((current, module) => setModulePermission(current, module, column, enabled), [...permissions]);
}
export function additionalPermissions(permissions: readonly Permission[], catalog: readonly PermissionModule[]): Permission[] {
  const modules = new Set(catalog.filter(module => module.kind === 'module').map(module => module.id));
  return permissions.filter(permission => !modules.has(permission.module_id) || permission.scope_key !== '*' || !['ver', 'editar'].includes(permission.capability));
}
export function removedAdditionalPermissions(before: readonly Permission[], after: readonly Permission[], catalog: readonly PermissionModule[]): Permission[] {
  const kept = new Set(after.map(permissionKey));
  return additionalPermissions(before, catalog).filter(permission => !kept.has(permissionKey(permission)));
}
