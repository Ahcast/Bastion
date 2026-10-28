import { useEffect, useRef, useState } from 'react';
import { message } from '../api.js';
import { Loading, Notice, useCore, useLoad } from '../ui.js';
import { useUnsavedChanges } from '../navigationGuard.js';
import type { Expedient } from '../tenders.js';

type Entry = { key: string; label: string };
type Order = { items: Entry[]; version: number };
export default function TenderPdfOrder({ record, editable, saved }: { record: Expedient; editable: boolean; saved: (value: Expedient) => void }) {
  const { api } = useCore();
  const result = useLoad(signal => api.request<Order>(`/api/v1/licitaciones/${record.id}/pdf/orden`, { signal }), [record.id, record.version]);
  const [items, setItems] = useState<Entry[]>([]), [busy, setBusy] = useState(false), [error, setError] = useState('');
  const baseline = useRef('[]');
  useEffect(() => { if (result.data) { setItems(result.data.items); baseline.current = JSON.stringify(result.data.items.map(e => e.key)); } }, [result.data]);
  useUnsavedChanges(JSON.stringify(items.map(e => e.key)) !== baseline.current, busy);
  const move = (from: number, to: number) => setItems(current => {
    if (from < 0 || to < 0 || from >= current.length || to >= current.length) return current;
    const next = [...current], [entry] = next.splice(from, 1); if (!entry) return current;
    next.splice(to, 0, entry); return next;
  });
  return <section className="stack"><p>Ordena los formularios, los lotes con ítems y los archivos del expediente. Arrastra cada elemento o usa las flechas.</p><Notice>{error || result.error}</Notice>
    {!result.data && !result.error && <Loading />}
    {result.data && <><ol className="tender-order">{items.map((entry, index) => <li key={entry.key} draggable={editable && !busy} onDragStart={event => event.dataTransfer.setData('text/plain', entry.key)} onDragOver={event => { if (editable) event.preventDefault(); }} onDrop={event => { event.preventDefault(); if (editable) move(items.findIndex(x => x.key === event.dataTransfer.getData('text/plain')), index); }}><span>{entry.label}</span>{editable && <span className="actions"><button type="button" disabled={busy || index === 0} aria-label={`Subir ${entry.label}`} onClick={() => move(index,index - 1)}>↑</button><button type="button" disabled={busy || index === items.length - 1} aria-label={`Bajar ${entry.label}`} onClick={() => move(index,index + 1)}>↓</button></span>}</li>)}</ol>{editable && <button className="primary" disabled={busy || JSON.stringify(items.map(e => e.key)) === baseline.current} onClick={async () => { setBusy(true); setError(''); try { const updated = await api.request<Expedient>(`/api/v1/licitaciones/${record.id}/pdf/orden`, { method:'PUT', ifMatch:record.version, body:{ items:items.map(e => e.key) } }); baseline.current = JSON.stringify(items.map(e => e.key)); saved(updated); } catch (reason) { setError(message(reason)); } finally { setBusy(false); } }}>{busy ? 'Guardando…' : 'Guardar orden'}</button>}</>}
    <p>La generación y el foliado del PDF final están en desarrollo.</p>
  </section>;
}
