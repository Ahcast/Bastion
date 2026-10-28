import { useEffect, useRef, useState } from 'react';
import { ApiError, message } from '../api.js';
import { Heading, Loading, Notice, Table, useCore, useLoad } from '../ui.js';
import { QuoteActions as InvoiceActions } from '../components/QuoteActions.js';
import { QuoteClientPicker } from '../components/QuoteClientPicker.js';
import { InvoiceCommandDialog, InvoicePaymentFields, newPayment } from '../components/InvoiceCommands.js';
import { reservePdfViewer } from '../quoteDocuments.js';
import type { QuotePdf } from '../quoteDocuments.js';
import { Decimal } from '../components/QuoteItems.js';
import { InvoiceItems } from '../components/InvoiceItems.js';
import { useNavigationGuard, useUnsavedChanges } from '../navigationGuard.js';
import { moduleCan } from '../types.js';
import { newInvoice, newInvoiceItem } from '../invoices.js';
import type { InvoiceCalculation, InvoiceDraft, InvoiceInput, InvoiceItem, InvoicePage, InvoiceSummary } from '../invoices.js';
import type { DiscountType } from '../quotes.js';

export default function Facturas({ initialId }: { initialId?: number } = {}) {
  const { access } = useCore();
  if (!moduleCan(access, 'facturas', 'documentos', 'ver')) return <Notice>No tienes acceso a Facturas.</Notice>;
  return <Workspace initialId={initialId} />;
}

const stage = 'El número SAR se asigna al marcar como enviado. Las facturas enviadas y pagadas no se editan ni eliminan.';

function Workspace({ initialId }: { initialId?: number }) {
  const { api, access, notify } = useCore();
  const [command, setCommand] = useState<{invoice: InvoiceSummary; action: 'sent' | 'payment' | 'delete'} | null>(null);
  const cloneAttempts = useRef(new Map<string,string>());
  const documents = useLoad(signal => api.request<{pdf:boolean}>('/api/v1/facturas/document-readiness', {signal}));
  const [selected, setSelected] = useState<InvoiceDraft | 'new' | null>(null), [mode, setMode] = useState<'view' | 'edit'>('view');
  const [search, setSearch] = useState(''), [query, setQuery] = useState(''), [page, setPage] = useState(1), [error, setError] = useState(''), [busy, setBusy] = useState(false);
  const editable = moduleCan(access, 'facturas', 'documentos', 'editar');
  const removable = moduleCan(access, 'facturas', 'documentos', 'eliminar');
  const result = useLoad(signal => api.request<InvoicePage>(`/api/v1/facturas?search=${encodeURIComponent(query)}&page=${page}&page_size=25`, { signal }), [query, page]);
  const open = async (id: number, next: 'view' | 'edit') => {
    setError(''); setBusy(true);
    try { setSelected(await api.request<InvoiceDraft>(`/api/v1/facturas/${id}`)); setMode(next); }
    catch (reason) { setError(message(reason)); } finally { setBusy(false); }
  };
  const clone = async (row: InvoiceSummary) => {
    const identity = `${row.id}:${row.version}`;
    if (!cloneAttempts.current.has(identity)) cloneAttempts.current.set(identity,crypto.randomUUID());
    setBusy(true);setError('');
    try { const copy = await api.request<InvoiceDraft>(`/api/v1/facturas/${row.id}/clone`,{method:'POST',ifMatch:row.version,idempotencyKey:cloneAttempts.current.get(identity)!}); cloneAttempts.current.delete(identity);setSelected(copy);setMode('edit');notify('Borrador clonado sin pagos, numeración ni relaciones de origen.'); }
    catch(reason) {setError(message(reason));if(reason instanceof ApiError && reason.status===412) result.reload();} finally{setBusy(false);}
  };
  const pdf = (row: InvoiceSummary, kind: string) => {
    let viewer: ReturnType<typeof reservePdfViewer>;
    try {viewer=reservePdfViewer();} catch(reason){setError(message(reason));return;}
    setBusy(true);setError('');
    void api.request<QuotePdf>(`/api/v1/facturas/${row.id}/documents/${kind}`,{method:'POST',ifMatch:row.version}).then(value=>viewer.show(value)).catch(reason=>{viewer.close();setError(message(reason));if(reason instanceof ApiError && reason.status===412) result.reload();}).finally(()=>setBusy(false));
  };
  useEffect(() => { if (initialId) void open(initialId, 'edit'); }, [initialId, api]);
  if (selected) return <InvoiceEditor key={selected === 'new' ? 'new' : selected.id} initial={selected === 'new' ? null : selected} editable={editable && mode === 'edit' && (selected==='new' || selected.status==='Borrador')} edit={editable && selected!=='new' && selected.status==='Borrador' && mode === 'view' ? () => setMode('edit') : undefined} close={() => { setSelected(null); result.reload(); }} />;
  return <><Heading title="Facturas" description="Administra facturas, documentos, pagos y saldos." action={editable && <button className="primary" disabled={busy} onClick={() => { setSelected('new'); setMode('edit'); }}>Nueva factura</button>} />
    <p className="quote-stage">{stage}</p>
    <section className="quote-list invoice-list" aria-labelledby="invoice-list-title"><div className="quote-list-heading"><h2 id="invoice-list-title">Listado de facturas</h2>{result.data && <span>{result.data.items.length} en esta página</span>}</div>
    <form className="quote-search" onSubmit={e => { e.preventDefault(); setQuery(search); setPage(1); }}><label><span className="quote-sr-only">Buscar factura, cliente o referencia</span><input placeholder="Buscar factura, cliente o referencia" maxLength={200} value={search} onChange={e => setSearch(e.target.value)} /></label><button disabled={busy}>Buscar</button></form>
    <Notice>{error || result.error}</Notice>{initialId && error && <button disabled={busy} onClick={() => void open(initialId, 'edit')}>Reintentar abrir documento convertido</button>}{result.error && <button onClick={result.reload}>Reintentar consulta</button>}
    {result.data ? <><Table columns={['Factura / referencia', 'Cliente', 'Fecha', 'Ítems', 'Total (HNL)', 'Saldo (HNL)', 'Estado', 'Acciones']} rows={result.data.items.map(row => [<span className="quote-number">{row.invoice_number ?? `Borrador ${row.id}`}</span>, <div className="quote-list-client">{row.client_name || 'Sin cliente'}{row.order_number && <small>{row.order_number}</small>}</div>, row.invoice_date, row.item_count, <span className="quote-money">{row.total}</span>, <span className="quote-money">{row.balance}</span>, <span className="quote-status" data-state={row.status}>{row.status}</span>, <InvoiceActions number={`borrador de factura ${row.id}`} busy={busy} actions={[
      { label: 'Ver', run: () => void open(row.id, 'view') },
      { label: 'Editar', disabled: !editable || row.status!=='Borrador', reason: !editable ? 'Sin permiso de edición' : row.status!=='Borrador' ? 'Una factura emitida no se edita' : undefined, run: () => void open(row.id, 'edit') },
      { label: 'Marcar como enviado', disabled: !editable || row.status!=='Borrador', reason: !editable ? 'Sin permiso de edición' : row.status!=='Borrador' ? 'La factura ya fue emitida' : undefined, run:()=>setCommand({invoice:row,action:'sent'}) },
      { label: 'Ingresar pago', disabled: !editable || row.status!=='Enviado' || row.balance==='0.00', reason: !editable ? 'Sin permiso de edición' : row.status==='Borrador' ? 'Primero marca la factura como enviada' : row.balance==='0.00' ? 'Sin saldo pendiente' : undefined, run:()=>setCommand({invoice:row,action:'payment'}) },
      { label: 'Clonar', disabled: !editable, reason: !editable ? 'Sin permiso de edición' : undefined, run:()=>void clone(row) },
      { label: 'PDF', disabled: !documents.data?.pdf, reason: !documents.data?.pdf ? 'Configura Empresa y el servicio de documentos' : undefined, run:()=>pdf(row,'invoice') },
      { label: 'Nota de entrega', disabled: !documents.data?.pdf, reason: !documents.data?.pdf ? 'Configura Empresa y el servicio de documentos' : undefined, run:()=>pdf(row,'delivery') },
      { label: 'Emitir Recibo', disabled: !documents.data?.pdf, reason: !documents.data?.pdf ? 'Configura Empresa y el servicio de documentos' : undefined, run:()=>pdf(row,'receipt') },
      { label: 'Eliminar', disabled: !removable || row.status!=='Borrador' || (!!row.paid_total && row.paid_total!=='0.00'), danger: true, reason: !removable ? 'Sin permiso de eliminación' : row.status!=='Borrador' ? 'Una factura emitida no se elimina' : row.paid_total!=='0.00' ? 'No se elimina un borrador con pagos' : undefined, run:()=>setCommand({invoice:row,action:'delete'}) },
    ]} />])} /><nav className="quote-pagination" aria-label="Paginación de facturas"><button disabled={busy || page === 1} onClick={() => setPage(p => p - 1)}>Anterior</button><span role="status">Página {page}</span><button disabled={busy || !result.data.has_more} onClick={() => setPage(p => p + 1)}>Siguiente</button></nav></> : !result.error && <Loading />}</section>
    {command && <InvoiceCommandDialog invoice={command.invoice} action={command.action} close={()=>setCommand(null)} changed={result.reload} completed={value=>{setCommand(null);result.reload();if(value){setSelected(value);setMode('view');}notify(command.action==='delete'?'Borrador eliminado.':'Factura actualizada.');}} />}
  </>;
}

export function InvoiceEditor({ initial, editable, edit, close }: { initial: InvoiceDraft | null; editable: boolean; edit?: () => void; close: () => void }) {
  const { api, access, notify } = useCore(), { navigate } = useNavigationGuard();
  const [saved, setSaved] = useState(initial), [data, setData] = useState<InvoiceInput>(() => initial?.data ?? newInvoice());
  const [dirty, setDirty] = useState(false), [busy, setBusy] = useState(false), [conflict, setConflict] = useState(false), [error, setError] = useState('');
  const [preview, setPreview] = useState<{ input: InvoiceInput; value: InvoiceCalculation } | null>(null), [calculationError, setCalculationError] = useState('');
  const attempt = useRef<{ body: string; key: string } | null>(null);
  const heading = useRef<HTMLHeadingElement>(null);
  useUnsavedChanges(dirty, busy);
  useEffect(() => { heading.current?.focus(); }, []);
  useEffect(() => {
    if (!editable || (saved && !dirty) || conflict) return;
    const controller = new AbortController(), input = data;
    setCalculationError('');
    const timer = setTimeout(() => {
      void api.request<InvoiceCalculation>(saved ? `/api/v1/facturas/${saved.id}/calculate` : '/api/v1/facturas/calculate', { method: 'POST', body: input, ifMatch:saved?.version, signal: controller.signal }).then(value => { if (!controller.signal.aborted) setPreview({ input, value }); }).catch(reason => { if (!controller.signal.aborted) setCalculationError(message(reason)); });
    }, 450);
    return () => { clearTimeout(timer); controller.abort(); };
  }, [api, data, editable, saved, dirty, conflict]);
  const calculation = saved && !dirty ? saved.calculation : preview?.input === data ? preview.value : null;
  const cashPending = data.payment_term_type === 'contado' && (!calculation || calculation.balance !== '0.00');
  const change = <K extends keyof InvoiceInput>(key: K, value: InvoiceInput[K]) => { setData(previous => ({ ...previous, [key]: value })); setDirty(true); };
  const item = <K extends keyof InvoiceItem>(index: number, key: K, value: InvoiceItem[K]) => change('items', data.items.map((row, i) => i === index ? { ...row, [key]: value } : row));
  return <><Heading title={saved ? saved.invoice_number ?? `Borrador de factura · ${saved.id}` : 'Nueva factura'} description={`Estado: ${saved?.status ?? 'Borrador'} · Moneda: HNL`} action={<div className="actions">{edit && <button onClick={edit}>Editar</button>}<button disabled={busy} onClick={() => void navigate(close)}>Volver</button></div>} />
    <h2 className="invoice-focus-heading" tabIndex={-1} ref={heading}>Editor de borrador de factura</h2><p className="quote-stage">{stage}</p>{saved?.source_quote && <p className="quote-note">Origen: {saved.source_quote.number ?? ("Cotización " + saved.source_quote.id)} · revisión {saved.source_quote.version}</p>}<Notice>{error}</Notice>{saved?.source_order&&<p className="quote-note">Origen: Orden {saved.source_order.number} · revisión {saved.source_order.version}</p>}
    {conflict && <div role="status"><p>Tus cambios se conservan en pantalla. Abre la versión actual para revisarlos antes de guardar.</p><button disabled={busy} onClick={() => void navigate(() => {
      setBusy(true); setError('');
      void api.request<InvoiceDraft>(`/api/v1/facturas/${saved!.id}`).then(latest => { setSaved(latest); setData(latest.data); setDirty(false); setConflict(false); attempt.current = null; }).catch(reason => setError(message(reason))).finally(() => setBusy(false));
    })}>Abrir versión actual</button></div>}
    <form className="stack quote-editor invoice-editor" onSubmit={async e => {
      e.preventDefault(); if (busy || conflict || !editable || cashPending) return;
      setBusy(true); setError('');
      try {
        let updated: InvoiceDraft;
        if (saved) updated = await api.request<InvoiceDraft>(`/api/v1/facturas/${saved.id}`, { method: 'PUT', body: data, ifMatch: saved.version });
        else {
          const body = JSON.stringify(data);
          if (!attempt.current || attempt.current.body !== body) attempt.current = { body, key: crypto.randomUUID() };
          updated = await api.request<InvoiceDraft>('/api/v1/facturas', { method: 'POST', body: data, idempotencyKey: attempt.current.key });
        }
        setSaved(updated); setData(updated.data); setDirty(false); attempt.current = null; notify('Borrador de factura guardado.');
      } catch (reason) { setError(message(reason)); if (reason instanceof ApiError && reason.status === 412) setConflict(true); }
      finally { setBusy(false); }
    }}><fieldset className="form-fields quote-form" disabled={busy || conflict || !editable}><legend className="quote-sr-only">Datos de factura</legend>
      <section className="quote-section" aria-labelledby="invoice-general-title"><div className="quote-section-heading"><h2 id="invoice-general-title">Generales</h2><span>HNL · {saved?.status ?? 'Borrador'}</span></div>
        <div className="quote-general-fields invoice-general-fields">
          <QuoteClientPicker value={data.client_name} selected={data.id_client ? { id: data.id_client, cliente_name: data.client_name, rtn: data.client_rtn ?? '', email: data.client_email ?? '' } : null}
            canSearch={moduleCan(access, 'clientes', 'catalogo', 'ver')} readOnly={!editable}
            change={(name, client) => { setData(previous => ({ ...previous, id_client: client?.id ?? null, client_name: name, client_rtn: client?.rtn ?? '', client_email: client?.email ?? '' })); setDirty(true); }} />
          <label className="quote-order-reference">Referencia de orden<input maxLength={100} value={data.order_number} onChange={e => change('order_number', e.target.value)} /></label>
          <label>Fecha<input type="date" min="1000-01-01" max="9999-12-31" required value={data.invoice_date} onChange={e => change('invoice_date', e.target.value)} /></label>
          <label>Términos<select name="payment_term_type" value={data.payment_term_type} onChange={e => { const value = e.target.value as InvoiceInput['payment_term_type']; setData(old => ({ ...old, payment_term_type: value, credit_days: value === 'contado' ? 0 : old.credit_days })); setDirty(true); }}><option value="contado">Contado</option><option value="credito">Crédito</option></select></label>
          {data.payment_term_type === 'credito' && <label>Días de crédito<input type="number" required min={0} max={3650} value={data.credit_days} onChange={e => change('credit_days', Number(e.target.value))} /></label>}
          <label>Orden de Compra / Exenta No.<input maxLength={150} value={data.purchase_order_exempt_number} onChange={e => change('purchase_order_exempt_number', e.target.value)} /></label>
          <label>Constancia de Registro No.<input maxLength={150} value={data.registration_certificate_number} onChange={e => change('registration_certificate_number', e.target.value)} /></label>
          <label>Registro SAG No.<input maxLength={150} value={data.sag_registration_number} onChange={e => change('sag_registration_number', e.target.value)} /></label>
        </div>
        {data.payment_term_type === 'contado' && <p className="quote-note" role="status">Contado exige pago total antes de guardar. Registra el pago inicial debajo de los datos de factura.</p>}
      </section>
      {editable && !(saved?.payments?.length) && <section className="quote-section"><h2>Pago inicial</h2><label><input type="checkbox" checked={Boolean(data.initial_payment)} onChange={event=>change('initial_payment',event.target.checked?newPayment():null)} /> Registrar pago al guardar</label>{data.initial_payment && <InvoicePaymentFields value={data.initial_payment} change={value=>change('initial_payment',value)} />}</section>}
      <InvoiceItems items={data.items} calculation={calculation} change={item} remove={index => change('items', data.items.filter((_, i) => i !== index))} add={() => change('items', [...data.items, newInvoiceItem()])} />
      <section className="quote-section" aria-labelledby="invoice-expenses-title"><div className="quote-section-heading"><h2 id="invoice-expenses-title">Gastos</h2><span>{data.expenses.length} / 200</span></div>
        <div className="invoice-expenses">{data.expenses.map((expense, index) => <fieldset className="invoice-expense" key={index}><legend>Gasto {index + 1}</legend><label>Descripción del gasto<input aria-label={`Descripción del gasto ${index + 1}`} maxLength={1000} value={expense.description} onChange={e => change('expenses', data.expenses.map((row, i) => i === index ? { ...row, description: e.target.value } : row))} /></label><Decimal label={`Monto del gasto ${index + 1} (HNL)`} caption="Monto (HNL)" value={expense.amount} change={v => change('expenses', data.expenses.map((row, i) => i === index ? { ...row, amount: v } : row))} /><button type="button" className="quote-remove" aria-label={`Quitar gasto ${index + 1}`} onClick={() => change('expenses', data.expenses.filter((_, i) => i !== index))}>Quitar gasto</button></fieldset>)}</div>
        <button type="button" className="quote-add-item" disabled={data.expenses.length >= 200} onClick={() => change('expenses', [...data.expenses, { description: 'Gasto', amount: '0' }])}><span aria-hidden="true">＋</span> Agregar gasto</button>
      </section>
      <div className="quote-editor-bottom">
        <section className="quote-section" aria-labelledby="invoice-notes-title"><div className="quote-section-heading"><h2 id="invoice-notes-title">Notas</h2></div><div className="stack"><label>Notas privadas<textarea rows={3} maxLength={3000} value={data.notes} onChange={e => change('notes', e.target.value)} /></label><label>Notas públicas<textarea rows={3} maxLength={3000} value={data.public_notes} onChange={e => change('public_notes', e.target.value)} /></label></div></section>
        <div className="quote-summary"><section className="quote-section" aria-labelledby="invoice-discount-title"><div className="quote-section-heading"><h2 id="invoice-discount-title">Descuento general</h2></div><div className="quote-discount-fields"><label>Descuento<select value={data.discount_type} onChange={e => change('discount_type', e.target.value as DiscountType)}><option value="amount">Valor (HNL)</option><option value="percent">Porcentaje</option></select></label><Decimal label="Valor descuento" value={data.discount_value} percent={data.discount_type === 'percent'} change={v => change('discount_value', v)} /></div></section>
          <Notice>{calculationError}</Notice>{calculation ? <InvoiceTotals calculation={calculation} /> : !calculationError && <p role="status">Calculando totales…</p>}
        </div>
      </div>
      {editable && <div className="quote-save"><button className="primary" type="submit" disabled={cashPending}>{busy ? 'Guardando…' : 'Guardar borrador'}</button></div>}
    </fieldset></form>
    {!!saved?.payments?.length && <section className="card"><h2>Pagos registrados</h2><Table columns={['Fecha','Monto (HNL)','Método','Referencia']} rows={saved.payments.map(payment=>[payment.date,payment.amount,payment.method || '—',payment.reference || '—'])} /></section>}
  </>;
}

export function InvoiceTotals({ calculation: c }: { calculation: InvoiceCalculation }) {
  const t = c.totals;
  const entries = [['Subtotal', t.subtotal], ['Exonerado', t.importe_exonerado], ['Exento', t.importe_exento], ['Gravado 15%', t.importe_gravado_15], ['Gravado 18%', t.importe_gravado_18], ['Descuentos y rebajas', t.descuentos_rebajas], ['ISV 15%', t.isv_15], ['ISV 18%', t.isv_18], ['Total de venta', t.total], ['Inversión en productos', c.product_investment], ['Otros gastos', c.other_expenses], ['Inversión total', c.investment_total], ['Utilidad estimada', c.profit], ['Pagado', c.paid_total], ['Saldo', c.balance]];
  return <section className="quote-totals invoice-totals" aria-label="Totales de factura"><h2>Resumen</h2><p className="quote-note">HNL · Cálculo del servidor</p><dl>{entries.map(([label, value]) => <div key={label}><dt>{label}</dt><dd>L {value}</dd></div>)}</dl></section>;
}
