import { useEffect, useRef, useState } from 'react';
import { ApiError, message } from '../api.js';
import { Heading, Loading, Notice, Table, useCore, useLoad } from '../ui.js';
import { moduleCan } from '../types.js';
import { newQuote, newQuoteItem, quoteStatuses } from '../quotes.js';
import type { DiscountType, QuoteDraft, QuoteInput, QuoteItem, QuotePage, QuoteCalculation, QuoteStatus } from '../quotes.js';

export default function Cotizaciones() {
  const { access } = useCore();
  if (!moduleCan(access, 'cotizaciones', 'documentos', 'ver')) return <Notice>No tienes acceso a Cotizaciones.</Notice>;
  return <Workspace />;
}

function Workspace() {
  const { api, access } = useCore();
  const [selected, setSelected] = useState<QuoteDraft | 'new' | null>(null);
  const [search, setSearch] = useState(''), [query, setQuery] = useState(''), [page, setPage] = useState(1), [error, setError] = useState(''), [opening, setOpening] = useState(false);
  const editable = moduleCan(access, 'cotizaciones', 'documentos', 'editar');
  const result = useLoad(signal => api.request<QuotePage>(`/api/v1/cotizaciones?q=${encodeURIComponent(query)}&page=${page}&page_size=25`, { signal }), [query, page]);
  if (selected) return <Editor key={selected === 'new' ? 'new' : selected.id} initial={selected === 'new' ? null : selected} editable={editable} close={() => { setSelected(null); result.reload(); }} />;
  return <><Heading title="Cotizaciones" description="Prepara cotizaciones con descuentos, impuestos y totales calculados por el servidor." action={editable && <button className="primary" onClick={() => setSelected('new')}>Nueva cotización</button>} />
    <p className="quote-stage">Desarrollo local · Cálculo y numeración cotejados con CastilloIT. PDF, adjuntos, correo y conversiones siguen pendientes.</p>
    <form className="row quote-search" onSubmit={event => { event.preventDefault(); setQuery(search); setPage(1); }}><label>Buscar cliente, referencia o ítem<input value={search} maxLength={200} onChange={event => setSearch(event.target.value)} /></label><button type="submit">Buscar</button></form>
    <Notice>{error || result.error}</Notice>
    {result.data ? <><Table columns={['Número', 'Cliente', 'Fecha', 'Vence', 'Total (HNL)', 'Estado', 'Ítems', 'Acción']} rows={result.data.items.map(draft => [draft.quote_number ?? 'Sin numerar', draft.client_name || 'Sin cliente', draft.quote_date, draft.valid_until, draft.total, draft.status, draft.item_count, <button disabled={opening} onClick={async () => { setError(''); setOpening(true); try { setSelected(await api.request<QuoteDraft>(`/api/v1/cotizaciones/${draft.id}`)); } catch (reason) { setError(message(reason)); } finally { setOpening(false); } }}>Abrir</button>])} /><div className="row"><button disabled={page === 1 || opening} onClick={() => setPage(p => p - 1)}>Anterior</button><span>Página {page}</span><button disabled={!result.data.has_more || opening} onClick={() => setPage(p => p + 1)}>Siguiente</button></div></> : !result.error && <Loading />}
  </>;
}

function Editor({ initial, editable, close }: { initial: QuoteDraft | null; editable: boolean; close: () => void }) {
  const { api, notify } = useCore();
  const [saved, setSaved] = useState(initial), [data, setData] = useState<QuoteInput>(() => initial?.data ?? newQuote()), [busy, setBusy] = useState(false), [error, setError] = useState(''), [dirty, setDirty] = useState(false), [conflict, setConflict] = useState(false);
  const [preview, setPreview] = useState<{ input: QuoteInput; value: QuoteCalculation } | null>(null), [calculationError, setCalculationError] = useState('');
  useEffect(() => {
    if (!editable || (saved && !dirty)) return;
    const controller = new AbortController(), input = data;
    setCalculationError('');
    const timer = setTimeout(() => {
      void api.request<QuoteCalculation>('/api/v1/cotizaciones/calculate', { method: 'POST', body: input, signal: controller.signal }).then(value => { if (!controller.signal.aborted) setPreview({ input, value }); }).catch(reason => { if (!controller.signal.aborted) setCalculationError(message(reason)); });
    }, 450);
    return () => { clearTimeout(timer); controller.abort(); };
  }, [api, data, editable, saved, dirty]);
  const calculation = saved && !dirty ? saved.calculation : preview?.input === data ? preview.value : null;
  const attempt = useRef<{ body: string; key: string } | null>(null);
  const change = <K extends keyof QuoteInput>(key: K, value: QuoteInput[K]) => { setData(previous => ({ ...previous, [key]: value })); setDirty(true); };
  const item = <K extends keyof QuoteItem>(index: number, key: K, value: QuoteItem[K]) => change('items', data.items.map((row, i) => i === index ? { ...row, [key]: value } : row));
  const back = () => { if (!busy && (!dirty || confirm('Hay cambios sin guardar. ¿Volver al listado?'))) close(); };
  return <><Heading title={saved ? saved.quote_number ?? `Cotización · referencia interna ${saved.id}` : 'Nueva cotización'} description={`Estado: ${data.status} · Moneda: HNL`} action={<button disabled={busy} onClick={back}>Volver</button>} />
    <p className="quote-stage">El número se asigna al guardar. Los totales de la vista previa usan el mismo cálculo del servidor que guarda la cotización.</p>
    <Notice>{error}</Notice>
    {conflict && <p role="status">Tus cambios se conservan en pantalla. Vuelve al listado y abre la versión actual antes de reintentarlos.</p>}
    <form className="stack" onSubmit={async event => {
      event.preventDefault(); if (busy || !editable || conflict) return;
      setError(''); setBusy(true);
      try {
        let updated: QuoteDraft;
        if (saved) updated = await api.request<QuoteDraft>(`/api/v1/cotizaciones/${saved.id}`, { method: 'PUT', body: data, ifMatch: saved.version });
        else {
          const body = JSON.stringify(data);
          if (!attempt.current || attempt.current.body !== body) attempt.current = { body, key: crypto.randomUUID() };
          updated = await api.request<QuoteDraft>('/api/v1/cotizaciones', { method: 'POST', body: data, idempotencyKey: attempt.current.key });
        }
        setSaved(updated); setData(updated.data); setDirty(false); attempt.current = null; notify('Cotización guardada.');
      } catch (reason) { setError(message(reason)); if (reason instanceof ApiError && reason.status === 412) setConflict(true); }
      finally { setBusy(false); }
    }}>
      <fieldset disabled={busy || !editable || conflict} className="form-fields quote-form"><legend>Datos de cotización</legend><div className="quote-fields">
        <label>Cliente (manual)<input maxLength={255} value={data.client_name} onChange={e => change('client_name', e.target.value)} /></label>
        <label>Referencia de orden<input maxLength={100} value={data.order_number} onChange={e => change('order_number', e.target.value)} /></label>
        <label>Fecha<input type="date" min="1000-01-01" max="9999-12-31" required value={data.quote_date} onChange={e => change('quote_date', e.target.value)} /></label>
        <label>Vigencia (días)<input type="number" min={0} max={3650} required value={data.valid_days} onChange={e => change('valid_days', Number(e.target.value))} /></label>
        <label>Estado<select value={data.status} onChange={e => change('status', e.target.value as QuoteStatus)}>{quoteStatuses.map(status => <option key={status}>{status}</option>)}</select></label>
        <div><strong>Contacto / preparado por</strong><p>{saved ? `${saved.data.prepared_by_name} · ${saved.data.contact_email}` : 'Se asigna al usuario que guarda la cotización.'}</p></div>
      </div>
      <div className="row"><h2>Ítems</h2><button type="button" disabled={data.items.length >= 200} onClick={() => change('items', [...data.items, newQuoteItem()])}>Agregar ítem</button></div>
      {data.items.length === 0 && <p className="muted">Agrega los productos o servicios de la cotización.</p>}
      {data.items.map((row, index) => <fieldset className="quote-item" key={index}><legend>Ítem {index + 1}</legend><label>Descripción<textarea maxLength={3000} rows={2} value={row.description} onChange={e => item(index, 'description', e.target.value)} /></label><div className="quote-fields">
        <Decimal label="Cantidad" value={row.quantity} change={value => item(index, 'quantity', value)} />
        <Decimal label="Precio unitario (HNL)" value={row.unit_price} change={value => item(index, 'unit_price', value)} />
        <label>Descuento<select value={row.discount_type} onChange={e => item(index, 'discount_type', e.target.value as DiscountType)}><option value="amount">Monto (HNL)</option><option value="percent">Porcentaje</option></select></label>
        <Decimal label="Valor del descuento" value={row.discount_value} percent={row.discount_type === 'percent'} change={value => item(index, 'discount_value', value)} />
        <label>Impuesto<select value={row.tax_type} onChange={e => item(index, 'tax_type', e.target.value as QuoteItem['tax_type'])}><option value="none">Exento</option><option value="isv15">ISV 15%</option><option value="isv18">ISV 18%</option></select></label>
      </div><button type="button" className="danger" onClick={() => change('items', data.items.filter((_, i) => i !== index))}>Quitar ítem</button></fieldset>)}
      <h2>Descuento general</h2><div className="quote-fields"><label>Tipo<select value={data.discount_type} onChange={e => change('discount_type', e.target.value as DiscountType)}><option value="amount">Monto (HNL)</option><option value="percent">Porcentaje</option></select></label><Decimal label="Valor del descuento general" value={data.discount_value} percent={data.discount_type === 'percent'} change={value => change('discount_value', value)} /></div>
      <label>Notas privadas<textarea rows={3} maxLength={3000} value={data.notes} onChange={e => change('notes', e.target.value)} /></label>
      <label>Términos<textarea rows={3} maxLength={5000} value={data.terms} onChange={e => change('terms', e.target.value)} /></label>
      {editable && <button className="primary" type="submit">{busy ? 'Guardando…' : 'Guardar cotización'}</button>}
      </fieldset>
    </form>
    <Notice>{calculationError}</Notice>
    {calculation ? <QuoteTotalsPanel calculation={calculation} /> : !calculationError && <p role="status">Calculando totales…</p>}
  </>;
}

export function QuoteTotalsPanel({ calculation }: { calculation: QuoteCalculation }) {
  const t = calculation.totals;
  const entries = [['Subtotal', t.subtotal], ['Importe exonerado', t.importe_exonerado], ['Importe exento', t.importe_exento], ['Importe gravado 15%', t.importe_gravado_15], ['Importe gravado 18%', t.importe_gravado_18], ['Descuentos y rebajas', t.descuentos_rebajas], ['ISV 15%', t.isv_15], ['ISV 18%', t.isv_18], ['Total a pagar', t.total]];
  return <section className="card quote-totals" aria-label="Totales de cotización"><h2>Totales</h2><p>Válida hasta {calculation.valid_until} · HNL</p><dl>{entries.map(([label, value]) => <div key={label}><dt>{label}</dt><dd>L {value}</dd></div>)}</dl></section>;
}

function Decimal({ label, value, change, positive = false, percent = false }: { label: string; value: string; change: (value: string) => void; positive?: boolean; percent?: boolean }) {
  return <label>{label}<input type="text" inputMode="decimal" required maxLength={17} pattern={percent ? '(?:[0-9]{1,2}(?:[.][0-9]{1,4})?|100(?:[.]0{1,4})?)' : '[0-9]{1,12}(?:[.][0-9]{1,4})?'} title={percent ? 'De 0 a 100; hasta 4 decimales con punto.' : 'Número sin signo; hasta 4 decimales con punto.'} value={value} onChange={e => { e.target.setCustomValidity(positive && /^0+(?:\.0+)?$/.test(e.target.value) ? 'La cantidad debe ser mayor que cero.' : ''); change(e.target.value); }} /></label>;
}
