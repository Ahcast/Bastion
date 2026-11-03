import { useEffect, useRef, useState } from 'react';
import { message } from '../api.js';
import { Loading, Notice, useCore, useLoad } from '../ui.js';
import { useUnsavedChanges } from '../navigationGuard.js';
import type { Expedient } from '../tenders.js';

type Entry = { key: string; label: string; title_editable?: boolean };
const signature = (items: Entry[]) => JSON.stringify(items.map(({ key, label }) => ({ key, label })));
type Order = { items: Entry[]; version: number };
type PdfJob = { id: string; expediente_version: number; state: 'queued' | 'running' | 'uploading' | 'ready' | 'failed' | 'expired'; byte_size: number | null };
export default function TenderPdfOrder({ record, latest = () => record, editable, saved }: { record: Expedient; latest?: () => Expedient; editable: boolean; saved: (value: Expedient) => void }) {
  const { api } = useCore();
  const result = useLoad(signal => api.request<Order>(`/api/v1/licitaciones/${record.id}/pdf/orden`, { signal }), [record.id, record.version]);
  const [items, setItems] = useState<Entry[]>([]), [busy, setBusy] = useState(false), [error, setError] = useState('');
  const [job, setJob] = useState<PdfJob | null>(null), [generating, setGenerating] = useState(false), [jobsLoaded, setJobsLoaded] = useState(false);
  const baseline = useRef('[]');
  const original = useRef<Entry[]>([]);
  useEffect(() => {
    let active = true;
    api.request<PdfJob | null>(`/api/v1/licitaciones/${record.id}/pdf/ultimo`).then(value => { if (active) setJob(value); }).catch(reason => { if (active) setError(message(reason)); }).finally(() => { if (active) setJobsLoaded(true); });
    return () => { active = false; };
  }, [api, record.id]);
  useEffect(() => {
    if (!job || !['queued', 'running', 'uploading'].includes(job.state)) return;
    let active = true;
    const timer = window.setInterval(() => { api.request<PdfJob>(`/api/v1/licitaciones/${record.id}/pdf/trabajos/${job.id}`).then(value => { if (active) setJob(value); }).catch(reason => { if (active) setError(message(reason)); }); }, 3000);
    return () => { active = false; window.clearInterval(timer); };
  }, [api, job?.id, job?.state, record.id]);
  useEffect(() => { if (result.data) { const incoming = result.data.items, previous = baseline.current, old = original.current; original.current = incoming; baseline.current = signature(incoming); setItems(current => { if (signature(current) === previous) return incoming; return [...current.filter(e => incoming.some(next => next.key === e.key)).map(e => { const fresh = incoming.find(next => next.key === e.key)!; if (fresh.title_editable === false) return fresh; return e.label === old.find(previous => previous.key === e.key)?.label ? {...e,label:fresh.label,title_editable:fresh.title_editable} : {...e,title_editable:fresh.title_editable}; }), ...incoming.filter(e => !current.some(old => old.key === e.key))]; }); } }, [result.data]);
  const save = async () => { setBusy(true); setError(''); try { if (items.some(e => !e.label.trim() || e.label.trim().length > 1000 || /[\u0000-\u001f\u007f]/.test(e.label))) throw new Error('Cada título debe tener entre 1 y 1000 caracteres, sin saltos de línea.'); const updated = await api.request<Expedient>(`/api/v1/licitaciones/${record.id}/pdf/orden`, { method:'PUT', ifMatch:latest().version, body:{ items:items.map(e => e.key), titles:Object.fromEntries(items.filter(e => e.title_editable !== false).map(e => [e.key,e.label.trim()])) } }); baseline.current = signature(items); saved(updated); return true; } catch (reason) { setError(message(reason)); return false; } finally { setBusy(false); } };
  useUnsavedChanges(signature(items) !== baseline.current, busy, save, 50);
  const move = (from: number, to: number) => setItems(current => {
    if (from < 0 || to < 0 || from >= current.length || to >= current.length) return current;
    const next = [...current], [entry] = next.splice(from, 1); if (!entry) return current;
    next.splice(to, 0, entry); return next;
  });
  return <section className="stack"><p>Ordena los formularios, los lotes con ítems y los archivos del expediente. Edita el título que aparecerá en el índice. Arrastra cada elemento o usa las flechas para ordenar.</p><Notice>{error || result.error}</Notice>
    {!result.data && !result.error && <Loading />}
    {result.data && <><ol className="tender-order">{items.map((entry, index) => <li key={entry.key} draggable={editable && !busy} onDragStart={event => event.dataTransfer.setData('text/plain', entry.key)} onDragOver={event => { if (editable) event.preventDefault(); }} onDrop={event => { event.preventDefault(); if (editable) move(items.findIndex(x => x.key === event.dataTransfer.getData('text/plain')), index); }}><label>Título del documento<input aria-label={`Título del documento ${index + 1}`} value={entry.label} maxLength={1000} required disabled={!editable || busy || entry.title_editable === false} onChange={event => { const label = event.target.value; setItems(current => current.map(item => item.key === entry.key ? {...item,label} : item)); }} onDragStart={event => event.stopPropagation()} /></label>{editable && <span className="actions"><button type="button" disabled={busy || index === 0} aria-label={`Subir ${entry.label}`} onClick={() => move(index,index - 1)}>↑</button><button type="button" disabled={busy || index === items.length - 1} aria-label={`Bajar ${entry.label}`} onClick={() => move(index,index + 1)}>↓</button></span>}</li>)}</ol>{editable && <button className="primary" disabled={busy || signature(items) === baseline.current} onClick={() => { void save(); }}>{busy ? 'Guardando…' : 'Guardar orden y títulos'}</button>}</>}
    <div className="stack"><h3>Generar PDF final</h3>
      <p>El PDF incluye portada, índice, formularios, lotes ofertados y archivos en el orden guardado.</p>
      {job && <p role="status">{job.state === 'queued' ? 'En espera para generar.' : job.state === 'running' ? 'Generando el PDF…' : job.state === 'uploading' ? 'Guardando el PDF…' : job.state === 'failed' ? 'No se pudo generar el PDF. Revisa las variables, formularios y archivos e inténtalo de nuevo.' : job.state === 'expired' ? 'Este PDF ya no está disponible.' : job.expediente_version !== record.version ? 'El expediente cambió; genera un PDF actualizado.' : 'PDF listo para descargar.'}</p>}
      {job?.state === 'ready' && job.expediente_version === record.version && <a className="button" href={api.downloadHref(`/api/v1/licitaciones/${record.id}/pdf/trabajos/${job.id}/archivo`)} target="_blank" rel="noopener noreferrer">Descargar PDF</a>}
      {editable && <button type="button" className="primary" disabled={busy || generating || !jobsLoaded || !result.data || signature(items) !== baseline.current || Boolean(job && ['queued','running','uploading'].includes(job.state))} onClick={async () => { setGenerating(true); setError(''); try { const value = await api.request<PdfJob>(`/api/v1/licitaciones/${record.id}/pdf/generar`, { method:'POST', ifMatch:record.version, body:{} }); setJob(value); } catch (reason) { setError(message(reason)); } finally { setGenerating(false); } }}>{generating ? 'Preparando…' : 'Generar PDF'}</button>}
      {editable && signature(items) !== baseline.current && <p>Guarda el orden y los títulos antes de generar el PDF.</p>}
    </div>
  </section>;
}
