import { useCallback, useEffect, useRef, useState } from 'react';
import { ApiClient, message } from '../api.js';
import { Notice } from '../ui.js';

type Entry = { id: number; client_name: string; client_id: string; scopes: string[] };
type Page = { items: Entry[]; next_before: number | null };
type Managed = { id: number; client_id: string; client_name: string; scopes: string[] };
type ManagedPage = { items: Managed[]; next_before: number | null };
type Connection = Managed & { expires_at: string; authorizations: number; additional_scopes_available: boolean };
type ConnectionPage = { items: Connection[]; next_before: number | null };
const options = [
  { scope: 'cotizaciones:read', label: 'Consultar cotizaciones' },
  { scope: 'facturas:read', label: 'Consultar facturas' },
  { scope: 'ordenes_compra:read', label: 'Consultar órdenes de compra' },
  { scope: 'licitaciones:read', label: 'Consultar licitaciones' },
  { scope: 'procesos:read', label: 'Consultar procesos vigentes' },
  { scope: 'clientes:read', label: 'Consultar clientes' },
  { scope: 'proveedores:read', label: 'Consultar proveedores' },
  { scope: 'gastos:read', label: 'Consultar gastos' },
  { scope: 'colaboracion:read', label: 'Consultar metadatos de archivos de empresa' },
  { scope: 'reportes:ordenes:read', label: 'Consultar reportes de órdenes de compra' },
  { scope: 'reportes:facturas:read', label: 'Consultar reportes de facturas' },
  { scope: 'reportes:gastos:read', label: 'Consultar reportes de gastos' },
  { scope: 'cotizaciones:write', label: 'Crear y editar cotizaciones en borrador y solicitar envíos con confirmación' },
  { scope: 'facturas:write', label: 'Crear y editar facturas en borrador' },
  { scope: 'ordenes_compra:write', label: 'Crear y editar órdenes pendientes sin compras ni entregas' },
  { scope: 'licitaciones:write', label: 'Crear y editar licitaciones en borrador' },
] as const;
type Scope = (typeof options)[number]['scope'];
function label(scope: string) { return options.find(option => option.scope === scope)?.label ?? scope; }
function validClientId(value: string) {
  try { const url = new URL(value); return url.protocol === 'https:' && !!url.hostname && !url.username && !url.password && !url.hash && url.pathname !== '/'; }
  catch { return false; }
}

/** Draft only; the parent must require an authenticated Bastion session. */
export default function McpClients({ api, isAdministrator }: { api: ApiClient; isAdministrator: boolean }) {
  const [clientId, setClientId] = useState(''), [requested, setRequested] = useState<Scope[]>([]);
  const [pending, setPending] = useState<Page | null>(null);
  const [clients, setClients] = useState<ManagedPage | null>(null);
  const [connections, setConnections] = useState<ConnectionPage | null>(null);
  const [disconnecting, setDisconnecting] = useState<Connection | null>(null);
  const [editing, setEditing] = useState<Managed | null>(null), [editScopes, setEditScopes] = useState<string[]>([]);
  const [removing, setRemoving] = useState<Managed | null>(null);
  const [selected, setSelected] = useState<Record<number, string[]>>({});
  const [busy, setBusy] = useState(false), [error, setError] = useState('');
  const [submitted, setSubmitted] = useState(false);
  const working = useRef(false);

  const refresh = useCallback(async () => {
    const [connectionPage, pendingPage, clientPage] = await Promise.all([
      api.request<ConnectionPage>('/oauth/connections/list', { method: 'POST', body: {} }),
      isAdministrator ? api.request<Page>('/oauth/clients/pending', { method: 'POST', body: {} }) : null,
      isAdministrator ? api.request<ManagedPage>('/oauth/clients/admin/list', { method: 'POST', body: {} }) : null,
    ]);
    setConnections(connectionPage);
    setPending(pendingPage); setClients(clientPage);
  }, [api, isAdministrator]);
  useEffect(() => { let active = true; void refresh().catch(reason => { if (active) setError(message(reason)); }); return () => { active = false; }; }, [refresh]);

  async function submit() {
    if (working.current) return;
    if (!validClientId(clientId) || requested.length === 0) { setError('Indica la dirección HTTPS del cliente y al menos un permiso.'); return; }
    working.current = true; setBusy(true); setError(''); setSubmitted(false);
    try { await api.request('/oauth/clients/request', { method: 'POST', body: { client_id: clientId, scopes: requested } }); setClientId(''); setRequested([]); setSubmitted(true); await refresh(); }
    catch (reason) { setError(message(reason)); }
    finally { working.current = false; setBusy(false); }
  }
  async function review(entry: Entry, decision: 'approve' | 'reject') {
    if (working.current || !isAdministrator) return;
    const scopes = selected[entry.id] ?? [];
    if (decision === 'approve' && scopes.length === 0) { setError('Selecciona al menos un permiso para aprobar.'); return; }
    working.current = true; setBusy(true); setError('');
    try {
      const body = decision === 'approve' ? { request_id: entry.id, decision, client_id: entry.client_id, scopes } : { request_id: entry.id, decision };
      await api.request('/oauth/clients/review', { method: 'POST', body });
      await refresh();
    } catch (reason) { setError(message(reason)); }
    finally { working.current = false; setBusy(false); }
  }
  async function loadOlderPending() {
    const before = pending?.next_before;
    if (!before || working.current || !isAdministrator) return;
    working.current = true; setBusy(true); setError('');
    try {
      const page = await api.request<Page>('/oauth/clients/pending', { method: 'POST', body: { before } });
      setPending(current => current ? { items: [...current.items, ...page.items], next_before: page.next_before } : page);
    } catch (reason) { setError(message(reason)); }
    finally { working.current = false; setBusy(false); }
  }
  async function loadMoreClients() {
    if (!clients?.next_before || working.current || !isAdministrator) return;
    working.current = true; setBusy(true); setError('');
    try { const page = await api.request<ManagedPage>('/oauth/clients/admin/list', { method: 'POST', body: { before: clients.next_before } }); setClients(current => current ? { items: [...current.items, ...page.items], next_before: page.next_before } : page); }
    catch (reason) { setError(message(reason)); }
    finally { working.current = false; setBusy(false); }
  }
  async function saveClient() {
    if (!editing || !isAdministrator || working.current) return;
    if (!editScopes.length) { setError('Selecciona al menos un permiso; para retirar todos, elimina el cliente.'); return; }
    if (editScopes.some(scope => scope.endsWith(':write') && !editScopes.includes(`${scope.slice(0, -6)}:read`))) { setError('Para habilitar escritura también debes seleccionar la consulta del mismo módulo.'); return; }
    working.current = true; setBusy(true); setError('');
    try { await api.request('/oauth/clients/admin/scopes', { method: 'POST', body: { id: editing.id, expected_scopes: editing.scopes, scopes: editScopes } }); setEditing(null); await refresh(); }
    catch (reason) { setError(message(reason)); }
    finally { working.current = false; setBusy(false); }
  }
  async function removeClient() {
    if (!removing || !isAdministrator || working.current) return;
    working.current = true; setBusy(true); setError('');
    try { await api.request('/oauth/clients/admin/disable', { method: 'POST', body: { id: removing.id, confirmed_client_id: removing.client_id } }); setRemoving(null); setEditing(null); await refresh(); }
    catch (reason) { setError(message(reason)); }
    finally { working.current = false; setBusy(false); }
  }

  async function disconnect() {
    if (!disconnecting || working.current) return;
    working.current = true; setBusy(true); setError('');
    try { await api.request('/oauth/connections/disconnect', { method: 'POST', body: { id: disconnecting.id, confirmed_client_id: disconnecting.client_id } }); setDisconnecting(null); await refresh(); }
    catch (reason) { setError(message(reason)); }
    finally { working.current = false; setBusy(false); }
  }
  async function moreConnections() {
    if (!connections?.next_before || working.current) return;
    working.current = true; setBusy(true); setError('');
    try { const page = await api.request<ConnectionPage>('/oauth/connections/list', { method: 'POST', body: { before: connections.next_before } }); setConnections(current => current ? { items: [...current.items, ...page.items], next_before: page.next_before } : page); }
    catch (reason) { setError(message(reason)); }
    finally { working.current = false; setBusy(false); }
  }

  return <section><h1>Clientes MCP</h1><Notice>{error}</Notice>
    <p>En tu aplicación agrega el servidor <code>https://api.bastion.castilloit.net/mcp</code> e inicia la conexión. Bastion te guía para revisar la aplicación y autorizar sus permisos.</p>
    <p>Una aplicación nueva requiere aprobación administrativa. Guardar nuevos permisos aquí requiere también volver a autorizar desde la aplicación para que pueda usarlos.</p>
    <h2>Mis conexiones activas</h2><p>Estos son los accesos y permisos vigentes de las aplicaciones en tu cuenta.</p>
    {connections === null ? <p role="status">Cargando conexiones…</p> : connections.items.length === 0 ? <p>Sin conexiones activas. Inicia la conexión desde tu aplicación.</p> : connections.items.map(connection => <article key={connection.id}>
      <h3>{connection.client_name}</h3><p>{connection.client_id}</p><p>Permisos: {connection.scopes.map(label).join(', ')}</p>
      <p>Autorizaciones vigentes: {connection.authorizations}. La más reciente vence el {new Date(connection.expires_at).toLocaleString('es-HN')}.</p>
      {connection.additional_scopes_available && <p>Hay permisos adicionales aprobados. Vuelve a autorizar desde la aplicación para solicitarlos.</p>}
      <button type="button" disabled={busy} onClick={() => setDisconnecting(connection)}>Desconectar mis accesos</button>
      {disconnecting?.id === connection.id && <section aria-label={`Confirmar desconexión de ${connection.client_name}`}><p>Se revocarán todos los accesos de esta aplicación a tu cuenta. Para usarla otra vez, inicia una conexión nueva desde la aplicación.</p><button type="button" disabled={busy} onClick={() => void disconnect()}>Confirmar desconexión</button>{' '}<button type="button" disabled={busy} onClick={() => setDisconnecting(null)}>Cancelar</button></section>}
    </article>)}
    {connections?.next_before && <button type="button" disabled={busy} onClick={() => void moreConnections()}>Ver más conexiones</button>}
    <details><summary>Solicitar revisión manual de un cliente</summary><p>Usa esta opción si conoces la dirección de metadatos CIMD de la aplicación.</p>
    <label>Dirección de metadatos del cliente <input type="url" value={clientId} maxLength={2048} onChange={event => setClientId(event.currentTarget.value)} /></label>
    <p>Para crear o editar en un módulo, solicita también su permiso de consulta.</p>
    <fieldset><legend>Permisos solicitados</legend>{options.map(option => <label key={option.scope}><input type="checkbox" checked={requested.includes(option.scope)} onChange={event => { const checked = event.currentTarget.checked; setRequested(current => checked ? [...current, option.scope] : current.filter(item => item !== option.scope)); }} /> {option.label}</label>)}</fieldset>
    <button type="button" disabled={busy} onClick={() => void submit()}>Solicitar revisión</button>
    {submitted && <p role="status">Solicitud enviada para revisión.</p>}
    </details>
    {isAdministrator && <><h2>Clientes autorizados</h2><p>Los cambios afectan a todos los usuarios del cliente. Al retirar permisos se revocan las autorizaciones afectadas.</p>
    {clients === null ? <p role="status">Cargando…</p> : clients.items.length === 0 ? <p>Sin clientes autorizados.</p> : clients.items.map(client => <article key={client.id}>
      <h3>{client.client_name}</h3><p>{client.client_id}</p><p>Permisos: {client.scopes.map(label).join(', ') || 'Ninguno'}</p>
      <button type="button" disabled={busy} onClick={() => { setRemoving(null); setEditing(client); setEditScopes([...client.scopes]); setError(''); }}>Editar permisos</button>{' '}
      <button type="button" disabled={busy} onClick={() => { setEditing(null); setRemoving(client); setError(''); }}>Eliminar cliente</button>
      {editing?.id === client.id && <section aria-label={`Editar permisos de ${client.client_name}`}><p>Los usuarios deberán autorizar nuevamente si se retiran permisos que tenían.</p><fieldset><legend>Permisos autorizados</legend>{options.map(option => <label key={option.scope}><input type="checkbox" checked={editScopes.includes(option.scope)} onChange={event => { const checked = event.currentTarget.checked; setEditScopes(current => checked ? [...current, option.scope] : current.filter(value => value !== option.scope)); }} /> {option.label}</label>)}</fieldset><button type="button" disabled={busy} onClick={() => void saveClient()}>Guardar permisos</button>{' '}<button type="button" disabled={busy} onClick={() => setEditing(null)}>Cancelar</button></section>}
      {removing?.id === client.id && <section aria-label={`Confirmar eliminación de ${client.client_name}`}><p>Se desactivará este cliente para todos los usuarios, se revocarán sus accesos y se rechazarán sus solicitudes pendientes. Se conservará el historial.</p><button type="button" disabled={busy} onClick={() => void removeClient()}>Confirmar eliminación</button>{' '}<button type="button" disabled={busy} onClick={() => setRemoving(null)}>Cancelar</button></section>}
    </article>)}{clients?.next_before && <button type="button" disabled={busy} onClick={() => void loadMoreClients()}>Ver más clientes</button>}
    <h2>Solicitudes pendientes</h2>{pending === null ? <p role="status">Cargando…</p> : pending.items.length === 0 ? <p>Sin solicitudes pendientes.</p> : pending.items.map(item => <article key={item.id}>
      <h3>{item.client_name}</h3><p>{item.client_id}</p><fieldset><legend>Permisos a aprobar</legend>{item.scopes.map(scope => <label key={scope}><input type="checkbox" checked={(selected[item.id] ?? []).includes(scope)} onChange={event => { const checked = event.currentTarget.checked; setSelected(current => ({ ...current, [item.id]: checked ? [...(current[item.id] ?? []), scope] : (current[item.id] ?? []).filter(value => value !== scope) })); }} /> {label(scope)}</label>)}</fieldset>
      <button type="button" disabled={busy} onClick={() => void review(item, 'approve')}>Aprobar</button>{' '}<button type="button" className="ghost" disabled={busy} onClick={() => void review(item, 'reject')}>Rechazar</button>
    </article>)}{pending?.next_before && <button type="button" disabled={busy} onClick={() => void loadOlderPending()}>Ver solicitudes pendientes anteriores</button>}</>}
  </section>;
}
