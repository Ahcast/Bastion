import { useEffect, useRef, useState } from 'react';
import type { ProcessQuoteSeed } from '../processQuote.js';
import { ProcessQuoteNotice } from '../components/ProcessQuoteNotice.js';
import { ApiError, message } from '../api.js';
import { Dialog, Heading, Loading, Notice, Table, useCore, useLoad } from '../ui.js';
import { QuoteConversionDialog } from '../components/QuoteConversionDialog.js';
import type { ConversionModule, ConversionAttempt, OpenConverted } from '../conversions.js';
import { QuoteActions } from '../components/QuoteActions.js';
import { QuoteEmailComposer } from '../components/QuoteEmailComposer.js';
import { QuoteClientPicker } from '../components/QuoteClientPicker.js';
import { QuoteAttachments } from '../components/QuoteAttachments.js';
import { Decimal, QuoteItems } from '../components/QuoteItems.js';
import { reservePdfViewer } from '../quoteDocuments.js';
import type { DocumentReadiness, QuotePdf } from '../quoteDocuments.js';
import { moduleCan } from '../types.js';
import { useUnsavedChanges } from '../navigationGuard.js';
import { newQuote, newQuoteItem, quoteStatuses } from '../quotes.js';
import type { DiscountType, QuoteDraft, QuoteInput, QuoteItem, QuotePage, QuoteCalculation, QuoteStatus, QuoteSummary } from '../quotes.js';

interface ProcessQuoteProps { openConverted?: OpenConverted; processSeed?: ProcessQuoteSeed | null; clearProcessSeed?: () => void }

export default function Cotizaciones({ processSeed, clearProcessSeed, openConverted }: ProcessQuoteProps = {}) {
  const { access } = useCore();
  if (!moduleCan(access, 'cotizaciones', 'documentos', 'ver')) return <Notice>No tienes acceso a Cotizaciones.</Notice>;
  return <Workspace processSeed={processSeed} clearProcessSeed={clearProcessSeed} openConverted={openConverted} />;
}

function Workspace({ processSeed, clearProcessSeed, openConverted }: ProcessQuoteProps) {
  const [seed, setSeed] = useState(processSeed);
  const { api, access, notify } = useCore();
  const [selected, setSelected] = useState<QuoteDraft | 'new' | null>(processSeed ? 'new' : null);
  const [search, setSearch] = useState(''), [query, setQuery] = useState(''), [page, setPage] = useState(1), [error, setError] = useState(''), [opening, setOpening] = useState(false);
  const [mode, setMode] = useState<'view' | 'edit'>(processSeed ? 'edit' : 'view'), [deleting, setDeleting] = useState<QuoteSummary | null>(null);
  const [email, setEmail] = useState<QuoteSummary | null>(null);
  const [conversion, setConversion] = useState<{ summary: QuoteSummary; kind: ConversionModule } | null>(null);
  const conversionAttempts = useRef(new Map<string, ConversionAttempt>());
  const cloneAttempt = useRef<{ id: number; version: number; key: string } | null>(null);
  const editable = moduleCan(access, 'cotizaciones', 'documentos', 'editar');
  const removable = moduleCan(access, 'cotizaciones', 'documentos', 'eliminar');
  const targetAllowed = (module: ConversionModule) => editable && moduleCan(access, module, 'documentos', 'ver') && moduleCan(access, module, 'documentos', 'editar');
  const targetReason = (module: ConversionModule) => !editable ? 'Sin permiso de edición de Cotizaciones' : !targetAllowed(module) ? 'Requiere el módulo destino activo y permisos de lectura y edición' : undefined;
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
  const previewPdf = (draft: QuoteSummary) => {
    setError(''); let viewer: ReturnType<typeof reservePdfViewer>;
    try { viewer = reservePdfViewer(); } catch (reason) { setError(message(reason)); return; }
    setOpening(true);
    void api.request<QuotePdf>(`/api/v1/cotizaciones/${draft.id}/pdf`, { ifMatch: draft.version }).then(value => viewer.show(value))
      .catch(reason => { viewer.close(); setError(message(reason)); }).finally(() => setOpening(false));
  };
  if (selected) return <Editor key={selected === 'new' ? 'new' : selected.id} initial={selected === 'new' ? null : selected} seed={seed} editable={editable && mode === 'edit'} edit={editable && mode === 'view' ? () => setMode('edit') : undefined} close={() => { setSelected(null); setSeed(null); clearProcessSeed?.(); result.reload(); }} />;
  return <><Heading title="Cotizaciones" description="Prepara y consulta tus cotizaciones." action={editable && <button className="primary" onClick={() => { setMode('edit'); setSelected('new'); }}>Nueva cotización</button>} />
    {documents.data && !documents.data.pdf && <p className="quote-stage">PDF requiere la configuración de Empresa en Ajustes del Core y el servicio de documentos del servidor.</p>}
    <Notice>{documents.error}</Notice>
    <section className="quote-list" aria-labelledby="quote-list-title"><div className="quote-list-heading"><h2 id="quote-list-title">Listado de cotizaciones</h2>{result.data && <span>{result.data.items.length} en esta página</span>}</div>
    <form className="quote-search" onSubmit={event => { event.preventDefault(); setQuery(search); setPage(1); }}><label><span className="quote-sr-only">Buscar cliente, referencia o ítem</span><input placeholder="Buscar por cliente, referencia o ítem" value={search} maxLength={200} onChange={event => setSearch(event.target.value)} /></label><button type="submit">Buscar</button></form>
    <Notice>{error || result.error}</Notice>
    {result.data ? <><Table columns={['Número', 'Cliente', 'Fecha', 'Vence', 'Total (HNL)', 'Estado', 'Ítems', 'Acción']} rows={result.data.items.map(draft => [<span className="quote-number">{draft.quote_number ?? 'Sin numerar'}</span>, <div className="quote-list-client">{draft.client_name || 'Sin cliente'}{draft.order_number && <small>{draft.order_number}</small>}</div>, draft.quote_date, draft.valid_until, <span className="quote-money">{draft.total ?? '—'}</span>, <span className="quote-status" data-state={draft.status}>{draft.status}</span>, draft.item_count, <QuoteActions number={draft.quote_number ?? String(draft.id)} busy={opening} actions={[
      { label: 'Ver', run: () => void open(draft, 'view') },
      { label: 'Editar', disabled: !editable, reason: !editable ? 'Sin permiso de edición' : undefined, run: () => void open(draft, 'edit') },
      { label: 'Marcar como enviado', disabled: !editable, run: () => void status(draft, 'Enviada') },
      { label: 'Convertir a orden de compra', disabled: !targetAllowed('ordenes_compra'), reason: targetReason('ordenes_compra'), run: () => setConversion({ summary: draft, kind: 'ordenes_compra' }) },
      { label: 'Convertir a factura', disabled: !targetAllowed('facturas'), reason: targetReason('facturas'), run: () => setConversion({ summary: draft, kind: 'facturas' }) },
      { label: 'Aprobar', disabled: !editable, run: () => void status(draft, 'Aprobada') },
      { label: 'Clonar', disabled: !editable, run: () => void clone(draft) },
      { label: 'PDF', disabled: !documents.data?.pdf, reason: !documents.data?.pdf ? 'Configura Empresa y el servicio de documentos' : undefined, run: () => previewPdf(draft) },
      { label: 'Enviar por correo', disabled: !editable || !documents.data?.pdf || !documents.data?.email, reason: !editable ? 'Sin permiso de edición' : !documents.data?.pdf ? 'Configura Empresa y documentos' : !documents.data?.email ? 'Configura SMTP global y tus credenciales en Mi cuenta' : undefined, run: () => setEmail(draft) },
      { label: 'Eliminar', disabled: !removable, danger: true, run: () => setDeleting(draft) },
    ]} />])} /><div className="quote-pagination"><button disabled={page === 1 || opening} onClick={() => setPage(p => p - 1)}>Anterior</button><span>Página {page}</span><button disabled={!result.data.has_more || opening} onClick={() => setPage(p => p + 1)}>Siguiente</button></div></> : !result.error && <Loading />}
    </section>
    {conversion && <QuoteConversionDialog summary={conversion.summary} kind={conversion.kind} attempts={conversionAttempts.current} close={() => setConversion(null)} converted={value => {
      setConversion(null); result.reload(); notify(value.reused ? 'Se abrió la factura existente de esta cotización.' : 'Conversión guardada. Completa los costos del documento.');
      openConverted?.(value.target_module,value.target_id);
    }} />}
    {email && <QuoteEmailComposer summary={email} close={() => setEmail(null)} sent={result.reload} />}
    {deleting && <Dialog title={`Eliminar ${deleting.quote_number ?? 'cotización'}`} close={() => { if (!opening) setDeleting(null); }}><p>Esta acción elimina la cotización y no se puede deshacer.</p><Notice>{error}</Notice><div className="actions"><button disabled={opening} onClick={() => setDeleting(null)}>Cancelar</button><button disabled={opening} className="danger" onClick={async () => {
      setError(''); setOpening(true);
      try { await api.request(`/api/v1/cotizaciones/${deleting.id}`, { method: 'DELETE', ifMatch: deleting.version }); setDeleting(null); setPage(1); result.reload(); notify('Cotización eliminada.'); }
      catch (reason) { setError(message(reason)); if (reason instanceof ApiError && reason.status === 412) { setDeleting(null); result.reload(); } }
      finally { setOpening(false); }
    }}>{opening ? 'Eliminando…' : 'Eliminar cotización'}</button></div></Dialog>}
  </>;
}

function Editor({ initial, editable, edit, close, seed }: { seed?: ProcessQuoteSeed | null; initial: QuoteDraft | null; editable: boolean; edit?: () => void; close: () => void }) {
  const { api, access, notify } = useCore();
  const [saved, setSaved] = useState(initial), [data, setData] = useState<QuoteInput>(() => initial?.data ?? seed?.data ?? newQuote()), [busy, setBusy] = useState(false), [error, setError] = useState(''), [dirty, setDirty] = useState(Boolean(seed)), [conflict, setConflict] = useState(false);
  const [attachmentBusy, setAttachmentBusy] = useState(false), [pendingFiles, setPendingFiles] = useState(0);
  useUnsavedChanges(dirty || pendingFiles > 0, busy || attachmentBusy);
  const documents = useLoad(signal => api.request<DocumentReadiness>('/api/v1/cotizaciones/document-readiness', { signal }));
  const [preview, setPreview] = useState<{ input: QuoteInput; value: QuoteCalculation } | null>(null), [calculationError, setCalculationError] = useState('');
  const missingQuantities = seed ? data.items.filter(item => !item.quantity.trim()).length : 0;
  useEffect(() => {
    if (!editable || (saved && !dirty) || missingQuantities) return;
    const controller = new AbortController(), input = data;
    setCalculationError('');
    const timer = setTimeout(() => {
      void api.request<QuoteCalculation>('/api/v1/cotizaciones/calculate', { method: 'POST', body: input, signal: controller.signal }).then(value => { if (!controller.signal.aborted) setPreview({ input, value }); }).catch(reason => { if (!controller.signal.aborted) setCalculationError(message(reason)); });
    }, 450);
    return () => { clearTimeout(timer); controller.abort(); };
  }, [api, data, editable, saved, dirty, missingQuantities]);
  const calculation = saved && !dirty ? saved.calculation : preview?.input === data ? preview.value : null;
  const attempt = useRef<{ body: string; key: string } | null>(null);
  const change = <K extends keyof QuoteInput>(key: K, value: QuoteInput[K]) => { setData(previous => ({ ...previous, [key]: value })); setDirty(true); };
  const item = <K extends keyof QuoteItem>(index: number, key: K, value: QuoteItem[K]) => change('items', data.items.map((row, i) => i === index ? { ...row, [key]: value } : row));
  const back = () => { if (!busy && !attachmentBusy && (!(dirty || pendingFiles) || confirm('Hay cambios sin guardar. ¿Volver al listado?'))) close(); };
  return <><Heading title={saved ? saved.quote_number ?? `Cotización · referencia interna ${saved.id}` : 'Nueva cotización'} description={`Estado: ${data.status} · Moneda: HNL`} action={<div className="actions">{edit && <button onClick={edit}>Editar</button>}<button disabled={busy || attachmentBusy} onClick={back}>Volver</button></div>} />
    {!saved && <p className="quote-note">El número se asigna al guardar.</p>}
    {seed && <ProcessQuoteNotice seed={seed} missingQuantities={missingQuantities} />}
    <Notice>{error}</Notice>
    {conflict && <p role="status">Tus cambios se conservan en pantalla. Vuelve al listado y abre la versión actual antes de reintentarlos.</p>}
    <form className="quote-editor" onSubmit={async event => {
      event.preventDefault(); if (busy || attachmentBusy || !editable || conflict) return;
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
      <fieldset disabled={busy || attachmentBusy || !editable || conflict} className="form-fields quote-form"><legend className="quote-sr-only">Datos de cotización</legend>
        <section className="quote-section" aria-labelledby="quote-general-title">
          <div className="quote-section-heading"><h2 id="quote-general-title">Generales</h2><span>HNL</span></div>
          <div className="quote-general-fields">
            <QuoteClientPicker value={data.client_name} selected={data.id_client ? { id: data.id_client, cliente_name: data.client_name, rtn: data.client_rtn ?? '', email: data.client_email ?? '' } : null}
              canSearch={moduleCan(access, 'clientes', 'catalogo', 'ver')} readOnly={!editable}
              change={(name, client) => { setData(previous => ({ ...previous, id_client: client?.id ?? null, client_name: name, client_rtn: client?.rtn ?? '', client_email: client?.email ?? '' })); setDirty(true); }} />
            <label className="quote-order-reference">Referencia de orden<input maxLength={100} value={data.order_number} onChange={event => change('order_number', event.target.value)} /></label>
            <label>Fecha<input type="date" min="1000-01-01" max="9999-12-31" required value={data.quote_date} onChange={event => change('quote_date', event.target.value)} /></label>
            <label>Vigencia (días)<input type="number" min={0} max={3650} required value={data.valid_days} onChange={event => change('valid_days', Number(event.target.value))} /></label>
            <label>Estado<select value={data.status} onChange={event => change('status', event.target.value as QuoteStatus)}>{quoteStatuses.map(status => <option key={status}>{status}</option>)}</select></label>
          </div>
          <p className="quote-prepared"><span>Preparado por</span> {saved ? `${saved.data.prepared_by_name} · ${saved.data.contact_email}` : 'Se asignará al usuario que guarda la cotización.'}</p>
        </section>
        <QuoteItems items={data.items} calculation={calculation} change={item} remove={index => change('items', data.items.filter((_, i) => i !== index))} add={() => change('items', [...data.items, newQuoteItem()])} />
        <div className="quote-editor-bottom">
          <section className="quote-section" aria-labelledby="quote-notes-title">
            <div className="quote-section-heading"><h2 id="quote-notes-title">Notas y términos</h2></div>
            <div className="stack"><label>Notas privadas<textarea rows={3} maxLength={3000} value={data.notes} onChange={event => change('notes', event.target.value)} /></label>
            <label>Términos<textarea rows={3} maxLength={5000} value={data.terms} onChange={event => change('terms', event.target.value)} /></label></div>
          </section>
          <div className="quote-summary">
            <section className="quote-section" aria-labelledby="quote-discount-title"><div className="quote-section-heading"><h2 id="quote-discount-title">Descuento general</h2></div><div className="quote-discount-fields"><label>Tipo<select value={data.discount_type} onChange={event => change('discount_type', event.target.value as DiscountType)}><option value="amount">Monto (HNL)</option><option value="percent">Porcentaje</option></select></label><Decimal label="Valor del descuento general" value={data.discount_value} percent={data.discount_type === 'percent'} change={value => change('discount_value', value)} /></div></section>
            <Notice>{calculationError}</Notice>
            {calculation ? <QuoteTotalsPanel calculation={calculation} /> : !calculationError && !missingQuantities && <p role="status">Calculando totales…</p>}
          </div>
        </div>

      </fieldset>
      <QuoteAttachments quoteId={saved?.id ?? null} version={saved?.version ?? 0} editable={editable} disabled={busy || conflict}
        storageReady={Boolean(documents.data?.attachments ?? documents.data?.pdf)} busyChanged={setAttachmentBusy} pendingChanged={setPendingFiles}
        versionChanged={version => setSaved(previous => previous ? { ...previous, version } : previous)} conflict={() => setConflict(true)} />
      {editable && <div className="quote-save"><button disabled={busy || attachmentBusy || conflict} className="primary" type="submit">{busy ? 'Guardando…' : 'Guardar cotización'}</button></div>}
    </form>
  </>;
}

export function QuoteTotalsPanel({ calculation }: { calculation: QuoteCalculation }) {
  const t = calculation.totals;
  const entries = [['Subtotal', t.subtotal], ['Importe exonerado', t.importe_exonerado], ['Importe exento', t.importe_exento], ['Importe gravado 15%', t.importe_gravado_15], ['Importe gravado 18%', t.importe_gravado_18], ['Descuentos y rebajas', t.descuentos_rebajas], ['ISV 15%', t.isv_15], ['ISV 18%', t.isv_18], ['Total a pagar', t.total]];
  return <section className="quote-totals" aria-label="Totales de cotización"><h2>Totales</h2><p className="quote-note">Válida hasta {calculation.valid_until} · HNL</p><dl>{entries.map(([label, value]) => <div key={label}><dt>{label}</dt><dd>L {value}</dd></div>)}</dl></section>;
}
