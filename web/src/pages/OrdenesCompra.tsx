import { useEffect, useRef, useState } from 'react';
import { ApiError, message } from '../api.js';
import { Form, Heading, Loading, Notice, Table, useCore, useLoad } from '../ui.js';
import { QuoteActions } from '../components/QuoteActions.js';
import { useNavigationGuard, useUnsavedChanges } from '../navigationGuard.js';
import { moduleCan } from '../types.js';
import { newOrder, newProduct } from '../orders.js';
import type { Destination, OrderCalculation, OrderInput, OrderPage, OrderProduct, PurchaseOrder } from '../orders.js';

const base = '/api/v1/ordenes-compra';
const stage = 'Primera etapa: cliente manual, productos, gastos y destinos. Adjuntos, proveedores, clonación, eliminación y conversión a factura están pendientes.';
export default function OrdenesCompra() {
  const { access } = useCore();
  return moduleCan(access, 'ordenes_compra', 'documentos', 'ver') ? <Workspace /> : <Notice>No tienes acceso a Órdenes de compra.</Notice>;
}
function Workspace() {
  const { api, access } = useCore();
  const editable = moduleCan(access, 'ordenes_compra', 'documentos', 'editar');
  const [selected, setSelected] = useState<PurchaseOrder | 'new' | null>(null), [mode, setMode] = useState<'view' | 'edit'>('view');
  const [search, setSearch] = useState(''), [status, setStatus] = useState(''), [min, setMin] = useState(''), [max, setMax] = useState('');
  const [query, setQuery] = useState(''), [page, setPage] = useState(1), [error, setError] = useState(''), [busy, setBusy] = useState(false);
  const result = useLoad(signal => api.request<OrderPage>(`${base}?${query}&page=${page}&page_size=25`, { signal }), [query, page]);
  const open = async (id: number, next: 'view' | 'edit') => { setBusy(true); setError(''); try { setSelected(await api.request<PurchaseOrder>(`${base}/${id}`)); setMode(next); } catch (reason) { setError(message(reason)); } finally { setBusy(false); } };
  if (selected) return <OrderEditor key={selected === 'new' ? 'new' : selected.id} initial={selected === 'new' ? null : selected} editable={editable && mode === 'edit'} edit={editable && mode === 'view' ? () => setMode('edit') : undefined} close={() => { setSelected(null); result.reload(); }} />;
  return <><Heading title="Órdenes de compra" description="Sigue productos, venta, inversión y entrega de tus órdenes." action={editable && <button className="primary" disabled={busy} onClick={() => { setSelected('new'); setMode('edit'); }}>Nueva orden</button>} /><p className="quote-stage">{stage}</p>
    <form className="card order-filters" onSubmit={e => { e.preventDefault(); const params = new URLSearchParams(); if (search.trim()) params.set('search', search.trim()); if (status) params.set('status', status); if (min) params.set('min_total', min); if (max) params.set('max_total', max); setQuery(params.toString()); setPage(1); }}>
      <label>Buscar orden, factura o cliente<input maxLength={200} value={search} onChange={e => setSearch(e.target.value)} /></label>
      <label>Estado<select value={status} onChange={e => setStatus(e.target.value)}><option value="">Todos</option>{['Pendiente', 'Listo', 'Entregado'].map(value => <option key={value}>{value}</option>)}</select></label>
      <label>Valor mínimo (HNL)<input inputMode="decimal" pattern="[0-9]{1,12}([.][0-9]{1,4})?" maxLength={17} value={min} onChange={e => setMin(e.target.value)} /></label>
      <label>Valor máximo (HNL)<input inputMode="decimal" pattern="[0-9]{1,12}([.][0-9]{1,4})?" maxLength={17} value={max} onChange={e => setMax(e.target.value)} /></label>
      <div className="actions"><button disabled={busy}>Aplicar filtros</button><button type="button" disabled={busy} onClick={() => { setSearch(''); setStatus(''); setMin(''); setMax(''); setQuery(''); setPage(1); }}>Limpiar filtros</button></div>
    </form><Notice>{error || result.error}</Notice>{result.error && <button onClick={result.reload}>Reintentar consulta</button>}
    {result.data ? <><Table columns={['Orden', 'Factura', 'Cliente', 'Fecha', 'Total (HNL)', 'Estado', 'Acciones']} rows={result.data.items.map(row => [row.n_orders, row.n_bill || '—', row.client_name, row.order_date, row.total, row.status, <QuoteActions number={row.n_orders} busy={busy} actions={[
      { label: 'Ver', run: () => void open(row.id, 'view') }, { label: 'Editar', disabled: !editable, reason: !editable ? 'Sin permiso de edición' : undefined, run: () => void open(row.id, 'edit') },
      { label: 'Documentos', disabled: true, reason: 'Adjuntos privados pendientes' }, { label: 'Convertir a factura', disabled: true, reason: 'Conversión entre módulos pendiente' },
      { label: 'Clonar', disabled: true, reason: 'Clonación pendiente' }, { label: 'Eliminar', disabled: true, danger: true, reason: moduleCan(access, 'ordenes_compra', 'documentos', 'eliminar') ? 'Borrado y documentos pendientes' : 'Sin permiso de eliminación' },
    ]} />])} /><nav className="row" aria-label="Paginación de órdenes"><button disabled={busy || page === 1} onClick={() => setPage(p => p - 1)}>Anterior</button><span role="status">Página {page}</span><button disabled={busy || !result.data.has_more} onClick={() => setPage(p => p + 1)}>Siguiente</button></nav></> : !result.error && <Loading />}
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
    <h2 className="order-focus-heading" tabIndex={-1} ref={heading}>Editor de orden de compra</h2><p className="quote-stage">{stage}</p><Notice>{error}</Notice>
    {ambiguous && <p role="status">No se pudo confirmar la creación. Reintenta guardar los mismos datos para recuperar el resultado sin duplicar la orden.</p>}
    {conflict && <div role="status"><p>Tus cambios se conservan en pantalla. Abre la versión actual para revisarlos antes de guardar.</p><button disabled={busy} onClick={() => void navigate(() => { setBusy(true); setError(''); void api.request<PurchaseOrder>(`${base}/${saved!.id}`).then(latest => { setSaved(latest); setData(latest.data); setDirty(false); setConflict(false); }).catch(reason => setError(message(reason))).finally(() => setBusy(false)); })}>Abrir versión actual</button></div>}
    <Notice>{destinations.error}</Notice>{destinations.error && <button onClick={destinations.reload}>Reintentar destinos</button>}
    {destinations.data?.length === 0 && <p role="status">Agrega un destino en Ajustes &gt; Órdenes de compra antes de guardar. Si no tienes permiso, solicítalo al administrador.</p>}
    <form className="stack order-editor" onSubmit={async e => {
      e.preventDefault(); if (busy || conflict || !editable || !destinations.data?.length) return;
      setBusy(true); setError('');
      try {
        let updated: PurchaseOrder;
        if (saved) updated = await api.request<PurchaseOrder>(`${base}/${saved.id}`, { method: 'PUT', body: data, ifMatch: saved.version });
        else { const body = JSON.stringify(data); if (!attempt.current) attempt.current = { body, key: crypto.randomUUID() }; else if (attempt.current.body !== body) throw new Error('Reintenta con los datos originales.'); updated = await api.request<PurchaseOrder>(base, { method: 'POST', body: data, idempotencyKey: attempt.current.key }); }
        setSaved(updated); setData(updated.data); setDirty(false); setAmbiguous(false); attempt.current = null; notify('Orden de compra guardada.');
      } catch (reason) { setError(message(reason)); if (reason instanceof ApiError && reason.status === 412) setConflict(true); if (!saved) { if (reason instanceof ApiError && reason.status < 500 && reason.status !== 408) { attempt.current = null; setAmbiguous(false); } else setAmbiguous(true); } }
      finally { setBusy(false); }
    }}><fieldset className="form-fields quote-form" disabled={busy || conflict || ambiguous || !editable}><legend>Datos de orden</legend><div className="quote-fields">
      <label>Número de orden<input required maxLength={50} value={data.n_orders} onChange={e => change('n_orders', e.target.value)} /></label>
      <label>Cliente (manual)<input required maxLength={255} value={data.client_name} onChange={e => change('client_name', e.target.value)} /></label><label>RTN del cliente<input maxLength={30} value={data.client_rtn} onChange={e => change('client_rtn', e.target.value)} /></label>
      <label>Destino<select required value={data.id_destination ?? ''} onChange={e => change('id_destination', e.target.value ? Number(e.target.value) : null)}><option value="">Selecciona un destino</option>{destinations.data?.map(row => <option key={row.id} value={row.id}>{row.destination}</option>)}</select></label>
      <label>Fecha de orden<input required type="date" min="1000-01-01" max="9999-12-31" value={data.order_date} onChange={e => change('order_date', e.target.value)} /></label>
      <label>Fecha de entrega<input type="date" min="1000-01-01" max="9999-12-31" value={data.received_date ?? ''} onChange={e => change('received_date', e.target.value || null)} /></label>
      <label>Número de factura<input maxLength={120} value={data.n_bill} onChange={e => change('n_bill', e.target.value)} /><small>Una referencia de factura determina el estado Entregado. No emite una factura.</small></label>
    </div><div className="row"><h2>Productos</h2><button type="button" disabled={data.products.length >= 200} onClick={() => change('products', [...data.products, newProduct()])}>Agregar producto</button></div>
    {data.products.map((row, index) => <fieldset className="quote-item" key={index}><legend>Producto {index + 1}</legend><label>Descripción<textarea maxLength={2000} rows={2} value={row.description} onChange={e => product(index, 'description', e.target.value)} /></label><div className="quote-fields">
      <label>Cantidad<input required type="number" min={0} max={1000000} step={1} value={row.cantidad} onChange={e => product(index, 'cantidad', Number(e.target.value))} /></label>
      <Decimal label="Costo unitario (HNL)" value={row.sale_price} change={v => product(index, 'sale_price', v)} /><Decimal label="Precio de venta (HNL)" value={row.purchase_price} change={v => product(index, 'purchase_price', v)} />
      <label>Tracking<input maxLength={255} value={row.tracking} onChange={e => product(index, 'tracking', e.target.value)} /></label>
      <label>Estado del producto<select value={row.status} onChange={e => product(index, 'status', e.target.value as OrderProduct['status'])}><option>Pendiente</option><option>Comprado</option></select></label>
      <label className="order-tax"><input type="checkbox" checked={row.isv} onChange={e => product(index, 'isv', e.target.checked)} />ISV 15%</label><label>Comentario del producto<textarea rows={2} maxLength={1000} value={row.coments} onChange={e => product(index, 'coments', e.target.value)} /></label>
    </div><button type="button" className="danger" onClick={() => change('products', data.products.filter((_, i) => i !== index))}>Quitar producto</button></fieldset>)}
    <div className="row"><h2>Gastos adicionales</h2><button type="button" disabled={data.expenses.length >= 200} onClick={() => change('expenses', [...data.expenses, { description: 'Gasto', amount: '0' }])}>Agregar gasto</button></div>
    {data.expenses.map((expense, index) => <fieldset className="quote-item" key={index}><legend>Gasto {index + 1}</legend><label>Descripción del gasto<input maxLength={255} value={expense.description} onChange={e => change('expenses', data.expenses.map((row, i) => i === index ? { ...row, description: e.target.value } : row))} /></label><Decimal label="Monto del gasto (HNL)" value={expense.amount} change={v => change('expenses', data.expenses.map((row, i) => i === index ? { ...row, amount: v } : row))} /><button type="button" className="danger" onClick={() => change('expenses', data.expenses.filter((_, i) => i !== index))}>Quitar gasto</button></fieldset>)}
    <label>Comentarios de la orden<textarea rows={3} maxLength={3000} value={data.comments} onChange={e => change('comments', e.target.value)} /></label>
    </fieldset>{editable && <button className="primary" type="submit" disabled={busy || conflict || !destinations.data?.length}>{busy ? 'Guardando…' : ambiguous ? 'Reintentar guardar' : 'Guardar orden'}</button>}</form>
    <Notice>{calculationError}</Notice>{calculation ? <OrderSummaryTotals calculation={calculation} /> : !calculationError && <p role="status">Calculando totales…</p>}
  </>;
}
export function OrderSummaryTotals({ calculation: c }: { calculation: OrderCalculation }) {
  const t = c.totals, entries = [['Subtotal de venta', t.subtotal], ['Subtotal exonerado', t.subtotal_exonerado], ['ISV 15%', t.tax], ['Total de venta', t.total], ['Inversión en productos', t.products_investment], ['Otros gastos', t.other_expenses], ['Inversión total', t.investment_total], ['Utilidad estimada', t.profit]];
  return <section className="card quote-totals order-totals" aria-label="Totales de orden"><h2>Resumen</h2><p>HNL · {c.status}</p><dl>{entries.map(([label, value]) => <div key={label}><dt>{label}</dt><dd>L {value}</dd></div>)}</dl></section>;
}
function Decimal({ label, value, change }: { label: string; value: string; change: (value: string) => void }) { return <label>{label}<input required type="text" inputMode="decimal" maxLength={17} pattern="[0-9]{1,12}([.][0-9]{1,4})?" title="Número sin signo; hasta 4 decimales con punto." value={value} onChange={e => change(e.target.value)} /></label>; }

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
