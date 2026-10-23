import { useEffect, useId, useRef, useState } from 'react';
import type { CatalogClient, ClientPage } from '../clients.js';
import { useCore, useLoad } from '../ui.js';

export function QuoteClientPicker({ value, selected, canSearch, readOnly, change }: {
  value: string; selected: CatalogClient | null; canSearch: boolean; readOnly: boolean;
  change: (name: string, client: CatalogClient | null) => void;
}) {
  const { api } = useCore();
  const [open, setOpen] = useState(false), [query, setQuery] = useState(''), [page, setPage] = useState(1), [active, setActive] = useState(-1);
  const root = useRef<HTMLDivElement>(null), input = useRef<HTMLInputElement>(null), id = useId();
  const term = selected ? '' : value.trim().slice(0, 200), pending = term !== query;
  useEffect(() => {
    if (!open || !canSearch || readOnly || !pending) return;
    const timer = setTimeout(() => { setQuery(term); setPage(1); setActive(-1); }, 200);
    return () => clearTimeout(timer);
  }, [term, open, canSearch, readOnly, pending]);
  const result = useLoad(signal => open && canSearch && !readOnly && !pending
    ? api.request<ClientPage<CatalogClient>>(`/api/v1/clientes/catalogo?search=${encodeURIComponent(query)}&page=${page}&page_size=25`, { signal })
    : Promise.resolve(null), [open, canSearch, readOnly, query, page, pending]);
  const choices = pending ? [] : result.data?.items ?? [];
  const close = () => { setOpen(false); setActive(-1); };
  const choose = (client: CatalogClient | null) => { change(client?.cliente_name ?? value, client); close(); };
  useEffect(() => {
    if (!open) return;
    const outside = (event: PointerEvent) => { if (!root.current?.contains(event.target as Node)) setOpen(false); };
    document.addEventListener('pointerdown', outside);
    return () => document.removeEventListener('pointerdown', outside);
  }, [open]);
  useEffect(() => { if (active >= 0) document.getElementById(`${id}-option-${active}`)?.scrollIntoView({ block: 'nearest' }); }, [active, id]);
  return <div className="quote-client" ref={root} onBlur={event => { if (!event.currentTarget.contains(event.relatedTarget as Node | null)) close(); }}>
    <label htmlFor={id}>Cliente</label>
    <div className="quote-client-control">
      <input id={id} ref={input} role="combobox" autoComplete="off" maxLength={255} value={value} readOnly={readOnly}
        aria-autocomplete="list" aria-expanded={open} aria-controls={open ? `${id}-list` : undefined} aria-activedescendant={open && active >= 0 ? `${id}-option-${active}` : undefined} aria-describedby={`${id}-hint`}
        placeholder="Buscar cliente o escribir un nombre…"
        onFocus={() => { if (!readOnly && canSearch) { setActive(-1); setOpen(true); } }}
        onChange={event => { change(event.target.value, null); setPage(1); setActive(-1); if (canSearch) setOpen(true); }}
        onKeyDown={event => {
          if (event.key === 'Escape') { event.preventDefault(); close(); }
          if (event.key === 'Tab') close();
          if (event.key === 'Enter' && !readOnly) { event.preventDefault(); if (open && active >= 0) choose(active > 0 ? choices[active - 1] ?? null : null); else close(); }
          if (!readOnly && canSearch && ['ArrowDown', 'ArrowUp'].includes(event.key)) {
            event.preventDefault(); setOpen(true);
            setActive(previous => event.key === 'ArrowDown' ? (previous + 1) % (choices.length + 1) : (previous < 1 ? choices.length : previous - 1));
          }
        }} />
      {!readOnly && canSearch && <button type="button" className="quote-client-toggle" aria-label={open ? 'Ocultar clientes' : 'Mostrar clientes'} aria-expanded={open} onMouseDown={event => event.preventDefault()} onClick={() => { if (open) close(); else { input.current?.focus(); setActive(-1); setOpen(true); } }}>⌄</button>}
    </div>
    <p id={`${id}-hint`} className="quote-client-hint">{selected ? [selected.rtn && `RTN: ${selected.rtn}`, selected.email || 'Sin correo registrado'].filter(Boolean).join(' · ') : canSearch ? 'Manual · escribe un nombre o selecciona del catálogo.' : 'Manual · catálogo no disponible para esta cuenta.'}</p>
    {open && canSearch && !readOnly && <div className="quote-client-dropdown">
      <div id={`${id}-list`} role="listbox" aria-label="Clientes" className="quote-client-options">
        <button type="button" role="option" id={`${id}-option-0`} tabIndex={-1} aria-selected={active === 0} className="quote-client-manual" onMouseDown={event => event.preventDefault()} onClick={() => choose(null)}>
          <strong>{value.trim() ? 'Usar este nombre como cliente manual' : 'Dejar sin cliente'}</strong><small>{value || 'Sin vincular al catálogo'}</small>
        </button>
        {choices.map((client, index) => <button key={client.id} type="button" role="option" tabIndex={-1} id={`${id}-option-${index + 1}`} aria-selected={active === index + 1 || (active < 0 && selected?.id === client.id)} onMouseDown={event => event.preventDefault()} onClick={() => choose(client)}>
          <strong>{client.cliente_name}</strong><small>{[client.rtn && `RTN ${client.rtn}`, client.email, `#${client.id}`].filter(Boolean).join(' · ')}</small>
        </button>)}
      </div>
      {pending || (!result.data && !result.error) ? <p role="status">Buscando clientes…</p> : result.error ? <div role="alert"><p>{result.error} Puedes conservar el nombre manual.</p><button type="button" onClick={result.reload}>Reintentar clientes</button></div> : choices.length === 0 && <p role="status">Sin coincidencias. Puedes conservar el nombre manual.</p>}
      {result.data && !pending && (page > 1 || result.data.has_more) && <div className="quote-client-pages"><button type="button" disabled={page === 1} onClick={() => { setPage(value => value - 1); setActive(-1); }}>Anteriores</button><span>Pág. {page}</span><button type="button" disabled={!result.data.has_more} onClick={() => { setPage(value => value + 1); setActive(-1); }}>Más clientes</button></div>}
    </div>}
  </div>;
}
