import type { QuoteCalculation, QuoteItem } from '../quotes.js';

export function QuoteItems({ items, calculation, change, remove, add }: {
  items: QuoteItem[]; calculation: QuoteCalculation | null;
  change: <K extends keyof QuoteItem>(index: number, key: K, value: QuoteItem[K]) => void;
  remove: (index: number) => void; add: () => void;
}) {
  return <section className="quote-section" aria-labelledby="quote-items-title">
    <div className="quote-section-heading"><h2 id="quote-items-title">Ítems</h2><span>{items.length} / 200</span></div>
    <div className="quote-items-wrap"><table className="quote-items-table" aria-label="Ítems de cotización">
      <colgroup><col className="quote-col-description" /><col /><col className="quote-col-quantity" /><col className="quote-col-discount" /><col /><col className="quote-col-total" /><col className="quote-col-remove" /></colgroup>
      <thead><tr><th scope="col">Descripción</th><th scope="col">Precio unitario</th><th scope="col">Cantidad</th><th scope="col">Descuento</th><th scope="col">Impuesto</th><th scope="col">Total</th><th scope="col"><span className="quote-sr-only">Quitar</span></th></tr></thead>
      <tbody>{items.map((row, index) => <tr key={index}>
        <td className="quote-item-description"><label><span>Descripción</span><textarea aria-label={`Descripción del ítem ${index + 1}`} maxLength={3000} rows={2} value={row.description} onChange={event => change(index, 'description', event.target.value)} /></label></td>
        <td><Decimal label={`Precio unitario (HNL) del ítem ${index + 1}`} caption="Precio unitario (HNL)" value={row.unit_price} change={value => change(index, 'unit_price', value)} /></td>
        <td><Decimal label={`Cantidad del ítem ${index + 1}`} caption="Cantidad" value={row.quantity} change={value => change(index, 'quantity', value)} /></td>
        <td><div className="quote-item-discount"><Decimal label={`Descuento del ítem ${index + 1}`} caption="Descuento" value={row.discount_value} percent={row.discount_type === 'percent'} change={value => change(index, 'discount_value', value)} /><label><span>Tipo</span><select aria-label={`Tipo de descuento del ítem ${index + 1}`} value={row.discount_type} onChange={event => change(index, 'discount_type', event.target.value as QuoteItem['discount_type'])}><option value="amount">HNL</option><option value="percent">%</option></select></label></div></td>
        <td><label><span>Impuesto</span><select aria-label={`Impuesto del ítem ${index + 1}`} value={row.tax_type} onChange={event => change(index, 'tax_type', event.target.value as QuoteItem['tax_type'])}><option value="none">Exento</option><option value="isv15">ISV 15%</option><option value="isv18">ISV 18%</option></select></label></td>
        <td className="quote-item-total"><span>Total</span><output aria-label={`Total del ítem ${index + 1}`}>{calculation?.data.items.length === items.length && calculation.items[index] ? `L ${calculation.items[index].total}` : '—'}</output></td>
        <td className="quote-item-remove"><button type="button" className="quote-remove" aria-label={`Quitar ítem ${index + 1}`} title={`Quitar ítem ${index + 1}`} onClick={() => remove(index)}><svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" aria-hidden="true"><path d="M3 6h18M9 6V3h6v3M6 6l1 15h10l1-15M10 10v7M14 10v7" /></svg></button></td>
      </tr>)}</tbody>
    </table>{items.length === 0 && <p className="empty">Agrega los productos o servicios de la cotización.</p>}</div>
    <button type="button" className="quote-add-item" disabled={items.length >= 200} onClick={add}><span aria-hidden="true">＋</span> Agregar ítem</button>
    <p className="quote-note">Totales de línea antes del descuento general e impuestos.</p>
  </section>;
}

export function Decimal({ label, caption, value, change, percent = false }: { label: string; caption?: string; value: string; change: (value: string) => void; percent?: boolean }) {
  return <label><span>{caption ?? label}</span><input aria-label={label} type="text" inputMode="decimal" required maxLength={17} pattern={percent ? '(?:[0-9]{1,2}(?:[.][0-9]{1,4})?|100(?:[.]0{1,4})?)' : '[0-9]{1,12}(?:[.][0-9]{1,4})?'} title={percent ? 'De 0 a 100; hasta 4 decimales con punto.' : 'Número sin signo; hasta 4 decimales con punto.'} value={value} onChange={event => change(event.target.value)} /></label>;
}
