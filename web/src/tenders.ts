export type FormSection = 'legal' | 'economica' | 'tecnica';
export type VariableKind = 'text' | 'integer' | 'decimal' | 'date' | 'boolean';
export interface FormVariable { key: string; label: string; kind: VariableKind; required: boolean }
export interface FormDefinition { title: string; section: FormSection; variables: FormVariable[] }
export interface PublishedForm { id: number; revision: number; hash: string; definition: FormDefinition }
export type FormValues = Record<string, string | boolean>;
export interface FormCopy { id: number; source: PublishedForm; values: FormValues; missing_required: string[] }
export interface Expedient { id: number; version: number; state: string; forms: FormCopy[] }
export interface ExpedientSummary { id: number; version: number; state: string }
export interface TenderPage<T> { items: T[]; page: number; page_size: number; has_more: boolean }
export const sectionLabels: Record<FormSection, string> = { legal: 'Documentación legal', economica: 'Oferta económica', tecnica: 'Especificaciones técnicas' };
export const kindLabels: Record<VariableKind, string> = { text: 'Texto', integer: 'Entero', decimal: 'Decimal', date: 'Fecha', boolean: 'Sí / No' };

// Never parse decimal money with Number or inject schema values as HTML/code.
export function fieldValue(kind: VariableKind, value: string): string | boolean {
  return kind === 'boolean' && value !== '' ? value === 'true' : value;
}
export function missingLabels(copy: FormCopy): string[] {
  return copy.missing_required.map(key => copy.source.definition.variables.find(v => v.key === key)?.label ?? key);
}
