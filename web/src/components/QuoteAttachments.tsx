import { useRef, useState } from 'react';
import { ApiError, message } from '../api.js';
import { Notice, useCore, useLoad } from '../ui.js';
import { downloadFile, encodeAttachment, fileSize, maxAttachmentBytes } from '../quoteAttachments.js';
import type { AttachmentPage, AttachmentWrite, QuoteFile } from '../quoteAttachments.js';
import { reservePdfViewer } from '../quoteDocuments.js';
type Pending = { file: File; key: string };
export function QuoteAttachments({ quoteId, version, editable, disabled, storageReady, versionChanged, pendingChanged, busyChanged, conflict }: {
  quoteId: number | null; version: number; editable: boolean; disabled: boolean; storageReady: boolean;
  versionChanged: (version: number) => void; pendingChanged: (count: number) => void; busyChanged: (value: boolean) => void; conflict: () => void;
}) {
  const { api, notify } = useCore();
  const picker = useRef<HTMLInputElement>(null), queue = useRef<Pending[]>([]), running = useRef(false);
  const [pending, setPending] = useState<Pending[]>([]), [busy, setBusy] = useState(false), [active, setActive] = useState(''), [error, setError] = useState(''), [page, setPage] = useState(1);
  const files = useLoad(signal => quoteId ? api.request<AttachmentPage>(`/api/v1/cotizaciones/${quoteId}/attachments?page=${page}`, { signal }) : Promise.resolve({ items: [], page: 1, has_more: false, quote_version: 0 }), [quoteId, page]);
  const updateQueue = (next: Pending[]) => { queue.current = next; setPending(next); pendingChanged(next.length); };
  const working = (value: boolean) => { running.current = value; setBusy(value); busyChanged(value); };
  const fail = (reason: unknown) => { setError(message(reason)); if (reason instanceof ApiError && reason.status === 412) conflict(); };
  const upload = async () => {
    if (!quoteId || running.current || disabled || !storageReady) return;
    working(true); setError(''); let currentVersion = version, count = 0;
    try {
      for (const entry of [...queue.current]) {
        setActive(entry.file.name);
        const body = { filename: entry.file.name, title: entry.file.name, content_base64: await encodeAttachment(entry.file) };
        const result = await api.request<AttachmentWrite>(`/api/v1/cotizaciones/${quoteId}/attachments`, { method: 'POST', body, ifMatch: currentVersion, idempotencyKey: entry.key });
        currentVersion = result.quote_version; versionChanged(currentVersion); count++;
        updateQueue(queue.current.filter(item => item.key !== entry.key));
      }
      if (count) notify(`${count} archivo${count === 1 ? '' : 's'} adjuntado${count === 1 ? '' : 's'}.`);
    } catch (reason) { fail(reason); }
    finally { setActive(''); working(false); setPage(1); files.reload(); }
  };
  const content = (file: { id: string; mime: string }, preview: boolean) => {
    if (!quoteId || running.current) return;
    setError('');
    let viewer: ReturnType<typeof reservePdfViewer> | undefined;
    try { if (preview) viewer = reservePdfViewer(); } catch (reason) { fail(reason); return; }
    working(true);
    void api.request<QuoteFile>(`/api/v1/cotizaciones/${quoteId}/attachments/${file.id}/content`)
      .then(value => preview ? viewer!.show(value) : downloadFile(value))
      .catch(reason => { viewer?.close(); fail(reason); }).finally(() => working(false));
  };
  return <section className="quote-section quote-attachments" aria-labelledby="quote-attachments-title">
    <div className="quote-section-heading"><h2 id="quote-attachments-title">Archivos adjuntos</h2>{editable && <button type="button" disabled={busy || disabled || !storageReady} onClick={() => picker.current?.click()}>Adjuntar archivos</button>}</div>
    <input ref={picker} type="file" multiple className="quote-sr-only" aria-label="Seleccionar archivos adjuntos" disabled={busy || disabled || !editable || !storageReady} onChange={event => {
      const selected = Array.from(event.target.files ?? []); event.target.value = '';
      if (selected.some(file => !file.size || file.size > maxAttachmentBytes)) { setError('Cada archivo debe tener contenido y medir hasta 20 MiB.'); return; }
      setError(''); updateQueue([...queue.current, ...selected.map(file => ({ file, key: crypto.randomUUID() }))]);
    }} />
    <p className="quote-note">Puedes seleccionar varios archivos. Máximo 20 MiB por archivo.</p>
    {!storageReady && editable && <p className="quote-note">El almacenamiento de archivos no está disponible.</p>}
    <Notice>{error || files.error}</Notice>
    {files.error && <button type="button" disabled={busy} onClick={files.reload}>Reintentar adjuntos</button>}
    {pending.length > 0 && <div className="quote-attachment-pending"><ul>{pending.map(entry => <li key={entry.key}><span>{entry.file.name}<small>{fileSize(entry.file.size)} · pendiente de subir</small></span><button type="button" disabled={busy || disabled} onClick={() => updateQueue(queue.current.filter(item => item.key !== entry.key))} aria-label={`Quitar selección de ${entry.file.name}`}>Quitar</button></li>)}</ul>
      {!quoteId && <p className="quote-note">Guarda la cotización para subir los archivos seleccionados.</p>}
      <button type="button" className="primary" disabled={!quoteId || busy || disabled || !storageReady || !files.data} onClick={() => void upload()}>{busy ? 'Procesando…' : `Subir ${pending.length} archivo${pending.length === 1 ? '' : 's'}`}</button>
    </div>}
    {busy && <p role="status">{active ? `Subiendo ${active}…` : 'Procesando archivo…'}</p>}
    {files.data && files.data.items.length > 0 ? <ul className="quote-attachment-list">{files.data.items.map(file => <li key={file.id}>
      <span>{file.title || file.filename}<small>{fileSize(file.byte_size)}</small></span>
      <div className="actions">{file.mime === 'application/pdf' && <button type="button" disabled={busy || disabled} onClick={() => content(file, true)}>Ver PDF</button>}
      <button type="button" disabled={busy || disabled} onClick={() => content(file, false)}>Descargar</button>
      {editable && <button type="button" disabled={busy || disabled} onClick={async () => {
        if (!quoteId || !confirm(`¿Eliminar el adjunto "${file.filename}"?`)) return;
        working(true); setError('');
        try { const result = await api.request<{ quote_version: number }>(`/api/v1/cotizaciones/${quoteId}/attachments/${file.id}`, { method: 'DELETE', ifMatch: version }); versionChanged(result.quote_version); files.reload(); }
        catch (reason) { fail(reason); } finally { working(false); }
      }}>Eliminar</button>}</div>
    </li>)}</ul> : files.data && <p className="quote-note">No hay archivos adjuntos guardados.</p>}
    {files.data && (page > 1 || files.data.has_more) && <div className="quote-pagination"><button type="button" disabled={page === 1 || busy} onClick={() => setPage(value => value - 1)}>Anteriores</button><span>Página {page}</span><button type="button" disabled={!files.data.has_more || busy} onClick={() => setPage(value => value + 1)}>Siguientes</button></div>}
  </section>;
}
