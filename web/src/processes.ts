export interface ProcessRecord {
  marked: boolean; note: string; version: number;
  proceso_id: string; entidad: string; expediente: string; fecha_recepcion: string;
  puntaje: number; rubro: string; resumen_corto: string; source: Record<string, string>;
}
export interface ProcessPage { items: ProcessRecord[]; page: number; page_size: number; has_more: boolean }
export interface ProcessStatus {
  total: number; last_import_at: string | null; as_of: string; timezone: string;
  weekly: { week_start: string; count: number }[];
  due: { date: string; count: number }[];
}
export type ProcessSort = 'fecha_desc' | 'fecha_asc' | 'puntaje_desc' | 'puntaje_asc';
export interface ProcessFilters { entidad: string; q: string; sort: ProcessSort; marked: boolean; hide_municipalities: boolean }
export const initialProcessFilters: ProcessFilters = { entidad: '', q: '', sort: 'fecha_desc', marked: false, hide_municipalities: false };
export function processQuery(filters: ProcessFilters): string {
  const query = new URLSearchParams({ sort: filters.sort });
  if (filters.entidad) query.set('entidad', filters.entidad);
  if (filters.q.trim()) query.set('q', filters.q.trim());
  if (filters.marked) query.set('marked', 'true');
  if (filters.hide_municipalities) query.set('hide_municipalities', 'true');
  return query.toString();
}
export const processDate = (value: string) => new Intl.DateTimeFormat('es-HN', { timeZone: 'America/Tegucigalpa', dateStyle: 'medium', timeStyle: 'short' }).format(new Date(value));
// Anchor calendar-only dates at Honduras noon to preserve their day.
export const processDay = (value: string) => new Intl.DateTimeFormat('es-HN', { timeZone: 'America/Tegucigalpa', day: 'numeric', month: 'short' }).format(new Date(value + 'T12:00:00-06:00'));
