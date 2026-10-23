import { useEffect, useRef, useState } from 'react';
import { ApiError, message } from '../api.js';
import { Dialog, Heading, Loading, Notice, Table, useCore, useLoad } from '../ui.js';
import { QuoteActions } from '../components/QuoteActions.js';
import { QuoteEmailComposer } from '../components/QuoteEmailComposer.js';
import { QuoteClientPicker } from '../components/QuoteClientPicker.js';
import { downloadPdf } from '../quoteDocuments.js';
import type { DocumentReadiness, QuotePdf } from '../quoteDocuments.js';
import { moduleCan } from '../types.js';
import { newQuote, newQuoteItem, quoteStatuses } from '../quotes.js';
import type { DiscountType, QuoteDraft, QuoteInput, QuoteItem, QuotePage, QuoteCalculation, QuoteStatus, QuoteSummary } from '../quotes.js';

export default function Cotizaciones() {
  const { access } = useCore();
  if (!moduleCan(access, 'cotizaciones', 'documentos', 'ver')) return <Notice>No tienes acceso a Cotizaciones.</Notice>;
  return <Workspace />;
}

function Workspace() {
  const { api, access, notify } = useCore();
  const [selected, setSelected] = useState<QuoteDraft | 'new' | null>(null);
  const [search, setSearch] = useState(''), [query, setQuery] = useState(''), [page, setPage] = useState(1), [error, setError] = useState(''), [opening, setOpening] = useState(false);
  const [mode, setMode] = useState<'view' | 'edit'>('view'), [deleting, setDeleting] = useState<QuoteSummary | null>(null);
  const [email, setEmail] = useState<QuoteSummary | null>(null);
  const cloneAttempt = useRef<{ id: number; version: number; key: string } | null>(null);
  const editable = moduleCan(access, 'cotizaciones', 'documentos', 'editar');
  const removable = moduleCan(access, 'cotizaciones', 'documentos', 'eliminar');
  const documents = useLoad(signal => api.request<DocumentReadiness>('/api/v1/cotizaciones/document-readiness', { signal }));
  const result = useLoad(signal => api.request<QuotePage>(`/api/v1/cotizaciones?q=${encodeURIComponent(query)}&page=${page}&page_size=25`, { signal }), [query, page], true);
  const open = async (draft: QuoteSummary, nextMode: 'view' | 'edit') => {
    setError(''); setOpening(true);
    try { setSelected(await api.request<QuoteDraft>(`/api/v1/cotizaciones/${draft.id}`)); setMode(nextMode); }
    catch (reason) { setError(message(reason)); } finally { setOpening(false); }
  };
  const status = async (draft: QuoteSummary, value: QuoteStatus) => {
    setError(''); setOpening(true);
    try {
      const updated = await api.request<QuoteDraft>(`/api/v1/cotizaciones/${draft.id}/status`, { method: 'PATCH', body: { status: value }, ifMatch: draft.version });
      result.setData(current => current ? { ...current, items: current.items.map(row => row.id === draft.id ? { ...row, status: updated.status, version: updated.version } : row) } : current);
      notify(`Cotización marcada como ${value}.`);
    } catch (reason) { setError(message(reason)); if (reason instanceof ApiError && reason.status === 412) result.reload(); }
    finally { setOpening(false); }
  };
  const clone = async (draft: QuoteSummary) => {
    setError(''); setOpening(true);
    if (!cloneAttempt.current || cloneAttempt.current.id !== draft.id || cloneAttempt.current.version !== draft.version) cloneAttempt.current = { id: draft.id, version: draft.version, key: crypto.randomUUID() };
    try {
      setSelected(await api.request<QuoteDraft>(`/api/v1/cotizaciones/${draft.id}/clone`, { method: 'POST', ifMatch: draft.version, idempotencyKey: cloneAttempt.current.key }));
      setMode('edit'); cloneAttempt.current = null; notify('Cotización clonada. Completa el cliente de la copia.');
    } catch (reason) { setError(message(reason)); if (reason instanceof ApiError && reason.status === 412) result.reload(); }
    finally { setOpening(false); }
  };
  if (selected) return <Editor key={selected === 'new' ? 'new' : selected.id} initial={selected === 'new' ? null : selected} editable={editable && mode === 'edit'} edit={editable && mode === 'view' ? () => setMode('edit') : undefined} close={() => { setSelected(null); result.reload(); }} />;
  return <><Heading title="Cotizaciones" description="Prepara cotizaciones con descuentos, impuestos y totales calculados por el servidor." action={editable && <button className="primary" onClick={() => { setMode('edit'); setSelected('new'); }}>Nueva cotización</button>} />
    {!documents.data?.pdf && <p className="quote-stage">PDF requiere la configuración de Empresa en Ajustes del Core y el servicio de documentos del servidor.</p>}
    <Notice>{documents.error}</Notice>
    <form className="row quote-search" onSubmit={event => { event.preventDefault(); setQuery(search); setPage(1); }}><label>Buscar cliente, referencia o ítem<input value={search} maxLength={200} onChange={event => setSearch(event.target.value)} /></label><button type="submit">Buscar</button></form>
    <Notice>{error || result.error}</Notice>
    {result.data ? <><Table columns={['Número', 'Cliente', 'Fecha', 'Vence', 'Total (HNL)', 'Estado', 'Ítems', 'Acción']} rows={result.data.items.map(draft => [draft.quote_number ?? 'Sin numerar', draft.client_name || 'Sin cliente', draft.quote_date, draft.valid_until, draft.total, draft.status, draft.item_count, <QuoteActions number={draft.quote_number ?? String(draft.id)} busy={opening} actions={[
      { label: 'Ver', run: () => void open(draft, 'view') },
      { label: 'Editar', disabled: !editable, reason: !editable ? 'Sin permiso de edición' : undefined, run: () => void open(draft, 'edit') },
      { label: 'Marcar como enviado', disabled: !editable, run: () => void status(draft, 'Enviada') },
      { label: 'Convertir a orden de compra', disabled: true, reason: 'Disponible al incorporar Órdenes de compra' },
      { label: 'Convertir a factura', disabled: true, reason: 'Disponible al incorporar Facturas' },
      { label: 'Aprobar', disabled: !editable, run: () => void status(draft, 'Aprobada') },
      { label: 'Clonar', disabled: !editable, run: () => void clone(draft) },
      { label: 'PDF', disabled: !documents.data?.pdf, reason: !documents.data?.pdf ? 'Configura Empresa y el servicio de documentos' : undefined, run: () => { setError(''); setOpening(true); void api.request<QuotePdf>(`/api/v1/cotizaciones/${draft.id}/pdf`, { ifMatch: draft.version }).then(downloadPdf).catch(reason => setError(message(reason))).finally(() => setOpening(false)); } },
      { label: 'Enviar por correo', disabled: !editable || !documents.data?.pdf || !documents.data?.email, reason: !editable ? 'Sin permiso de edición' : !documents.data?.pdf ? 'Configura Empresa y documentos' : !documents.data?.email ? 'Configura SMTP global y tus credenciales en Mi cuenta' : undefined, run: () => setEmail(draft) },
      { label: 'Eliminar', disabled: !removable, danger: true, run: () => setDeleting(draft) },
    ]} />])} /><div className="row"><button disabled={page === 1 || opening} onClick={() => setPage(p => p - 1)}>Anterior</button><span>Página {page}</span><button disabled={!result.data.has_more || opening} onClick={() => setPage(p => p + 1)}>Siguiente</button></div></> : !result.error && <Loading />}
    {email && <QuoteEmailComposer summary={email} close={() => setEmail(null)} sent={result.reload} />}
    {deleting && <Dialog title={`Eliminar ${deleting.quote_number ?? 'cotización'}`} close={() => { if (!opening) setDeleting(null); }}><p>Esta acción elimina la cotización y no se puede deshacer.</p><Notice>{error}</Notice><div className="actions"><button disabled={opening} onClick={() => setDeleting(null)}>Cancelar</button><button disabled={opening} className="danger" onClick={async () => {
      setError(''); setOpening(true);
      try { await api.request(`/api/v1/cotizaciones/${deleting.id}`, { method: 'DELETE', ifMatch: deleting.version }); setDeleting(null); setPage(1); result.reload(); notify('Cotización eliminada.'); }
      catch (reason) { setError(message(reason)); if (reason instanceof ApiError && reason.status === 412) { setDeleting(null); result.reload(); } }
      finally { setOpening(false); }
    }}>{opening ? 'Eliminando…' : 'Eliminar cotización'}</button></div></Dialog>}
  </>;
}

function Editor({ initial, editable, edit, close }: { initial: QuoteDraft | null; editable: boolean; edit?: () => void; close: () => void }) {
  const { api, access, notify } = useCore();
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
  return <><Heading title={saved ? saved.quote_number ?? `Cotización · referencia interna ${saved.id}` : 'Nueva cotización'} description={`Estado: ${data.status} · Moneda: HNL`} action={<div className="actions">{edit && <button onClick={edit}>Editar</button>}<button disabled={busy} onClick={back}>Volver</button></div>} />
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
        <div>{editable && moduleCan(access, 'clientes', 'catalogo', 'ver') ? <QuoteClientPicker selected={data.id_client ? { id: data.id_client, cliente_name: data.client_name, rtn: data.client_rtn ?? '', email: data.client_email ?? '' } : null} change={client => { setData(previous => ({ ...previous, id_client: client?.id ?? null, client_name: client?.cliente_name ?? previous.client_name, client_rtn: client?.rtn ?? '', client_email: client?.email ?? '' })); setDirty(true); }} /> : editable && <p className="muted">El catálogo requiere el módulo Clientes habilitado y permiso para verlo.</p>}
        <label>{data.id_client ? 'Cliente seleccionado' : 'Cliente (manual)'}<input maxLength={255} readOnly={Boolean(data.id_client)} value={data.client_name} onChange={e => change('client_name', e.target.value)} /></label>
        {data.id_client && <><p className="muted">{data.client_rtn && `RTN: ${data.client_rtn} · `}{data.client_email || 'Sin correo registrado'}</p>{editable && !moduleCan(access, 'clientes', 'catalogo', 'ver') && <button type="button" onClick={() => { setData(previous => ({ ...previous, id_client: null, client_rtn: '', client_email: '' })); setDirty(true); }}>Usar cliente manual</button>}</>}</div>
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
