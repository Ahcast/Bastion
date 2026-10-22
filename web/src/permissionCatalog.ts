import type { Module } from './types.js';

export interface PermissionModule {
  id: string;
  label: string;
  kind: 'module' | 'session' | 'administration';
}

// Pending module identifiers follow the documented architecture. They cannot
// grant permissions until the server includes and allows that exact module.
export const PERMISSION_MODULES: readonly PermissionModule[] = [
  { id: 'inicio', label: 'Inicio', kind: 'session' },
  { id: 'clientes', label: 'Clientes', kind: 'module' },
  { id: 'proveedores', label: 'Proveedores', kind: 'module' },
  { id: 'ordenes_compra', label: 'Órdenes de Compra', kind: 'module' },
  { id: 'cotizaciones', label: 'Cotizaciones', kind: 'module' },
  { id: 'facturas', label: 'Facturas', kind: 'module' },
  { id: 'gastos', label: 'Gastos', kind: 'module' },
  { id: 'reportes', label: 'Reportes', kind: 'module' },
  { id: 'procesos', label: 'Procesos', kind: 'module' },
  { id: 'colaboracion', label: 'Colaboración', kind: 'module' },
  { id: 'core', label: 'Configuración', kind: 'administration' },
];

export function permissionModuleCatalog(registry: readonly Module[]): PermissionModule[] {
  const known = new Set(PERMISSION_MODULES.map(module => module.id));
  return [...PERMISSION_MODULES, ...registry.filter(module => !known.has(module.module_id)).map(module => ({
    id: module.module_id, label: module.display_name, kind: 'module' as const,
  }))];
}

export function moduleIsConfigurable(module: PermissionModule, registry: readonly Module[]): boolean {
  return module.kind === 'module' && registry.some(entry => entry.module_id === module.id && entry.included_in_release && entry.environment_allowed);
}
