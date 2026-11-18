import { mainNavigation, settingsSections, reportCan } from './navigation.js';
import { can, moduleCan } from './types.js';
import type { Access, Page } from './types.js';
import type { SettingsSection } from './navigation.js';

export const modulePaths: Record<Page, string> = { inicio: '/', procesos: '/procesos', cotizaciones: '/cotizaciones', clientes: '/clientes', proveedores: '/proveedores', ordenes_compra: '/ordenes-compra', facturas: '/facturas', gastos: '/gastos', contabilidad: '/contabilidad', reportes: '/reportes', colaboracion: '/colaboracion', licitaciones: '/licitaciones', ajustes: '/ajustes' };
type Screen = { views?: readonly string[]; record?: boolean | 'opaque'; fields?: readonly string[] };
// Navigation identifiers only. Never put form data, credentials or API URLs here.
export const routeCatalog: Record<string, Screen> = {
  inicio: { fields: ['panel'] },
  procesos: { views: ['compras', 'licitaciones'], record: 'opaque', fields: ['pagina'] },
  cotizaciones: { record: true, fields: ['modo', 'pagina', 'correo', 'envios'] },
  clientes: { record: true, fields: ['modo', 'pagina'] },
  proveedores: { record: true, fields: ['pagina'] },
  ordenes_compra: { record: true, fields: ['modo', 'documentos', 'pagina'] },
  facturas: { record: true, fields: ['modo', 'pagina'] },
  gastos: { record: true, fields: ['pagina'] },
  contabilidad: { views: ['diario', 'integrar', 'cuentas', 'periodos', 'balance'], record: true, fields: ['cuenta', 'periodo', 'crear'] },
  reportes: { views: ['ordenes', 'facturas', 'gastos'] },
  colaboracion: { views: ['files', 'forms'], record: true, fields: ['ambito', 'persona', 'papelera', 'pagina', 'archivo'] },
  licitaciones: { record: true, views: ['resumen', 'variables', 'legal', 'tecnica', 'economica', 'pdf', 'adjudicaciones'], fields: ['pagina', 'crear', 'adjudicacion', 'factura', 'lote'] },
  'ajustes/cuenta': { views: ['profile', 'security', 'smtp'] },
  'ajustes/core': { views: ['modules', 'company', 'smtp', 'audit', 'mail'] },
  'ajustes/personas': { record: true },
  'ajustes/usuarios': { fields: ['invitar'] },
  'ajustes/roles': { record: true, fields: ['crear'] },
  'ajustes/mcp': {}, 'ajustes/respaldos': {}, 'ajustes/procesos': {},
  'ajustes/cotizaciones': {}, 'ajustes/clientes': {}, 'ajustes/proveedores': {},
  'ajustes/ordenes_compra': { record: true }, 'ajustes/facturas': {}, 'ajustes/gastos': {},
  'ajustes/contabilidad': { views: ['cuentas', 'periodos'], fields: ['cuenta', 'periodo', 'crear'] },
  'ajustes/reportes': {}, 'ajustes/colaboracion': {}, 'ajustes/licitaciones': { record: true },
};
export type AppRoute = { page: Page; section?: SettingsSection; values: Record<string, string> };
export const routeArea = (r: AppRoute) => r.page === 'ajustes' ? `ajustes/${r.section ?? 'cuenta'}` : r.page;
export function validRecord(raw: string, opaque = false) {
  return raw === 'nuevo' || (opaque ? /^[\p{L}\p{N}_.:@-]{1,180}$/u.test(raw) && raw !== '.' && raw !== '..' : /^[1-9]\d{0,14}$/.test(raw) && Number.isSafeInteger(Number(raw)));
}
export function parseRoute(pathname: string, search = ''): AppRoute | null {
  let parts: string[];
  try { parts = pathname.split('/').filter(Boolean).map(decodeURIComponent); } catch { return null; }
  if (parts.some(p => /[\\/\u0000-\u001f]/.test(p))) return null;
  const first = parts.shift() ?? '';
  const page = (Object.keys(modulePaths) as Page[]).find(p => modulePaths[p] === `/${first}`);
  if (!page) return null;
  const section = page === 'ajustes' ? (parts.shift() ?? 'cuenta') as SettingsSection : undefined;
  const route: AppRoute = { page, section, values: {} };
  const schema = routeCatalog[routeArea(route)];
  if (!schema) return null;
  for (const part of parts) {
    if (schema.views?.includes(part) && !route.values.vista) route.values.vista = part;
    else if (schema.record && !route.values.registro && validRecord(part, schema.record === 'opaque')) route.values.registro = part;
    else return null;
  }
  for (const [key, value] of new URLSearchParams(search)) {
    if (!schema.fields?.includes(key) || key in route.values || value.length > 180 || /[\u0000-\u001f]/.test(value)) return null;
    if (['pagina', 'persona', 'documentos', 'archivo', 'adjudicacion', 'factura', 'lote', 'correo', 'envios'].includes(key) && (value === 'nuevo' || !validRecord(value))) return null;
    if (key === 'modo' && !['view', 'edit'].includes(value)) return null;
    if (key === 'ambito' && !['empresa', 'persona'].includes(value)) return null;
    if (['crear', 'invitar', 'papelera'].includes(key) && value !== '1') return null;
    if (key === 'panel' && !['procesos', 'cotizaciones', 'ordenes', 'facturas', 'gastos'].includes(value)) return null;
    if (key === 'cuenta' && !/^\d{1,12}$/.test(value)) return null;
    if (key === 'periodo' && !/^\d{4}-(0[1-9]|1[0-2])$/.test(value)) return null;
    route.values[key] = value;
  }
  return route;
}
export function routeHref(route: AppRoute) {
  let path = modulePaths[route.page];
  if (route.page === 'ajustes') path += '/' + (route.section ?? 'cuenta');
  const { registro, vista, ...query } = route.values;
  if (registro) path += '/' + encodeURIComponent(registro);
  if (vista) path += (path.endsWith('/') ? '' : '/') + encodeURIComponent(vista);
  const params = new URLSearchParams(Object.entries(query).filter(([, v]) => v !== ''));
  return path + (params.size ? '?' + params : '');
}
export function routeAllowed(route: AppRoute, access: Access, mcpEnabled = false) {
  if (!mainNavigation(access).some(p => p.id === route.page)) return false;
  if (route.page === 'ajustes' && !settingsSections(access, mcpEnabled).some(s => s.id === route.section)) return false;
  const { registro, vista, modo } = route.values;
  const editing = registro === 'nuevo' || modo === 'edit';
  const scopes: Record<string, string> = { cotizaciones: 'documentos', facturas: 'documentos', ordenes_compra: 'documentos', clientes: 'catalogo', proveedores: 'catalogo', gastos: 'registros', contabilidad: 'libros' };
  if (editing && scopes[route.page] && !moduleCan(access, route.page, scopes[route.page]!, 'editar')) return false;
  if (route.page === 'ajustes' && route.section === 'personas' && registro && !can(access, 'personas', 'editar')) return false;
  if (route.page === 'ajustes' && route.section === 'licitaciones' && registro && !access.is_system_administrator) return false;
  if (route.page === 'ajustes' && route.section === 'ordenes_compra' && registro && !moduleCan(access, 'ordenes_compra', 'destinos', 'editar')) return false;
  if ((route.page === 'contabilidad' || route.page === 'ajustes' && route.section === 'contabilidad') && (registro === 'nuevo' || route.values.crear || route.values.cuenta || route.values.periodo) && !moduleCan(access, 'contabilidad', 'libros', 'editar')) return false;
  if (route.page === 'procesos' && !moduleCan(access, 'procesos', 'consulta', 'ver')) return false;
  if (route.page === 'reportes' && vista && !reportCan(access, vista as 'ordenes' | 'facturas' | 'gastos')) return false;
  if (route.page === 'colaboracion') {
    if (vista === 'forms' && !moduleCan(access, 'colaboracion', 'empresa', 'ver')) return false;
    if (vista === 'forms' && registro && !access.is_system_administrator) return false;
    if (route.values.ambito === 'empresa' && !moduleCan(access, 'colaboracion', 'empresa', 'ver')) return false;
    if (route.values.ambito === 'persona' && route.values.persona && !moduleCan(access, 'colaboracion', `persona:${route.values.persona}`, 'ver')) return false;
  }
  if (route.page === 'cotizaciones' && route.values.correo && !moduleCan(access, 'cotizaciones', 'documentos', 'editar')) return false;
  if (route.page === 'licitaciones') {
    if (route.values.crear && !moduleCan(access, 'licitaciones', 'expedientes', 'editar')) return false;
    if ((route.values.adjudicacion || route.values.factura) && (!moduleCan(access, 'licitaciones', 'expedientes', 'editar') || !moduleCan(access, 'facturas', 'documentos', 'ver') || !moduleCan(access, 'facturas', 'documentos', 'editar') || !moduleCan(access, 'clientes', 'catalogo', 'ver'))) return false;
  }
  return true;
}
