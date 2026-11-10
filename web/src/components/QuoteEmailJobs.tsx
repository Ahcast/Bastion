import { useState } from 'react';
import { message } from '../api.js';
import { emailFailure } from '../quoteDocuments.js';
import type { QuoteEmailJob } from '../quoteDocuments.js';
import { quoteSchedule } from '../quotes.js';
import type { QuoteSummary } from '../quotes.js';
import { Dialog, Loading, Notice, Table, useCore, useLoad } from '../ui.js';

const status = { pending: 'En cola', delivering: 'Enviando', sent: 'Enviado', failed: 'Fallido', cancelled: 'Cancelado' };

export function QuoteEmailJobs({ quote, close, changed }: { quote: QuoteSummary; close: () => void; changed: () => void }) {
  const { api, notify } = useCore();
  const path = `/api/v1/cotizaciones/${quote.id}/emails`;
  const result = useLoad(signal => api.request<QuoteEmailJob[]>(path, { signal }), [quote.id]);
  const [busy, setBusy] = useState(false), [error, setError] = useState('');

  async function change(job: QuoteEmailJob, action: 'cancel' | 'retry') {
    if (busy || !window.confirm(action === 'cancel' ? '¿Cancelar este envío programado?' : '¿Volver a poner este correo en cola para envío inmediato?')) return;
    setBusy(true); setError('');
    try {
      await api.request(`${path}/${job.id}/${action}`, { method: 'POST' });
      notify(action === 'cancel' ? 'Envío cancelado.' : 'Envío nuevamente en cola.');
      changed();
    } catch (reason) { setError(message(reason)); }
    finally { result.reload(); setBusy(false); }
  }

  return <Dialog title={`Envíos de ${quote.quote_number ?? 'la cotización'}`} close={() => { if (!busy) close(); }}>
    <Notice>{error || result.error}</Notice>
    <button type="button" disabled={busy} onClick={result.reload}>Actualizar estado</button>
    {result.data ? result.data.length ? <Table columns={['Envío', 'Programado', 'Estado', 'Acción']} rows={result.data.map(job => [
      `#${job.id}`, quoteSchedule(job.scheduled_at) ?? '—',
      <>{status[job.delivery_status]}{job.delivery_status === 'failed' && <p>{emailFailure(job.last_error_code)}</p>}</>,
      job.delivery_status === 'pending' ? <button disabled={busy} onClick={() => void change(job, 'cancel')}>Cancelar envío</button> :
        job.delivery_status === 'failed' && job.last_error_code === 'configuration_invalid'
          ? <button disabled={busy} onClick={() => void change(job, 'retry')}>Reintentar</button> : '—',
    ])} /> : <p>No tienes envíos para esta cotización.</p> : !result.error && <Loading />}
  </Dialog>;
}
