import { useRef, useState } from 'react';
import { ApiClient, message } from '../api.js';
import { Notice } from '../ui.js';

type PendingConsent = { handle: string; client_name: string; scopes: string[]; expires_at: string };
type Decision = 'approve' | 'deny';

const scopeLabels: Record<string, string> = {
  'cotizaciones:read': 'Consultar cotizaciones',
  'facturas:read': 'Consultar facturas',
  'ordenes_compra:read': 'Consultar órdenes de compra',
  'licitaciones:read': 'Consultar licitaciones',
  'cotizaciones:write': 'Crear y editar cotizaciones en borrador',
  'facturas:write': 'Crear y editar facturas en borrador',
  'ordenes_compra:write': 'Crear y editar órdenes pendientes sin compras ni entregas',
  'licitaciones:write': 'Crear y editar licitaciones en borrador',
};

function validPending(value: unknown): value is PendingConsent {
  if (typeof value !== 'object' || value === null) return false;
  const item = value as Record<string, unknown>;
  return typeof item.handle === 'string' && /^[A-Za-z0-9_-]{43}$/.test(item.handle)
    && typeof item.client_name === 'string' && item.client_name.length > 0 && item.client_name.length <= 255
    && Array.isArray(item.scopes) && item.scopes.length > 0 && item.scopes.length <= 64
    && item.scopes.every(scope => typeof scope === 'string' && scope.length > 0 && scope.length <= 191)
    && typeof item.expires_at === 'string' && !Number.isNaN(Date.parse(item.expires_at));
}

function safeRedirect(value: unknown): string {
  if (typeof value !== 'string' || value.length > 8192) throw new Error('No se pudo completar la solicitud.');
  let url: URL;
  try { url = new URL(value); } catch { throw new Error('No se pudo completar la solicitud.'); }
  const loopback = url.hostname === '127.0.0.1' || url.hostname === '[::1]' || url.hostname === 'localhost';
  if (url.username || url.password || url.hash || !(url.protocol === 'https:' || (url.protocol === 'http:' && loopback))) {
    throw new Error('No se pudo completar la solicitud.');
  }
  return url.href;
}

/** Draft UI. The parent must authenticate the user and remove OAuth query
 * parameters from browser history before rendering this page. */
export default function McpConsent({ api, authorizationQuery, navigate = url => window.location.assign(url) }: {
  api: ApiClient; authorizationQuery: string; navigate?: (url: string) => void;
}) {
  const [pending, setPending] = useState<PendingConsent | null>(null);
  const [busy, setBusy] = useState(false), [error, setError] = useState('');
  const inFlight = useRef(false);

  async function begin() {
    if (inFlight.current) return;
    inFlight.current = true; setBusy(true); setError('');
    try {
      const result = await api.request<unknown>('/oauth/consent/begin', { method: 'POST', body: { authorization_query: authorizationQuery } });
      if (!validPending(result)) throw new Error('No se pudo verificar la solicitud.');
      setPending(result);
    } catch (reason) { setError(message(reason)); }
    finally { inFlight.current = false; setBusy(false); }
  }

  async function decide(decision: Decision) {
    if (!pending || inFlight.current) return;
    inFlight.current = true; setBusy(true); setError('');
    try {
      const result = await api.request<unknown>('/oauth/consent/decision', { method: 'POST', body: { handle: pending.handle, decision } });
      if (typeof result !== 'object' || result === null || !('redirect_to' in result)) throw new Error('No se pudo completar la solicitud.');
      navigate(safeRedirect(result.redirect_to));
    } catch (reason) { setError(message(reason)); inFlight.current = false; setBusy(false); }
  }

  return <section className="auth-wrap"><div className="auth-card">
    <h1>Autorizar una aplicación</h1>
    <Notice>{error}</Notice>
    {!pending ? <><p>Revisa los permisos solicitados antes de decidir si deseas conectar esta aplicación con Bastion.</p>
      <button type="button" disabled={busy} onClick={() => void begin()}>{busy ? 'Verificando…' : 'Revisar solicitud'}</button></>
      : <><p><strong>{pending.client_name}</strong> solicita acceso a tu cuenta.</p>
        <p>Si autorizas, podrá realizar estas consultas según tus permisos vigentes:</p>
        <ul>{pending.scopes.map(scope => <li key={scope}>{Object.hasOwn(scopeLabels, scope) ? scopeLabels[scope] : scope}</li>)}</ul>
        <p>La aplicación no obtiene permisos que tu cuenta no tenga. Puedes rechazar esta solicitud.</p>
        <div className="actions"><button type="button" disabled={busy} onClick={() => void decide('approve')}>Autorizar</button>{' '}
          <button type="button" className="ghost" disabled={busy} onClick={() => void decide('deny')}>Rechazar</button></div></>}
  </div></section>;
}
