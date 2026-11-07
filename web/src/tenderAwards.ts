import { newInvoice, newInvoiceItem } from './invoices.js';
import type { InvoiceInput } from './invoices.js';

export interface AwardSelection { lot_number: number; item_number: number; quantity: string }
export interface AwardItem extends AwardSelection { description: string; unit: string }
export interface Award {
  id: number;
  expedient_id: number;
  version: number;
  snapshot: { expedient_name: string; expedient_version: number; items: AwardItem[] };
}
export interface AwardSummary { id: number; expedient_id: number; item_count: number }
export interface AvailableAwardLot {
  lot_number: number;
  version: number;
  items: { item_number: number; description: string; offered_quantity: string; remaining_quantity: string; unit: string }[];
}
export interface TenderInvoiceResult { target_module: 'facturas'; target_id: number; award_id: number; reused: boolean }

export function invoiceFromAward(award: Award): InvoiceInput {
  const input = newInvoice();
  input.payment_term_type = 'credito';
  input.credit_days = 30;
  input.items = award.snapshot.items.map(item => {
    const [whole, fraction = ''] = item.quantity.split('.');
    if (!/^[0-9]{1,12}(?:[.][0-9]{1,12})?$/.test(item.quantity) || !whole || fraction.replace(/0+$/, '').length > 2) {
      throw new Error('Esta adjudicación contiene cantidades que requieren más de dos decimales. Facturas necesita ampliar su precisión antes de facturarlas.');
    }
    const decimals = fraction.replace(/0+$/, '');
    const quantity = decimals ? `${whole}.${decimals}` : whole;
    return { ...newInvoiceItem(), description: item.description, quantity };
  });
  return input;
}
