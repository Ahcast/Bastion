import { useRef, useState } from 'react';
import { ApiError, message } from '../api.js';
import { Heading, Loading, Notice, Table, useCore, useLoad } from '../ui.js';
import { moduleCan } from '../types.js';
import { useNavigationGuard, useUnsavedChanges } from '../navigationGuard.js';
import { clientInput, emptyClient } from '../clients.js';
import type { Client, ClientInput, ClientPage, ClientWrite } from '../clients.js';

const fields = [
  ['cliente_name', 'Cliente', 255, 'text'], ['user_contact', 'Contacto', 255, 'text'],
  ['email', 'Correo', 50, 'email'], ['rtn', 'RTN (opcional)', 30, 'text'],
  ['phone_number', 'Teléfono', 20, 'tel'], ['mobile_number', 'Móvil', 20, 'tel'],
] as const;

export default function Clientes() {
  const { access } = useCore();
  if (!moduleCan(access, 'clientes', 'catalogo', 'ver')) return <Notice>No tienes acceso a Clientes.</Notice>;
  return <Workspace />;
}

function Workspace() {
  const { api, access, notify } = useCore(), { navigate } = useNavigationGuard();
  const [search, setSearch] = useState(''), [query, setQuery] = useState(''), [page, setPage] = useState(1);
  const [saved, setSaved] = useState<Client | null>(null), [data, setData] = useState<ClientInput>(emptyClient), [editing, setEditing] = useState(false);
  const [dirty, setDirty] = useState(false), [busy, setBusy] = useState(false), [error, setError] = useState(''), [conflict, setConflict] = useState(false);
  const attempt = useRef<{ body: string; key: string } | null>(null);
  const heading = useRef<HTMLHeadingElement>(null);
  const editable = moduleCan(access, 'clientes', 'catalogo', 'editar');
  useUnsavedChanges(dirty, busy);
  const result = useLoad(signal => api.request<ClientPage>(`/api/v1/clientes?search=${encodeURIComponent(query)}&page=${page}&page_size=25`, { signal }), [query, page]);
  const reset = () => { setSaved(null); setData(emptyClient()); setEditing(false); setDirty(false); setConflict(false); attempt.current = null; };
  const open = async (id: number) => {
    setError(''); setBusy(true);
    try {
      const client = await api.request<Client>(`/api/v1/clientes/${id}`);
      setSaved(client); setData(clientInput(client));
      setEditing(true); setDirty(false); setConflict(false); attempt.current = null;
      heading.current?.focus();
    } catch (reason) { setError(message(reason)); }
    finally { setBusy(false); }
  };
  const change = (key: keyof ClientInput, value: string) => { setData(old => ({ ...old, [key]: value })); setDirty(true); };
  return <><Heading title="Clientes" description="Catálogo de clientes y contactos." action={editable && <button className="primary" disabled={busy} onClick={() => void navigate(() => { reset(); setEditing(true); })}>Nuevo</button>} />
    <form className="row client-search" onSubmit={event => { event.preventDefault(); setQuery(search); setPage(1); }}><label>Buscar cliente, correo o RTN<input name="search" value={search} maxLength={200} onChange={event => setSearch(event.target.value)} /></label><button type="submit">Buscar</button></form>
    <Notice>{error || result.error}</Notice>
    {editing && editable && <section className="card client-editor" aria-label="Formulario de cliente"><h2 ref={heading} tabIndex={-1}>{saved ? `Editar cliente · ${saved.cliente_name}` : 'Nuevo cliente'}</h2>
      <p>El nombre es obligatorio. Al crear, una coincidencia por nombre o RTN completa solo los datos vacíos del cliente existente.</p>
      {conflict && <div role="status"><p>Tus cambios se conservan en pantalla. Abre la versión actual para revisarlos antes de guardar de nuevo.</p><button disabled={busy} onClick={() => saved && void navigate(() => void open(saved.id))}>Abrir versión actual</button></div>}
      <form onSubmit={async event => {
        event.preventDefault(); if (busy || conflict) return;
        if (!data.cliente_name.trim()) { setError('El nombre del cliente es obligatorio.'); return; }
        setBusy(true); setError('');
        try {
          let merged = false;
          if (saved) await api.request<Client>(`/api/v1/clientes/${saved.id}`, { method: 'PUT', body: data, ifMatch: saved.version });
          else {
            const body = JSON.stringify(data);
            if (!attempt.current || attempt.current.body !== body) attempt.current = { body, key: crypto.randomUUID() };
            const response = await api.request<ClientWrite>('/api/v1/clientes', { method: 'POST', body: data, idempotencyKey: attempt.current.key });
            merged = response.merged;
          }
          reset(); result.reload(); notify(merged ? 'Cliente existente completado; se conservaron sus datos anteriores.' : 'Cliente guardado.');
        } catch (reason) { setError(message(reason)); if (reason instanceof ApiError && reason.status === 412) setConflict(true); }
        finally { setBusy(false); }
      }}><fieldset disabled={busy || conflict} className="form-fields"><div className="client-fields">{fields.map(([key, label, max, type]) => <label key={key}>{label}{key === 'cliente_name' ? ' *' : ''}<input name={key} type={type} maxLength={max} required={key === 'cliente_name'} value={data[key]} onChange={event => change(key, event.target.value)} /></label>)}<label className="client-address">Dirección<textarea name="direction" rows={3} maxLength={1500} value={data.direction} onChange={event => change('direction', event.target.value)} /></label></div><button type="submit" className="primary">{busy ? 'Guardando…' : 'Guardar'}</button></fieldset><button type="button" disabled={busy} onClick={() => void navigate(reset)}>Cancelar</button></form>
    </section>}
    {result.data ? <><Table columns={['Cliente', 'Contacto', 'Correo', 'Móvil / teléfono', 'Acciones']} rows={result.data.items.map(client => [client.cliente_name, client.user_contact || '—', client.email || '—', client.mobile_number || client.phone_number || '—', editable ? <button disabled={busy} onClick={() => void navigate(() => void open(client.id))}>Editar</button> : '—'])} /><nav className="row client-pagination" aria-label="Paginación de clientes"><button disabled={page === 1 || busy} onClick={() => setPage(p => p - 1)}>Anterior</button><span role="status">Página {page}</span><button disabled={!result.data.has_more || busy} onClick={() => setPage(p => p + 1)}>Siguiente</button></nav></> : !result.error && <Loading />}
  </>;
}
