export interface ShortlistItem {
  id: string;
  version: string;
  first_seen_at: string;
  updated_at: string;
  expediente: string;
  entidad: string;
  unidad_compra: string;
  modalidad: string;
  tipo_adquisicion: string;
  etapa: string;
  objeto: string;
  tipo_fuente: string;
  fuente: string;
  fecha_recepcion: string;
  lugar_recepcion: string;
  coincidencias_interes: string;
  posibles_interes: string;
  evidencia_interes: string;
  url: string;
  extraido_en: string;
}
export interface TenderShortlist {
  availability: 'available' | 'missing' | 'unreadable' | 'invalid';
  checked_at: string;
  generated_at: string | null;
  scan_status: 'exitoso' | 'parcial' | null;
  items: ShortlistItem[];
}
export type ShortlistReceipts = Record<string, string>;
export type InterestTag = 'BID' | 'JICA' | 'UCP';
export function interestTags(item: ShortlistItem): string[] {
  return [...new Set([item.coincidencias_interes, item.posibles_interes].join(' | ').split(' | ').filter(Boolean))];
}
export function shortlistMatches(item: ShortlistItem, query: string, tag: string): boolean {
  const text = [item.expediente, item.entidad, item.unidad_compra, item.objeto, item.fuente, item.lugar_recepcion].join(' ');
  const normalize = (value: string) => value.normalize('NFD').replace(/[\u0300-\u036f]/g, '').toLowerCase();
  return (!tag || interestTags(item).includes(tag)) && normalize(text).includes(normalize(query.trim()));
}
export function readShortlistReceipts(value: string | null): ShortlistReceipts {
  try {
    const entries: unknown = JSON.parse(value ?? '{}');
    if (!entries || typeof entries !== 'object' || Array.isArray(entries)) return {};
    return Object.fromEntries(Object.entries(entries).filter(([id, version]) =>
      /^[a-f0-9]{64}$/i.test(id) && typeof version === 'string' && /^[a-f0-9]{64}$/i.test(version)).slice(-10_000));
  } catch { return {}; }
}
