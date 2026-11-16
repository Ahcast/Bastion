import type { DocumentFilters } from '../documentFilters.js';

export function DocumentListFilters({ value, change, apply, clear, busy, kind }: {
  value: DocumentFilters; change: (value: DocumentFilters) => void; apply: () => void; clear: () => void; busy: boolean; kind: 'orders' | 'invoices';
}) {
  const orders = kind === 'orders';
  const field = (name: keyof DocumentFilters, next: string) => change({ ...value, [name]: next });
  return <form className="order-filters document-list-filters" onSubmit={event => { event.preventDefault(); apply(); }}>
    <label>{orders ? 'Buscar orden, factura, cliente o descripción de producto' : 'Buscar factura, cliente, referencia o descripción de ítem'}<input maxLength={200} value={value.search} onChange={event => field('search', event.target.value)} /></label>
    <label>Cliente<input maxLength={200} placeholder="Nombre del cliente" value={value.client} onChange={event => field('client', event.target.value)} /></label>
    <label>Fecha desde<input type="date" min="1000-01-01" max={value.date_to || '9999-12-31'} value={value.date_from} onChange={event => field('date_from', event.target.value)} /></label>
    <label>Fecha hasta<input type="date" min={value.date_from || '1000-01-01'} max="9999-12-31" value={value.date_to} onChange={event => field('date_to', event.target.value)} /></label>
    <label>Estado<select value={value.status} onChange={event => field('status', event.target.value)}><option value="">Todos</option>{(orders ? ['Pendiente', 'Listo', 'Entregado'] : ['Borrador', 'Enviado', 'Pagado']).map(status => <option key={status}>{status}</option>)}</select></label>
    {orders && <><label>Valor mínimo (HNL)<input inputMode="decimal" pattern="[0-9]{1,12}([.][0-9]{1,4})?" maxLength={17} value={value.min_total} onChange={event => field('min_total', event.target.value)} /></label><label>Valor máximo (HNL)<input inputMode="decimal" pattern="[0-9]{1,12}([.][0-9]{1,4})?" maxLength={17} value={value.max_total} onChange={event => field('max_total', event.target.value)} /></label></>}
    <div className="actions"><button disabled={busy}>Aplicar filtros</button><button type="button" disabled={busy} onClick={clear}>Limpiar filtros</button></div>
  </form>;
}
