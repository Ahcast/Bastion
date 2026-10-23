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

// Scout preserves HonduCompras cells as [number, quantity, specifications, unit, ...].
// Only unambiguous decimals are accepted; never infer a quantity from AI prose.
export function sourceQuantity(value: string): string {
  const raw = value.trim();
  if (/^[0-9]{1,12}(?:\.[0-9]{1,4})?$/.test(raw)) return raw;
  // A single comma followed by three digits could be a decimal or a thousands separator.
  if (/^[0-9]{1,12},(?:[0-9]{1,2}|[0-9]{4})$/.test(raw)) return raw.replace(',', '.');
  return '';
}

function originalUrl(value: string | undefined): string | null {
  try {
    const url = new URL(value ?? '');
    return ['https:', 'http:'].includes(url.protocol) && !url.username && !url.password ? url.href : null;
  } catch { return null; }
}

export function quoteFromProcess(row: ProcessRecord, now = new Date()): ProcessQuoteSeed {
  let source: unknown;
  try { source = JSON.parse(row.source.items_fuente || '[]'); }
  catch { throw new Error('No se pueden leer los ítems originales de Scout. Revisa el proceso original.'); }
  if (!Array.isArray(source)) throw new Error('No se reconoce el formato de los ítems originales de Scout. Revisa el proceso original.');
  let items: QuoteItem[];
  if (Array.isArray(source) && source.length && source.every(cells => Array.isArray(cells) && cells.length >= 3 && cells.every(cell => typeof cell === 'string'))) {
    items = (source as string[][]).map(cells => ({
      ...newQuoteItem(),
      // Keep the unit and any additional source cells; never join by position with AI items.
      description: cells.slice(2).filter(cell => cell.trim()).join(' · '),
      quantity: sourceQuantity(cells[1]!),
    }));
  } else if (Array.isArray(source) && source.length) {
    throw new Error('No se reconoce el formato de los ítems originales de Scout. Revisa el proceso original.');
  } else {
    items = processItems(row.source.items ?? '').map(description => ({ ...newQuoteItem(), description, quantity: '' }));
  }
  if (!items.length) throw new Error('El proceso no contiene ítems para cargar en la cotización.');
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
