import { useRef, useState } from 'react';
import { ApiError, message } from '../api.js';
import { Heading, Loading, Notice, Table, useCore, useLoad } from '../ui.js';
import { moduleCan } from '../types.js';
import { useNavigationGuard, useUnsavedChanges } from '../navigationGuard.js';
import { emptySupplier, supplierInput } from '../proveedores.js';
import type { Supplier, SupplierInput, SupplierPage } from '../proveedores.js';
import '../styles/proveedores.css';

const fields = [
  ['supplier_name', 'Proveedor', 255], ['user_contact', 'Contacto', 255], ['email', 'Correo', 254],
  ['rtn', 'RTN', 100], ['phone_number', 'Teléfono', 60], ['mobile_number', 'Móvil', 60],
] as const;

export default function Proveedores() {
  const { access } = useCore();
  if (!moduleCan(access, 'proveedores', 'catalogo', 'ver')) return <Notice>No tienes acceso a Proveedores.</Notice>;
  return <Workspace />;
}

export function SupplierSettings() {
  return <section className="card"><h2>Proveedores</h2><p>Catálogo de proveedores y contactos con búsqueda, alta y edición.</p><p>Esta versión no tiene ajustes editables. Los ajustes propios del módulo siguen pendientes de definición.</p></section>;
}

function Workspace() {
  const { api, access, notify } = useCore(), { navigate } = useNavigationGuard();
  const [search, setSearch] = useState(''), [query, setQuery] = useState(''), [page, setPage] = useState(1);
  const [saved, setSaved] = useState<Supplier | null>(null), [data, setData] = useState<SupplierInput>(emptySupplier), [dirty, setDirty] = useState(false);
  const [busy, setBusy] = useState(false), [error, setError] = useState(''), [conflict, setConflict] = useState(false);
  const attempt = useRef<{ body: string; key: string } | null>(null);
  const heading = useRef<HTMLHeadingElement>(null);
  const editable = moduleCan(access, 'proveedores', 'catalogo', 'editar');
  useUnsavedChanges(dirty, busy);
  const result = useLoad(signal => api.request<SupplierPage>(`/api/v1/proveedores?search=${encodeURIComponent(query)}&page=${page}&page_size=25`, { signal }), [query, page]);

  const open = async (id: number) => {
    setError(''); setBusy(true);
    try {
      const supplier = await api.request<Supplier>(`/api/v1/proveedores/${id}`);
      setSaved(supplier); setData(supplierInput(supplier)); setDirty(false); setConflict(false); attempt.current = null;
      heading.current?.focus();
    } catch (reason) { setError(message(reason)); }
    finally { setBusy(false); }
  };
  const openAfterGuard = (id: number) => { void navigate(() => { void open(id); }); };
  const change = (key: keyof SupplierInput, value: string) => { setData(previous => ({ ...previous, [key]: value })); setDirty(true); };

  return <>
    <Heading title="Proveedores" description="Catálogo de proveedores y contactos." />
    <Notice>{error || result.error}</Notice>
    {editable && <section className="supplier-editor" aria-label="Formulario de proveedor">
      <div className="supplier-section-heading"><h2 ref={heading} tabIndex={-1}>{saved ? `Editar proveedor · ${saved.supplier_name}` : 'Nuevo proveedor'}</h2><span>CATÁLOGO</span></div>
      <p className="supplier-note">El nombre del proveedor es obligatorio. Los demás campos son opcionales.</p>
      {conflict && <div className="supplier-conflict" role="status"><p>Tus cambios se conservan en pantalla. Abre la versión actual antes de volver a guardar.</p><button type="button" disabled={busy} onClick={() => saved && openAfterGuard(saved.id)}>Abrir versión actual</button></div>}
      <form className="supplier-form" onSubmit={async event => {
        event.preventDefault(); if (busy || conflict) return;
        if (!data.supplier_name.trim()) { setError('El nombre del proveedor es obligatorio.'); return; }
        setBusy(true); setError('');
        try {
          if (saved) await api.request<Supplier>(`/api/v1/proveedores/${saved.id}`, { method: 'PUT', body: data, ifMatch: saved.version });
          else {
            const body = JSON.stringify(data);
            if (!attempt.current || attempt.current.body !== body) attempt.current = { body, key: crypto.randomUUID() };
            await api.request<Supplier>('/api/v1/proveedores', { method: 'POST', body: data, idempotencyKey: attempt.current.key });
          }
          setSaved(null); setData(emptySupplier()); setDirty(false); setConflict(false); attempt.current = null;
          result.reload(); notify('Proveedor guardado.');
        } catch (reason) { setError(message(reason)); if (reason instanceof ApiError && reason.status === 412) setConflict(true); }
        finally { setBusy(false); }
      }}>
        <fieldset disabled={busy || conflict} className="supplier-fields">
          {fields.map(([key, label, max]) => <label key={key}>{label}{key === 'supplier_name' ? ' *' : ''}<input name={key} type="text" maxLength={max} required={key === 'supplier_name'} value={data[key]} onChange={event => change(key, event.target.value)} /></label>)}
          <label className="supplier-address">Dirección<textarea name="direction" rows={3} maxLength={1500} value={data.direction} onChange={event => change('direction', event.target.value)} /></label>
          <div className="supplier-save"><button type="submit" className="primary">{busy ? 'Guardando…' : 'Guardar proveedor'}</button></div>
        </fieldset>
      </form>
    </section>}
    <section className="supplier-list" aria-labelledby="supplier-list-title">
      <div className="supplier-section-heading"><h2 id="supplier-list-title">Listado de proveedores</h2><span>{result.data ? `PÁGINA ${page}` : 'CATÁLOGO'}</span></div>
      <form className="supplier-search" onSubmit={event => { event.preventDefault(); setQuery(search); setPage(1); }}><label>Buscar proveedor<input name="search" value={search} maxLength={200} onChange={event => setSearch(event.target.value)} /></label><button type="submit">Buscar</button></form>
      {result.data ? <>
        <Table columns={['Proveedor', 'Contacto', 'Correo', 'Móvil / teléfono', 'Acciones']} rows={result.data.items.map(supplier => [supplier.supplier_name, supplier.user_contact || '—', supplier.email || '—', supplier.mobile_number || supplier.phone_number || '—', editable ? <button className="supplier-edit" disabled={busy} onClick={() => openAfterGuard(supplier.id)}>Editar</button> : '—'])} />
        <nav className="supplier-pagination" aria-label="Paginación de proveedores"><button disabled={page === 1 || busy} onClick={() => setPage(value => value - 1)}>Anterior</button><span role="status">Página {page}</span><button disabled={!result.data.has_more || busy} onClick={() => setPage(value => value + 1)}>Siguiente</button></nav>
      </> : !result.error && <Loading />}
    </section>
  </>;
}
