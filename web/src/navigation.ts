import { can, moduleCan } from './types.js';
import type { Access, Page } from './types.js';
import { collaborationCan } from './collaboration.js';
export const pageLabels: Record<Page, string> = { inicio: 'Inicio', procesos: 'Procesos', cotizaciones: 'Cotizaciones', clientes: 'Clientes', proveedores: 'Proveedores', ordenes_compra: 'Órdenes de compra', facturas: 'Facturas', gastos: 'Gastos', reportes: 'Reportes', colaboracion: 'Colaboración', licitaciones: 'Licitaciones', ajustes: 'Ajustes' };
export const reportScope = { ordenes: 'ordenes', facturas: 'facturas', gastos: 'gastos' } as const;
export function reportCan(access: Access, kind: keyof typeof reportScope) {
  if (!moduleCan(access, 'reportes', reportScope[kind], 'ver')) return false;
  if (kind === 'ordenes') return moduleCan(access, 'ordenes_compra', 'documentos', 'ver');
  if (kind === 'facturas') return moduleCan(access, 'facturas', 'documentos', 'ver');
  return moduleCan(access, 'gastos', 'registros', 'ver');
}
export function reportsAvailable(access: Access) {
  return (Object.keys(reportScope) as (keyof typeof reportScope)[]).some(kind => reportCan(access, kind));
}
export function mainNavigation(access: Access): { id: Page; label: string }[] {
  const pages: Page[] = ['inicio'];
  if (moduleCan(access, 'clientes', 'catalogo', 'ver')) pages.push('clientes');
  if (moduleCan(access, 'proveedores', 'catalogo', 'ver')) pages.push('proveedores');
  if (access.modules.some(module => module.module_id === 'procesos')) pages.push('procesos');
  if (moduleCan(access, 'cotizaciones', 'documentos', 'ver')) pages.push('cotizaciones');
  if (moduleCan(access, 'ordenes_compra', 'documentos', 'ver')) pages.push('ordenes_compra');
  if (moduleCan(access, 'facturas', 'documentos', 'ver')) pages.push('facturas');
  if (moduleCan(access, 'gastos', 'registros', 'ver')) pages.push('gastos');
  if (reportsAvailable(access)) pages.push('reportes');
  if (moduleCan(access, 'licitaciones', 'expedientes', 'ver')) pages.push('licitaciones');
  pages.push('ajustes');
  if (collaborationCan(access)) pages.splice(pages.length - 1, 0, 'colaboracion');
  return pages.map(id => ({ id, label: pageLabels[id] }));
}
export type SettingsSection = 'respaldos' | 'cuenta' | 'personas' | 'usuarios' | 'roles' | 'core' | 'cotizaciones' | 'clientes' | 'proveedores' | 'procesos' | 'ordenes_compra' | 'facturas' | 'gastos' | 'reportes' | 'colaboracion' | 'licitaciones';
export function settingsSections(access: Access): { id: SettingsSection; label: string }[] {
  const sections: { id: SettingsSection; label: string }[] = [{ id: 'cuenta', label: 'Mi cuenta' }];
  if (can(access, 'personas', 'ver')) sections.push({ id: 'personas', label: 'Personas' });
  if (access.is_system_administrator) sections.push({ id: 'usuarios', label: 'Usuarios' }, { id: 'roles', label: 'Roles y permisos' }, { id: 'core', label: 'Core' }, { id: 'respaldos', label: 'Respaldos y restauración' });
  if (moduleCan(access, 'cotizaciones', 'documentos', 'ver')) sections.push({ id: 'cotizaciones', label: 'Cotizaciones' });
  if (moduleCan(access, 'clientes', 'catalogo', 'ver')) sections.push({ id: 'clientes', label: 'Clientes' });
  if (moduleCan(access, 'proveedores', 'catalogo', 'ver')) sections.push({ id: 'proveedores', label: 'Proveedores' });
  if (moduleCan(access, 'procesos', 'consulta', 'ver')) sections.push({ id: 'procesos', label: 'Procesos' });
  if (moduleCan(access, 'ordenes_compra', 'documentos', 'ver') || moduleCan(access, 'ordenes_compra', 'destinos', 'ver')) sections.push({ id: 'ordenes_compra', label: 'Órdenes de compra' });
  if (moduleCan(access, 'facturas', 'documentos', 'ver')) sections.push({ id: 'facturas', label: 'Facturas' });
  if (moduleCan(access, 'gastos', 'registros', 'ver')) sections.push({ id: 'gastos', label: 'Gastos' });
  if (reportsAvailable(access)) sections.push({ id: 'reportes', label: 'Reportes' });
  if (collaborationCan(access)) sections.push({ id: 'colaboracion', label: 'Colaboración' });
  if (moduleCan(access, 'licitaciones', 'expedientes', 'ver')) sections.push({ id: 'licitaciones', label: 'Licitaciones' });
  return sections;
}
