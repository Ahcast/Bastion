import { test } from 'node:test';
import assert from 'node:assert/strict';
import { renderToStaticMarkup } from 'react-dom/server';
import { ApiClient } from '../src/api.js';
import { CoreContext } from '../src/ui.js';
import Cotizaciones, { QuoteTotalsPanel } from '../src/pages/Cotizaciones.js';
import { newQuote } from '../src/quotes.js';

test('quote dates use the Honduras calendar at UTC midnight', () => {
  const data = newQuote(new Date('2026-10-23T02:00:00Z'));
  assert.equal(data.quote_date, '2026-10-22');
  assert.equal(data.valid_days, 30);
  assert.equal(data.terms, 'Tiempo de entrega: ');
  assert.equal(data.items[0]?.tax_type, 'isv15');
  assert.equal(data.status, 'Borrador');
  assert.equal(data.discount_value, '0');
  assert.ok(!('quote_number' in data));
  assert.ok(!('total' in data));
});

test('the draft screen distinguishes read access, edit access and unavailable module', () => {
  for (const [active, edit] of [[true, true], [true, false], [false, true]]) {
    const access = { is_system_administrator: false, modules: active ? [{ module_id: 'cotizaciones', display_name: 'Cotizaciones' }] : [], permissions: [{ module_id: 'cotizaciones', scope_key: 'documentos', capability: 'ver' }, ...(edit ? [{ module_id: 'cotizaciones', scope_key: 'documentos', capability: 'editar' }] : [])] };
    const markup = renderToStaticMarkup(<CoreContext.Provider value={{ api: new ApiClient('https://example.test'), access, notify: () => {} }}><Cotizaciones /></CoreContext.Provider>);
    assert.equal(markup.includes('Nueva cotización'), active && edit);
    assert.equal(markup.includes('Cálculo y numeración cotejados'), active);
    assert.equal(markup.includes('No tienes acceso'), !active);
  }
});

test('server totals render without converting decimal strings to binary numbers', () => {
  const calculation = { data: newQuote(), valid_until: '2026-11-21', items: [], totals: { subtotal: '100.00', importe_exonerado: '0.00', importe_exento: '0.00', importe_gravado_15: '100.00', importe_gravado_18: '0.00', descuentos_rebajas: '0.00', isv_15: '15.00', isv_18: '0.00', total: '115.00' } };
  const markup = renderToStaticMarkup(<QuoteTotalsPanel calculation={calculation} />);
  assert.ok(markup.includes('2026-11-21'));
  assert.ok(markup.includes('Total a pagar'));
  assert.ok(markup.includes('L 115.00'));
  assert.ok(markup.includes('L 15.00'));
});
