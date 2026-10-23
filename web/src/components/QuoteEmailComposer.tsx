import { useEffect, useRef, useState } from 'react';
import { ApiError, message } from '../api.js';
import { Dialog, Loading, Notice, useCore, useLoad } from '../ui.js';
import { downloadPdf, emailFailure, hondurasSchedule } from '../quoteDocuments.js';
import type { QuoteEmailInput, QuoteEmailJob, QuotePdf } from '../quoteDocuments.js';
import type { QuoteDraft, QuoteSummary } from '../quotes.js';

export function QuoteEmailComposer({ summary, close, sent }: { summary: QuoteSummary; close: () => void; sent: () => void }) {
  const { api, notify } = useCore();
  const [submitting, setSubmitting] = useState(false);
  const loaded = useLoad(async signal => {
    const [draft, pdf] = await Promise.all([api.request<QuoteDraft>(`/api/v1/cotizaciones/${summary.id}`, { signal }), api.request<QuotePdf>(`/api/v1/cotizaciones/${summary.id}/pdf`, { signal, ifMatch: summary.version })]);
    if (draft.version !== summary.version) throw new Error('La cotización cambió. Cierra y vuelve a abrir el envío desde la versión actual.');
    return { draft, pdf };
  }, [summary.id, summary.version]);
  return <Dialog title={`Enviar ${summary.quote_number ?? 'cotización'} por correo`} close={() => { if (!submitting) close(); }}><Notice>{loaded.error}</Notice>{loaded.error && <button onClick={loaded.reload}>Reintentar PDF</button>}{loaded.data ? <Composer initial={loaded.data.draft} pdf={loaded.data.pdf} submitting={setSubmitting} sent={() => { sent(); notify('Cotización enviada.'); }} /> : !loaded.error && <Loading />}</Dialog>;
}
function Composer({ initial, pdf, sent, submitting }: { initial: QuoteDraft; pdf: QuotePdf; sent: () => void; submitting: (value: boolean) => void }) {
  const { api, notify } = useCore();
  const process = initial.data.order_number || initial.quote_number || '';
  const [fields, setFields] = useState({ to: initial.data.client_email ?? '', cc: '', bcc: '', subject: `Cotización ${process}`, message: `Estimados ${initial.data.client_name || 'Cliente'}\n\nAdjunto la cotización correspondiente al proceso ${process}`, scheduled: '' });
  const [busy, setBusy] = useState(false), [error, setError] = useState(''), [job, setJob] = useState<QuoteEmailJob | null>(null), [conflict, setConflict] = useState(false);
  const attempt = useRef<{ body: string; key: string } | null>(null);
  useEffect(() => {
    if (!job || !['pending', 'delivering'].includes(job.delivery_status)) return;
    const controller = new AbortController();
    const timer = setTimeout(() => {
      void api.request<QuoteEmailJob>(`/api/v1/cotizaciones/${initial.id}/emails/${job.id}`, { signal: controller.signal }).then(updated => {
        if (controller.signal.aborted) return;
        setJob(updated);
        if (updated.delivery_status === 'sent') sent();
      }).catch(reason => { if (!controller.signal.aborted) setError(message(reason)); });
    }, 2500);
    return () => { clearTimeout(timer); controller.abort(); };
  }, [job, api, initial.id]);
  return <><div className="quote-email-document"><strong>PDF de la cotización</strong><p>{pdf.filename}</p><button onClick={() => downloadPdf(pdf)}>Ver / descargar PDF</button><p>El PDF se adjunta al correo. Las notas privadas no se incluyen.</p></div><Notice>{error}</Notice>{job ? <section aria-label="Estado del correo" role="status"><p>{job.delivery_status === 'sent' ? 'Correo enviado.' : job.delivery_status === 'failed' ? emailFailure(job.last_error_code) : job.delivery_status === 'cancelled' ? 'Envío cancelado.' : job.delivery_status === 'delivering' ? 'Enviando correo…' : fields.scheduled ? 'Envío programado. Puedes cerrar esta ventana.' : 'Envío en cola. Puedes cerrar esta ventana.'}</p>{job.delivery_status === 'failed' && <button onClick={() => { setJob(null); attempt.current = null; setError(''); }}>Preparar otro envío</button>}{error && ['pending', 'delivering'].includes(job.delivery_status) && <button onClick={async () => { try { const updated = await api.request<QuoteEmailJob>(`/api/v1/cotizaciones/${initial.id}/emails/${job.id}`); setError(''); setJob(updated); if (updated.delivery_status === 'sent') sent(); } catch (reason) { setError(message(reason)); } }}>Consultar estado</button>}</section> : <form className="stack" onSubmit={async event => {
    event.preventDefault(); if (busy || conflict) return;
    setError(''); setBusy(true); submitting(true);
    try {
      const input: QuoteEmailInput = { to: fields.to, cc: fields.cc, bcc: fields.bcc, subject: fields.subject, message: fields.message, scheduled_at: hondurasSchedule(fields.scheduled) };
      const body = JSON.stringify(input);
      if (!attempt.current || attempt.current.body !== body) attempt.current = { body, key: crypto.randomUUID() };
      setJob(await api.request<QuoteEmailJob>(`/api/v1/cotizaciones/${initial.id}/emails`, { method: 'POST', body: input, ifMatch: initial.version, idempotencyKey: attempt.current.key }));
      notify(fields.scheduled ? 'Envío programado.' : 'Envío en cola.');
    } catch (reason) { setError(message(reason)); if (reason instanceof ApiError && reason.status === 412) setConflict(true); }
    finally { setBusy(false); submitting(false); }
  }}><fieldset className="form-fields" disabled={busy || conflict}>
    <label>Para<input type="email" maxLength={255} value={fields.to} onChange={e => setFields({ ...fields, to: e.target.value })} /></label>
    <label>CC<input maxLength={1000} value={fields.cc} onChange={e => setFields({ ...fields, cc: e.target.value })} placeholder="Correos separados por coma o punto y coma" /></label>
    <label>CCO<input maxLength={1000} value={fields.bcc} onChange={e => setFields({ ...fields, bcc: e.target.value })} /></label>
    <label>Asunto<input maxLength={255} value={fields.subject} onChange={e => setFields({ ...fields, subject: e.target.value })} /></label>
    <label>Mensaje<textarea rows={6} maxLength={5000} value={fields.message} onChange={e => setFields({ ...fields, message: e.target.value })} /></label>
    <label>Programar envío (hora de Honduras)<input type="datetime-local" value={fields.scheduled} onChange={e => setFields({ ...fields, scheduled: e.target.value })} /><small>Vacío envía ahora.</small></label>
    <button className="primary" type="submit">{busy ? 'Preparando envío…' : fields.scheduled ? 'Programar envío' : 'Enviar ahora'}</button>
  </fieldset></form>}</>;
}
