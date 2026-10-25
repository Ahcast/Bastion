import { can, moduleCan } from './types.js';
import type { Access, Page } from './types.js';
import { collaborationCan } from './collaboration.js';
export const pageLabels: Record<Page, string> = { inicio: 'Inicio', procesos: 'Procesos', cotizaciones: 'Cotizaciones', clientes: 'Clientes', ordenes_compra: 'Órdenes de compra', facturas: 'Facturas', colaboracion: 'Colaboración', ajustes: 'Ajustes' };
export function mainNavigation(access: Access): { id: Page; label: string }[] {
  const pages: Page[] = ['inicio'];
  if (moduleCan(access, 'clientes', 'catalogo', 'ver')) pages.push('clientes');
  if (access.modules.some(module => module.module_id === 'procesos')) pages.push('procesos');
  if (moduleCan(access, 'cotizaciones', 'documentos', 'ver')) pages.push('cotizaciones');
  if (moduleCan(access, 'ordenes_compra', 'documentos', 'ver')) pages.push('ordenes_compra');
  if (moduleCan(access, 'facturas', 'documentos', 'ver')) pages.push('facturas');
  pages.push('ajustes');
  if (collaborationCan(access)) pages.splice(pages.length - 1, 0, 'colaboracion');
  return pages.map(id => ({ id, label: pageLabels[id] }));
}
export type SettingsSection = 'cuenta' | 'personas' | 'usuarios' | 'roles' | 'core' | 'cotizaciones' | 'clientes' | 'procesos' | 'ordenes_compra' | 'facturas' | 'colaboracion';
export function settingsSections(access: Access): { id: SettingsSection; label: string }[] {
  const sections: { id: SettingsSection; label: string }[] = [{ id: 'cuenta', label: 'Mi cuenta' }];
  if (can(access, 'personas', 'ver')) sections.push({ id: 'personas', label: 'Personas' });
  if (access.is_system_administrator) sections.push({ id: 'usuarios', label: 'Usuarios' }, { id: 'roles', label: 'Roles y permisos' }, { id: 'core', label: 'Core' });
  if (moduleCan(access, 'cotizaciones', 'documentos', 'ver')) sections.push({ id: 'cotizaciones', label: 'Cotizaciones' });
  if (moduleCan(access, 'clientes', 'catalogo', 'ver')) sections.push({ id: 'clientes', label: 'Clientes' });
  if (moduleCan(access, 'procesos', 'consulta', 'ver')) sections.push({ id: 'procesos', label: 'Procesos' });
  if (moduleCan(access, 'ordenes_compra', 'documentos', 'ver') || moduleCan(access, 'ordenes_compra', 'destinos', 'ver')) sections.push({ id: 'ordenes_compra', label: 'Órdenes de compra' });
  if (moduleCan(access, 'facturas', 'documentos', 'ver')) sections.push({ id: 'facturas', label: 'Facturas' });
  if (collaborationCan(access)) sections.push({ id: 'colaboracion', label: 'Colaboración' });
  return sections;
}
