import { useEffect, useRef, useState } from 'react';
import { ApiError, message } from '../api.js';
import { Form, Heading, Loading, Notice, Table, useCore, useLoad } from '../ui.js';
import { OrderDocuments } from '../components/OrderDocuments.js';
import { QuoteActions } from '../components/QuoteActions.js';
import { Decimal } from '../components/QuoteItems.js';
import { useNavigationGuard, useUnsavedChanges } from '../navigationGuard.js';
import { moduleCan } from '../types.js';
import { newOrder, newProduct } from '../orders.js';
import type { Destination, OrderCalculation, OrderInput, OrderPage, OrderProduct, PurchaseOrder } from '../orders.js';

const base = '/api/v1/ordenes-compra';
const stage = 'Primera etapa: cliente manual, productos, gastos y destinos. Documentos privados por producto, clonación y conversión a borrador de factura disponibles. Proveedores y eliminación de órdenes pendientes.';
export default function OrdenesCompra({ initialId,openInvoice }: { initialId?: number;openInvoice?:(id:number)=>void } = {}) {
  const { access } = useCore();
  return moduleCan(access, 'ordenes_compra', 'documentos', 'ver') ? <Workspace initialId={initialId} openInvoice={openInvoice} /> : <Notice>No tienes acceso a Órdenes de compra.</Notice>;
}
function Workspace({ initialId,openInvoice }: { initialId?: number;openInvoice?:(id:number)=>void }) {
  const { api, access } = useCore();
  const editable = moduleCan(access, 'ordenes_compra', 'documentos', 'editar');
  const invoiceEditable=moduleCan(access,'facturas','documentos','editar')&&moduleCan(access,'facturas','documentos','ver');
  const [documentsId,setDocumentsId]=useState<number|null>(null);
  const attempts=useRef(new Map<string,{key:string;version:number}>()),running=useRef(false);
  const [uncertain,setUncertain]=useState(false);
  const action=async(id:number,version:number,kind:'clone'|'convert/invoice')=>{
    if(running.current)return;running.current=true;setBusy(true);setError('');
    const label=kind+':'+id;
    let attempt=attempts.current.get(label);
    if(!attempt){attempt={key:crypto.randomUUID(),version};attempts.current.set(label,attempt);}
    try{
      if(kind==='clone'){
        const clone=await api.request<PurchaseOrder>(base+'/'+id+'/clone',{method:'POST',ifMatch:attempt.version,idempotencyKey:attempt.key});
        setSelected(clone);setMode('edit');
      }else{
        const result=await api.request<{target_id:number}>(base+'/'+id+'/convert/invoice',{method:'POST',ifMatch:attempt.version,idempotencyKey:attempt.key});
        openInvoice?.(result.target_id);
      }
      attempts.current.delete(label);setUncertain(false);result.reload();
    }catch(reason){
      setError(kind==='convert/invoice'&&reason instanceof ApiError&&reason.status===400?'No se pudo convertir. Revisa que la orden tenga productos, ventas y costos con hasta dos decimales, y totales compatibles con Facturas.':message(reason));
      if(reason instanceof ApiError&&reason.status<500&&reason.status!==408){attempts.current.delete(label);setUncertain(false);if(reason.status===412)result.reload();}
      else setUncertain(true);
    }finally{running.current=false;setBusy(false);}
  };
  const [selected, setSelected] = useState<PurchaseOrder | 'new' | null>(null), [mode, setMode] = useState<'view' | 'edit'>('view');
  const [search, setSearch] = useState(''), [status, setStatus] = useState(''), [min, setMin] = useState(''), [max, setMax] = useState('');
  const [query, setQuery] = useState(''), [page, setPage] = useState(1), [error, setError] = useState(''), [busy, setBusy] = useState(false);
  useUnsavedChanges(uncertain,busy);
  const result = useLoad(signal => api.request<OrderPage>(`${base}?${query}&page=${page}&page_size=25`, { signal }), [query, page]);
  const open = async (id: number, next: 'view' | 'edit') => { setBusy(true); setError(''); try { setSelected(await api.request<PurchaseOrder>(`${base}/${id}`)); setMode(next); } catch (reason) { setError(message(reason)); } finally { setBusy(false); } };
  useEffect(() => { if (initialId) void open(initialId, 'edit'); }, [initialId, api]);
  if(documentsId!==null)return <OrderDocuments id={documentsId} close={()=>{setDocumentsId(null);result.reload();}}/>;
  if (selected) return <OrderEditor key={selected === 'new' ? 'new' : selected.id} initial={selected === 'new' ? null : selected} editable={editable && mode === 'edit'} edit={editable && mode === 'view' ? () => setMode('edit') : undefined} close={() => { setSelected(null); result.reload(); }} />;
  return <><Heading title="Órdenes de compra" description="Sigue productos, venta, inversión y entrega de tus órdenes." action={editable && <button className="primary" disabled={busy} onClick={() => { setSelected('new'); setMode('edit'); }}>Nueva orden</button>} /><p className="quote-stage">{stage}</p>
    <section className="quote-list order-list" aria-labelledby="order-list-title"><div className="quote-list-heading"><h2 id="order-list-title">Listado de órdenes</h2>{result.data && <span>{result.data.items.length} en esta página</span>}</div>
    <form className="order-filters" onSubmit={e => { e.preventDefault(); const params = new URLSearchParams(); if (search.trim()) params.set('search', search.trim()); if (status) params.set('status', status); if (min) params.set('min_total', min); if (max) params.set('max_total', max); setQuery(params.toString()); setPage(1); }}>
      <label>Buscar orden, factura o cliente<input maxLength={200} value={search} onChange={e => setSearch(e.target.value)} /></label>
      <label>Estado<select value={status} onChange={e => setStatus(e.target.value)}><option value="">Todos</option>{['Pendiente', 'Listo', 'Entregado'].map(value => <option key={value}>{value}</option>)}</select></label>
      <label>Valor mínimo (HNL)<input inputMode="decimal" pattern="[0-9]{1,12}([.][0-9]{1,4})?" maxLength={17} value={min} onChange={e => setMin(e.target.value)} /></label>
      <label>Valor máximo (HNL)<input inputMode="decimal" pattern="[0-9]{1,12}([.][0-9]{1,4})?" maxLength={17} value={max} onChange={e => setMax(e.target.value)} /></label>
      <div className="actions"><button disabled={busy}>Aplicar filtros</button><button type="button" disabled={busy} onClick={() => { setSearch(''); setStatus(''); setMin(''); setMax(''); setQuery(''); setPage(1); }}>Limpiar filtros</button></div>
    </form><Notice>{error || result.error}</Notice>{uncertain&&<p role="status">No se pudo confirmar la operación. Repite la misma acción para recuperar su resultado sin duplicados.</p>}{initialId && error && <button disabled={busy} onClick={() => void open(initialId, 'edit')}>Reintentar abrir documento convertido</button>}{result.error && <button onClick={result.reload}>Reintentar consulta</button>}
    {result.data ? <><Table columns={['Orden', 'Factura', 'Cliente', 'Fecha', 'Total (HNL)', 'Estado', 'Acciones']} rows={result.data.items.map(row => [<span className="quote-number">{row.n_orders}</span>, <span className="quote-number">{row.n_bill || '—'}</span>, <div className="quote-list-client">{row.client_name || 'Sin cliente'}</div>, row.order_date, <span className="quote-money">{row.total}</span>, <span className="quote-status" data-state={row.status}>{row.status}</span>, <QuoteActions number={row.n_orders} busy={busy} actions={[
      { label: 'Ver', run: () => void open(row.id, 'view') }, { label: 'Editar', disabled: !editable, reason: !editable ? 'Sin permiso de edición' : undefined, run: () => void open(row.id, 'edit') },
      { label: 'Documentos', run:()=>setDocumentsId(row.id) }, { label: 'Convertir a factura', disabled:!editable||!invoiceEditable, reason:!editable||!invoiceEditable?'Requiere ver y editar en Órdenes y Facturas':undefined, run:()=>{if(window.confirm('¿Crear o abrir el borrador de factura vinculado a esta orden?'))void action(row.id,row.version,'convert/invoice');} },
      { label: 'Clonar', disabled:!editable,reason:!editable?'Sin permiso de edición':undefined,run:()=>void action(row.id,row.version,'clone') }, { label: 'Eliminar', disabled: true, danger: true, reason: moduleCan(access, 'ordenes_compra', 'documentos', 'eliminar') ? 'Eliminación de órdenes pendiente' : 'Sin permiso de eliminación' },
    ]} />])} /><nav className="quote-pagination" aria-label="Paginación de órdenes"><button disabled={busy || page === 1} onClick={() => setPage(p => p - 1)}>Anterior</button><span role="status">Página {page}</span><button disabled={busy || !result.data.has_more} onClick={() => setPage(p => p + 1)}>Siguiente</button></nav></> : !result.error && <Loading />}
    </section>
  </>;
}

export function OrderEditor({ initial, editable, edit, close }: { initial: PurchaseOrder | null; editable: boolean; edit?: () => void; close: () => void }) {
  const { api, notify } = useCore(), { navigate } = useNavigationGuard();
  const [saved, setSaved] = useState(initial), [data, setData] = useState<OrderInput>(() => initial?.data ?? newOrder());
  const [dirty, setDirty] = useState(false), [busy, setBusy] = useState(false), [conflict, setConflict] = useState(false), [ambiguous, setAmbiguous] = useState(false), [error, setError] = useState('');
  const [preview, setPreview] = useState<{ input: OrderInput; value: OrderCalculation } | null>(null), [calculationError, setCalculationError] = useState('');
  const destinations = useLoad(signal => api.request<Destination[]>(`${base}/destinations`, { signal }));
  const attempt = useRef<{ body: string; key: string } | null>(null), heading = useRef<HTMLHeadingElement>(null);
  useUnsavedChanges(dirty || ambiguous, busy);
  useEffect(() => { heading.current?.focus(); }, []);
  useEffect(() => {
    if (!editable || (saved && !dirty) || conflict || ambiguous) return;
    const controller = new AbortController(), input = data; setCalculationError('');
    const timer = setTimeout(() => { void api.request<OrderCalculation>(`${base}/calculate`, { method: 'POST', body: input, signal: controller.signal }).then(value => { if (!controller.signal.aborted) setPreview({ input, value }); }).catch(reason => { if (!controller.signal.aborted) setCalculationError(message(reason)); }); }, 450);
    return () => { clearTimeout(timer); controller.abort(); };
  }, [api, data, editable, saved, dirty, conflict, ambiguous]);
  const calculation = saved && !dirty ? saved.calculation : preview?.input === data ? preview.value : null;
  const change = <K extends keyof OrderInput>(key: K, value: OrderInput[K]) => { setData(previous => ({ ...previous, [key]: value })); setDirty(true); };
  const product = <K extends keyof OrderProduct>(index: number, key: K, value: OrderProduct[K]) => change('products', data.products.map((row, i) => i === index ? { ...row, [key]: value } : row));
  return <><Heading title={saved ? `Orden · ${saved.data.n_orders}` : 'Nueva orden de compra'} description={`Estado: ${calculation?.status ?? saved?.status ?? 'Pendiente'} · Moneda: HNL`} action={<div className="actions">{edit && <button onClick={edit}>Editar</button>}<button disabled={busy} onClick={() => void navigate(close)}>Volver</button></div>} />
    <h2 className="order-focus-heading" tabIndex={-1} ref={heading}>Editor de orden de compra</h2><p className="quote-stage">{stage}</p>{saved?.source_quote && <p className="quote-note">Origen: {saved.source_quote.number ?? ("Cotización " + saved.source_quote.id)} · revisión {saved.source_quote.version}</p>}<Notice>{error}</Notice>
    {ambiguous && <p role="status">No se pudo confirmar la creación. Reintenta guardar los mismos datos para recuperar el resultado sin duplicar la orden.</p>}
    {conflict && <div role="status"><p>Tus cambios se conservan en pantalla. Abre la versión actual para revisarlos antes de guardar.</p><button disabled={busy} onClick={() => void navigate(() => { setBusy(true); setError(''); void api.request<PurchaseOrder>(`${base}/${saved!.id}`).then(latest => { setSaved(latest); setData(latest.data); setDirty(false); setConflict(false); }).catch(reason => setError(message(reason))).finally(() => setBusy(false)); })}>Abrir versión actual</button></div>}
    <Notice>{destinations.error}</Notice>{destinations.error && <button onClick={destinations.reload}>Reintentar destinos</button>}
    {destinations.data?.length === 0 && <p role="status">Agrega un destino en Ajustes &gt; Órdenes de compra antes de guardar. Si no tienes permiso, solicítalo al administrador.</p>}
    <form className="order-editor" onSubmit={async e => {
      e.preventDefault(); if (busy || conflict || !editable || !destinations.data?.length) return;
      setBusy(true); setError('');
      try {
        let updated: PurchaseOrder;
        if (saved) updated = await api.request<PurchaseOrder>(`${base}/${saved.id}`, { method: 'PUT', body: data, ifMatch: saved.version });
        else { const body = JSON.stringify(data); if (!attempt.current) attempt.current = { body, key: crypto.randomUUID() }; else if (attempt.current.body !== body) throw new Error('Reintenta con los datos originales.'); updated = await api.request<PurchaseOrder>(base, { method: 'POST', body: data, idempotencyKey: attempt.current.key }); }
        setSaved(updated); setData(updated.data); setDirty(false); setAmbiguous(false); attempt.current = null; notify('Orden de compra guardada.');
      } catch (reason) { setError(reason instanceof ApiError && reason.status === 409 ? 'Retira el archivo del producto antes de quitar su línea; tus cambios se conservan.' : message(reason)); if (reason instanceof ApiError && reason.status === 412) setConflict(true); if (!saved) { if (reason instanceof ApiError && reason.status < 500 && reason.status !== 408) { attempt.current = null; setAmbiguous(false); } else setAmbiguous(true); } }
      finally { setBusy(false); }
    }}><fieldset className="form-fields quote-form" disabled={busy || conflict || ambiguous || !editable}><legend className="quote-sr-only">Datos de orden</legend>
      <section className="quote-section" aria-labelledby="order-general-title"><div className="quote-section-heading"><h2 id="order-general-title">Generales</h2><span>HNL · {calculation?.status ?? saved?.status ?? 'Pendiente'}</span></div><div className="quote-general-fields order-general-fields">
        <label>Número de orden<input required maxLength={50} value={data.n_orders} onChange={e => change('n_orders', e.target.value)} /></label>
        <label>Cliente (manual)<input required maxLength={255} value={data.client_name} onChange={e => change('client_name', e.target.value)} /></label>
        <label>RTN del cliente<input maxLength={30} value={data.client_rtn} onChange={e => change('client_rtn', e.target.value)} /></label>
        <label>Destino<select required value={data.id_destination ?? ''} onChange={e => change('id_destination', e.target.value ? Number(e.target.value) : null)}><option value="">Selecciona un destino</option>{destinations.data?.map(row => <option key={row.id} value={row.id}>{row.destination}</option>)}</select></label>
        <label>Fecha de orden<input required type="date" min="1000-01-01" max="9999-12-31" value={data.order_date} onChange={e => change('order_date', e.target.value)} /></label>
        <label>Fecha de entrega<input type="date" min="1000-01-01" max="9999-12-31" value={data.received_date ?? ''} onChange={e => change('received_date', e.target.value || null)} /></label>
        <label className="order-reference">Número de factura<input maxLength={120} value={data.n_bill} onChange={e => change('n_bill', e.target.value)} /><small>Esta referencia determina el estado Entregado; no emite una factura.</small></label>
      </div></section>
      <OrderItems products={data.products} change={product} remove={index => change('products', data.products.filter((_, i) => i !== index))} add={() => change('products', [...data.products, newProduct()])} />
      <section className="quote-section" aria-labelledby="order-expenses-title"><div className="quote-section-heading"><h2 id="order-expenses-title">Gastos adicionales</h2><span>{data.expenses.length} / 200</span></div>
        <div className="order-expenses">{data.expenses.map((expense, index) => <fieldset className="order-expense" key={index}><legend>Gasto {index + 1}</legend><label>Descripción del gasto<input maxLength={255} value={expense.description} onChange={e => change('expenses', data.expenses.map((row, i) => i === index ? { ...row, description: e.target.value } : row))} /></label><Decimal label={`Monto del gasto ${index + 1} (HNL)`} caption="Monto (HNL)" value={expense.amount} change={v => change('expenses', data.expenses.map((row, i) => i === index ? { ...row, amount: v } : row))} /><button type="button" className="quote-remove" aria-label={`Quitar gasto ${index + 1}`} onClick={() => change('expenses', data.expenses.filter((_, i) => i !== index))}>Quitar gasto</button></fieldset>)}</div>
        <button type="button" className="quote-add-item order-expense-add" disabled={data.expenses.length >= 200} onClick={() => change('expenses', [...data.expenses, { description: 'Gasto', amount: '0' }])}>Agregar gasto</button>
      </section>
      <div className="quote-editor-bottom"><section className="quote-section" aria-labelledby="order-comments-title"><div className="quote-section-heading"><h2 id="order-comments-title">Comentarios</h2></div><label>Comentarios de la orden<textarea rows={3} maxLength={3000} value={data.comments} onChange={e => change('comments', e.target.value)} /></label></section>
        <div className="quote-summary"><Notice>{calculationError}</Notice>{calculation ? <OrderSummaryTotals calculation={calculation} /> : !calculationError && <p role="status">Calculando totales…</p>}</div>
      </div>
    </fieldset>{editable && <div className="quote-save"><button className="primary" type="submit" disabled={busy || conflict || !destinations.data?.length}>{busy ? 'Guardando…' : ambiguous ? 'Reintentar guardar' : 'Guardar orden'}</button></div>}</form>
  </>;
}
export function OrderSummaryTotals({ calculation: c }: { calculation: OrderCalculation }) {
  const t = c.totals, entries = [['Subtotal de venta', t.subtotal], ['Subtotal exonerado', t.subtotal_exonerado], ['ISV 15%', t.tax], ['Total de venta', t.total], ['Inversión en productos', t.products_investment], ['Otros gastos', t.other_expenses], ['Inversión total', t.investment_total], ['Utilidad estimada', t.profit]];
  return <section className="quote-totals order-totals" aria-label="Totales de orden"><h2>Resumen</h2><p className="quote-note">HNL · {c.status}</p><dl>{entries.map(([label, value]) => <div key={label}><dt>{label}</dt><dd>L {value}</dd></div>)}</dl></section>;
}
function OrderItems({ products, change, remove, add }: { products: OrderProduct[]; change: <K extends keyof OrderProduct>(index: number, key: K, value: OrderProduct[K]) => void; remove: (index: number) => void; add: () => void }) {
  return <section className="quote-section" aria-labelledby="order-products-title">
    <div className="quote-section-heading"><h2 id="order-products-title">Productos</h2><span>{products.length} / 200</span></div>
    <div className="quote-items-wrap"><table className="quote-items-table order-items-table" aria-label="Productos de la orden">
      <colgroup><col className="order-col-description" /><col className="order-col-quantity" /><col className="order-col-price" /><col className="order-col-price" /><col className="order-col-tax" /><col className="order-col-status" /><col className="order-col-tracking" /><col className="order-col-remove" /></colgroup>
      <thead><tr>{['Descripción y comentario', 'Cantidad', 'Venta unitaria', 'Costo unitario', 'ISV', 'Estado', 'Tracking'].map(label => <th scope="col" key={label}>{label}</th>)}<th scope="col"><span className="quote-sr-only">Quitar</span></th></tr></thead>
      <tbody>{products.map((row, index) => <tr key={index}>
        <td className="quote-item-description"><label><span>Descripción</span><textarea aria-label={`Descripción del producto ${index + 1}`} maxLength={2000} rows={2} value={row.description} onChange={e => change(index, 'description', e.target.value)} /></label><label><span>Comentario del producto</span><textarea aria-label={`Comentario del producto ${index + 1}`} maxLength={1000} rows={2} value={row.coments} onChange={e => change(index, 'coments', e.target.value)} /></label></td>
        <td><label><span>Cantidad</span><input aria-label={`Cantidad del producto ${index + 1}`} required type="number" min={0} max={1000000} step={1} value={row.cantidad} onChange={e => change(index, 'cantidad', Number(e.target.value))} /></label></td>
        <td><Decimal label={`Precio de venta (HNL) del producto ${index + 1}`} caption="Venta (HNL)" value={row.purchase_price} change={v => change(index, 'purchase_price', v)} /></td>
        <td><Decimal label={`Costo unitario (HNL) del producto ${index + 1}`} caption="Costo (HNL)" value={row.sale_price} change={v => change(index, 'sale_price', v)} /></td>
        <td><label className="order-tax"><span>ISV 15%</span><input aria-label={`ISV 15% del producto ${index + 1}`} type="checkbox" checked={row.isv} onChange={e => change(index, 'isv', e.target.checked)} /></label></td>
        <td><label><span>Estado</span><select aria-label={`Estado del producto ${index + 1}`} value={row.status} onChange={e => change(index, 'status', e.target.value as OrderProduct['status'])}><option>Pendiente</option><option>Comprado</option></select></label></td>
        <td><label><span>Tracking</span><input aria-label={`Tracking del producto ${index + 1}`} maxLength={255} value={row.tracking} onChange={e => change(index, 'tracking', e.target.value)} /></label></td>
        <td className="quote-item-remove"><button type="button" className="quote-remove" aria-label={`Quitar producto ${index + 1}`} title={`Quitar producto ${index + 1}`} onClick={() => remove(index)}><svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" aria-hidden="true"><path d="M3 6h18M9 6V3h6v3M6 6l1 15h10l1-15M10 10v7M14 10v7" /></svg></button></td>
      </tr>)}</tbody>
    </table>{products.length === 0 && <p className="empty">Agrega los productos de la orden.</p>}</div>
    <button type="button" className="quote-add-item" disabled={products.length >= 200} onClick={add}><span aria-hidden="true">＋</span> Agregar producto</button>
    <p className="quote-note">Los totales de venta e inversión se calculan en el servidor.</p>
  </section>;
}

export function OrderSettings() {
  const { access } = useCore();
  if (!moduleCan(access, 'ordenes_compra', 'documentos', 'ver') && !moduleCan(access, 'ordenes_compra', 'destinos', 'ver')) return <Notice>No tienes acceso a los destinos.</Notice>;
  return <DestinationCatalog />;
}
function DestinationCatalog() {
  const { api, access, notify } = useCore();
  const editable = moduleCan(access, 'ordenes_compra', 'destinos', 'editar');
  const result = useLoad(signal => api.request<Destination[]>(`${base}/destinations`, { signal }));
  const [selected, setSelected] = useState<Destination | null>(null), [revision, setRevision] = useState(0);
  return <section className="card"><h2>Órdenes de compra · Destinos</h2><p>Los destinos se administran aquí y se seleccionan al registrar una orden. La edición conserva su identificador.</p><p>Los demás ajustes del módulo requieren definición.</p>
    <Notice>{result.error}</Notice>{result.error && <button onClick={result.reload}>Reintentar destinos</button>}
    {editable && <><Form key={`${selected?.id ?? 'new'}-${revision}`} fields={[{ name: 'destination', label: 'Nombre del destino', required: true, maxLength: 255 }]} initial={selected ? { destination: selected.destination } : {}} submit={selected ? 'Guardar destino' : 'Agregar destino'} onSubmit={async fields => {
      await api.request<Destination>(`${base}/destinations${selected ? `/${selected.id}` : ''}`, { method: selected ? 'PUT' : 'POST', body: { destination: fields.destination }, ...(selected ? { ifMatch: selected.version } : {}) });
      notify('Destino guardado.'); setSelected(null); setRevision(v => v + 1); result.reload();
    }} />{selected && <div className="actions"><button onClick={() => setSelected(null)}>Cancelar edición</button><button onClick={() => { setSelected(null); result.reload(); }}>Recargar catálogo</button></div>}</>}
    {result.data ? <Table columns={['Destino', 'Acciones']} rows={result.data.map(row => [row.destination, editable ? <button onClick={() => setSelected(row)}>Editar destino</button> : 'Solo lectura'])} /> : !result.error && <Loading />}
  </section>;
}
