import { useRef, useState } from 'react';
import type { ApiClient } from '../api.js';
import { message } from '../api.js';
import { Notice } from '../ui.js';

type MailReview = {
  status: string; quote_id: number; quote_number: string | null; version: number;
  client_name: string; expires_at: string; attachments: string[];
  input: { to: string; cc: string; bcc: string; subject: string; message: string; scheduled_at: string };
};

function validReview(value: unknown): value is MailReview {
  if (!value || typeof value !== 'object') return false;
  const row = value as Partial<MailReview>;
  return typeof row.status === 'string' && Number.isSafeInteger(row.quote_id) && Number.isSafeInteger(row.version)
    && (row.quote_number === null || typeof row.quote_number === 'string')
    && typeof row.client_name === 'string' && typeof row.expires_at === 'string'
    && Array.isArray(row.attachments) && row.attachments.every(name => typeof name === 'string')
    && !!row.input && ['to', 'cc', 'bcc', 'subject', 'message', 'scheduled_at'].every(key => typeof (row.input as unknown as Record<string, unknown>)[key] === 'string')
    && !Number.isNaN(Date.parse(row.input.scheduled_at));
}

export default function McpQuoteMail({ api, handle }: { api: ApiClient; handle: string }) {
  const [review, setReview] = useState<MailReview | null>(null);
  const [busy, setBusy] = useState(false), [error, setError] = useState(''), [status, setStatus] = useState('');
  const inFlight = useRef(false);
  async function load() {
    if (inFlight.current) return;
    inFlight.current = true; setBusy(true); setError('');
    try {
      const value = await api.request<unknown>('/oauth/quote-email/review', { method: 'POST', body: { handle } });
      if (!validReview(value)) throw new Error('No se pudo verificar la solicitud.');
      setReview(value); setStatus(value.status);
    } catch (reason) { setError(message(reason)); }
    finally { inFlight.current = false; setBusy(false); }
  }
  async function decide(decision: 'approve' | 'deny') {
    if (!review || inFlight.current) return;
    inFlight.current = true; setBusy(true); setError('');
    try {
      const value = await api.request<{ status: string }>('/oauth/quote-email/decision', { method: 'POST', body: { handle, decision } });
      if (!['queued', 'denied'].includes(value.status)) throw new Error('No se pudo verificar el resultado. Revisa la solicitud antes de reintentar.');
      setStatus(value.status);
    } catch (reason) { setError(message(reason)); }
    finally { inFlight.current = false; setBusy(false); }
  }
  const pending = ['awaiting_user_confirmation', 'confirmed'].includes(status);
  return <section className="auth-wrap"><div className="auth-card">
    <h1>Confirmar envío de cotización</h1>
    <Notice>{error}</Notice>
    {!review ? <><p>Una aplicación solicita programar un correo en tu nombre. Revisa sus datos antes de decidir.</p><button disabled={busy || !handle} onClick={() => void load()}>Revisar envío</button></> : <>
      <p><strong>{review.quote_number || String(review.quote_id)}</strong> · {review.client_name} · Versión {review.version}</p>
      <dl><dt>Para</dt><dd>{review.input.to}</dd><dt>CC</dt><dd>{review.input.cc || 'Sin copias'}</dd><dt>CCO</dt><dd>{review.input.bcc || 'Sin copias ocultas'}</dd>
        <dt>Fecha de envío en Honduras</dt><dd>{new Date(review.input.scheduled_at).toLocaleString('es-HN', { timeZone: 'America/Tegucigalpa' })}</dd>
        <dt>Asunto</dt><dd>{review.input.subject}</dd><dt>Mensaje</dt><dd style={{ whiteSpace: 'pre-wrap' }}>{review.input.message}</dd>
        <dt>Adjuntos</dt><dd>PDF de la cotización{review.attachments.length > 0 && <ul>{review.attachments.map((name, index) => <li key={index}>{name}</li>)}</ul>}</dd>
      </dl>
      {pending && <><p>Al confirmar se encolará este correo usando tus credenciales de envío. La entrega se realizará a partir de la fecha indicada.</p><div className="actions"><button disabled={busy} onClick={() => void decide('approve')}>Confirmar y programar envío</button>{status !== 'confirmed' && <button disabled={busy} onClick={() => void decide('deny')}>Rechazar</button>}</div></>}
      {status === 'queued' && <p role="status">Envío encolado. El agente puede consultar su estado. Puedes cerrar esta página.</p>}
      {status === 'denied' && <p role="status">Solicitud rechazada.</p>}
      {status === 'expired' && <p role="status">La solicitud venció. Pide al agente que prepare una nueva.</p>}
    </>}
  </div></section>;
}
