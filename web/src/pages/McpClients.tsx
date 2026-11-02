import { useCallback, useEffect, useRef, useState } from 'react';
import { ApiClient, message } from '../api.js';
import { Notice } from '../ui.js';

type Scope = 'cotizaciones:read' | 'facturas:read' | 'ordenes_compra:read' | 'licitaciones:read' | 'cotizaciones:write' | 'facturas:write' | 'ordenes_compra:write' | 'licitaciones:write';
type Entry = { id: number; client_name: string; client_id: string; scopes: string[]; approved_scopes?: string[] | null; status: string };
type Page = { items: Entry[]; next_before: number | null };
const options: { scope: Scope; label: string }[] = [
  { scope: 'cotizaciones:read', label: 'Consultar cotizaciones' },
  { scope: 'facturas:read', label: 'Consultar facturas' },
  { scope: 'ordenes_compra:read', label: 'Consultar órdenes de compra' },
  { scope: 'licitaciones:read', label: 'Consultar licitaciones' },
  { scope: 'cotizaciones:write', label: 'Crear y editar cotizaciones en borrador' },
  { scope: 'facturas:write', label: 'Crear y editar facturas en borrador' },
  { scope: 'ordenes_compra:write', label: 'Crear y editar órdenes pendientes sin compras ni entregas' },
  { scope: 'licitaciones:write', label: 'Crear y editar licitaciones en borrador' },
];
function label(scope: string) { return options.find(option => option.scope === scope)?.label ?? scope; }
function validClientId(value: string) {
  try { const url = new URL(value); return url.protocol === 'https:' && !!url.hostname && !url.username && !url.password && !url.hash && url.pathname !== '/'; }
  catch { return false; }
}

/** Draft only; the parent must require an authenticated Bastion session. */
export default function McpClients({ api, isAdministrator }: { api: ApiClient; isAdministrator: boolean }) {
  const [clientId, setClientId] = useState(''), [requested, setRequested] = useState<Scope[]>([]);
  const [own, setOwn] = useState<Page | null>(null), [pending, setPending] = useState<Page | null>(null);
  const [selected, setSelected] = useState<Record<number, string[]>>({});
  const [busy, setBusy] = useState(false), [error, setError] = useState('');
  const working = useRef(false);

  const refresh = useCallback(async () => {
    const ownPage = await api.request<Page>('/oauth/clients/mine', { method: 'POST', body: {} });
    const pendingPage = isAdministrator ? await api.request<Page>('/oauth/clients/pending', { method: 'POST', body: {} }) : null;
    setOwn(ownPage); setPending(pendingPage);
  }, [api, isAdministrator]);
  useEffect(() => { let active = true; void refresh().catch(reason => { if (active) setError(message(reason)); }); return () => { active = false; }; }, [refresh]);

  async function submit() {
    if (working.current) return;
    if (!validClientId(clientId) || requested.length === 0) { setError('Indica la dirección HTTPS del cliente y al menos un permiso.'); return; }
    working.current = true; setBusy(true); setError('');
    try { await api.request('/oauth/clients/request', { method: 'POST', body: { client_id: clientId, scopes: requested } }); setClientId(''); setRequested([]); await refresh(); }
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
  async function loadOlder(admin: boolean) {
    const before = admin ? pending?.next_before : own?.next_before;
    if (!before || working.current || (admin && !isAdministrator)) return;
    working.current = true; setBusy(true); setError('');
    try {
      const page = await api.request<Page>(admin ? '/oauth/clients/pending' : '/oauth/clients/mine', { method: 'POST', body: { before } });
      if (admin) setPending(current => current ? { items: [...current.items, ...page.items], next_before: page.next_before } : page);
      else setOwn(current => current ? { items: [...current.items, ...page.items], next_before: page.next_before } : page);
    } catch (reason) { setError(message(reason)); }
    finally { working.current = false; setBusy(false); }
  }

  return <section><h1>Clientes MCP</h1><p>Solicita la habilitación de una aplicación y selecciona sus permisos en Bastion.</p><Notice>{error}</Notice>
    <label>Dirección de metadatos del cliente <input type="url" value={clientId} maxLength={2048} onChange={event => setClientId(event.currentTarget.value)} /></label>
    <p>Para crear o editar en un módulo, solicita también su permiso de consulta.</p>
    <fieldset><legend>Permisos solicitados</legend>{options.map(option => <label key={option.scope}><input type="checkbox" checked={requested.includes(option.scope)} onChange={event => { const checked = event.currentTarget.checked; setRequested(current => checked ? [...current, option.scope] : current.filter(item => item !== option.scope)); }} /> {option.label}</label>)}</fieldset>
    <button type="button" disabled={busy} onClick={() => void submit()}>Solicitar revisión</button>
    <h2>Mis solicitudes</h2>{own === null ? <p role="status">Cargando…</p> : own.items.length === 0 ? <p>Sin solicitudes.</p> : <ul>{own.items.map(item => <li key={item.id}>{item.client_name}: {item.status === 'approved' ? 'Aprobada' : item.status === 'rejected' ? 'Rechazada' : 'Pendiente'} — Solicitados: {item.scopes.map(label).join(', ')}{item.status === 'approved' && (item.approved_scopes ? ` — Aprobados: ${item.approved_scopes.map(label).join(', ')}` : ' — Detalle de aprobación no disponible')}</li>)}</ul>}
    {own?.next_before && <button type="button" disabled={busy} onClick={() => void loadOlder(false)}>Ver solicitudes anteriores</button>}
    {isAdministrator && <><h2>Solicitudes pendientes</h2>{pending === null ? <p role="status">Cargando…</p> : pending.items.length === 0 ? <p>Sin solicitudes pendientes.</p> : pending.items.map(item => <article key={item.id}>
      <h3>{item.client_name}</h3><p>{item.client_id}</p><fieldset><legend>Permisos a aprobar</legend>{item.scopes.map(scope => <label key={scope}><input type="checkbox" checked={(selected[item.id] ?? []).includes(scope)} onChange={event => { const checked = event.currentTarget.checked; setSelected(current => ({ ...current, [item.id]: checked ? [...(current[item.id] ?? []), scope] : (current[item.id] ?? []).filter(value => value !== scope) })); }} /> {label(scope)}</label>)}</fieldset>
      <button type="button" disabled={busy} onClick={() => void review(item, 'approve')}>Aprobar</button>{' '}<button type="button" className="ghost" disabled={busy} onClick={() => void review(item, 'reject')}>Rechazar</button>
    </article>)}{pending?.next_before && <button type="button" disabled={busy} onClick={() => void loadOlder(true)}>Ver solicitudes pendientes anteriores</button>}</>}
  </section>;
}
