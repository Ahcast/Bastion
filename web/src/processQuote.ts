import type { ProcessRecord } from './processes.js';
import { processItems } from './process-items.js';
import { newQuote, newQuoteItem } from './quotes.js';
import type { QuoteInput, QuoteItem } from './quotes.js';

export interface ProcessQuoteSeed {
  data: QuoteInput;
  expediente: string;
  sourceUrl: string | null;
  missingQuantities: number;
}

// Quantities are read from individual analyzed items, never from a generic source row.
export function sourceQuantity(value: string): string {
  let raw = value.trim();
  if (/^[1-9][0-9]{0,2}(?:,[0-9]{3})+(?:\.[0-9]{1,4})?$/.test(raw)) raw = raw.replaceAll(',', '');
  else if (/^[0-9]{1,12},[0-9]{1,4}$/.test(raw)) raw = raw.replace(',', '.');
  if (!/^[0-9]{1,12}(?:\.[0-9]{1,4})?$/.test(raw)) return '';
  return raw.replace(/^0+(?=[0-9])/, '');
}

function analyzedItem(text: string): QuoteItem {
  const original = { ...newQuoteItem(), description: text, quantity: '' };
  const named = /^(?:cantidad|cant\.?|qty|quantity)\s*[:=]?\s*([0-9]+(?:[.,][0-9]+)*)\s*(?:[×x:;,]\s*|\s+)(.+)$/iu.exec(text);
  const multiplied = /^([0-9]+(?:[.,][0-9]+)*)\s*[×x]\s*(.+)$/iu.exec(text);
  const leading = /^([0-9]+(?:[.,][0-9]+)*)\s+(.+)$/u.exec(text);
  const match = named ?? multiplied ?? leading;
  if (!match) return original;
  const quantity = sourceQuantity(match[1]!);
  const description = match[2]!.trim();
  // A dimension, model number or electrical specification is not a requested quantity.
  if (!quantity || !description || /^[0-9]/u.test(description)
    || (!named && !multiplied && /^(?:v(?:dc|ac)?|a|ah|hz|khz|w|kw|kv|hp|rpm|mm|cm|pulg(?:adas?|\.)?|inch(?:es)?|gb|mb|tb|puertos|voltios|amperios|usd|hnl|l)(?=[\s,;:.]|$)/iu.test(description))) return original;
  return { ...original, description, quantity };
}

function originalUrl(value: string | undefined): string | null {
  try {
    const url = new URL(value ?? '');
    return ['https:', 'http:'].includes(url.protocol) && !url.username && !url.password ? url.href : null;
  } catch { return null; }
}

export function quoteFromProcess(row: ProcessRecord, now = new Date()): ProcessQuoteSeed {
  const items = processItems(row.source.items ?? '').map(text => analyzedItem(text.trim()));
  if (!items.length) throw new Error('El proceso no contiene ítems analizados para cargar en la cotización.');
  if (items.length > 200) throw new Error('El proceso supera los 200 ítems admitidos por cotización. No se ha recortado el listado.');
  if (items.some(item => item.description.length > 3000)) throw new Error('Una especificación supera los 3000 caracteres admitidos por ítem. Revisa el proceso original.');
  if (row.entidad.length > 255 || row.expediente.length > 100) throw new Error('La entidad o el expediente supera la longitud admitida en Cotizaciones. No se han recortado los datos.');
  return {
    data: { ...newQuote(now), client_name: row.entidad, order_number: row.expediente, items },
    expediente: row.expediente,
    sourceUrl: originalUrl(row.source.url),
    missingQuantities: items.filter(item => !item.quantity).length,
  };
}
