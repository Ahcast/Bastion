import { useState } from 'react';
import type { CatalogClient, ClientPage } from '../clients.js';
import { Loading, Notice, useCore, useLoad } from '../ui.js';

export function QuoteClientPicker({ selected, change }: { selected: CatalogClient | null; change: (client: CatalogClient | null) => void }) {
  const { api } = useCore();
  const [search, setSearch] = useState(''), [query, setQuery] = useState(''), [page, setPage] = useState(1);
  const result = useLoad(signal => api.request<ClientPage<CatalogClient>>(`/api/v1/clientes/catalogo?search=${encodeURIComponent(query)}&page=${page}&page_size=25`, { signal }), [query, page]);
  const choices = result.data?.items ?? [];
  const options = selected && !choices.some(client => client.id === selected.id) ? [selected, ...choices] : choices;
  return <div className="stack quote-client">
    <div className="row"><label>Buscar cliente por nombre, RTN o correo<input maxLength={200} value={search} onChange={event => setSearch(event.target.value)} onKeyDown={event => {
      if (event.key === 'Enter') { event.preventDefault(); setQuery(search); setPage(1); result.reload(); }
    }} /></label><button type="button" onClick={() => { setQuery(search); setPage(1); result.reload(); }}>Buscar cliente</button></div>
    <Notice>{result.error}</Notice>{result.error && <button type="button" onClick={result.reload}>Reintentar clientes</button>}
    {!result.data && !result.error && <Loading />}
    <label>Cliente del catálogo<select value={selected?.id ?? ''} disabled={!result.data} onChange={event => change(options.find(client => client.id === Number(event.target.value)) ?? null)}>
      <option value="">Ingresar cliente manualmente</option>
      {options.map(client => <option key={client.id} value={client.id}>{[client.cliente_name, client.rtn && `RTN ${client.rtn}`, client.email, `#${client.id}`].filter(Boolean).join(' · ')}</option>)}
    </select></label>
    {result.data && <><div className="row"><button type="button" disabled={page === 1} onClick={() => setPage(value => value - 1)}>Clientes anteriores</button><span>Página de clientes {page}</span><button type="button" disabled={!result.data.has_more} onClick={() => setPage(value => value + 1)}>Más clientes</button></div>{choices.length === 0 && <p role="status">No se encontraron clientes. Puedes crearlos en Clientes o ingresar un nombre manual.</p>}</>}
    {selected && <button type="button" onClick={() => change(null)}>Usar cliente manual</button>}
  </div>;
}
