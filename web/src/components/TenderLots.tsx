import { useEffect, useRef, useState } from 'react';
import { message } from '../api.js';
import { Loading, Notice, useCore, useLoad } from '../ui.js';
import { useUnsavedChanges } from '../navigationGuard.js';
import type { Expedient } from '../tenders.js';

type Item = { descripcion: string; pais_origen: string; fecha_entrega: string; cantidad: string; unidad: string; transporte: string; precio_unitario: string };
type Lot = { numero: number; items: Item[]; total: string; version: number };
const emptyItem = (): Item => ({ descripcion: '', pais_origen: '', fecha_entrega: '', cantidad: '', unidad: '', transporte: '0.00', precio_unitario: '' });

export default function TenderLots({ record, latest = () => record, onDraftInfo, editable, saved }: { record: Expedient; latest?: () => Expedient; onDraftInfo?: (value: {numero:number; items:number}[]) => void; editable: boolean; saved: (value: Expedient) => void }) {
  const { api } = useCore();
  const count = Number(record.variables?.find(v => v.key === 'lotes')?.value ?? 0);
  const style = record.variables?.find(v => v.key === 'stylo')?.value;
  const [number, setNumber] = useState(1), [drafts, setDrafts] = useState<Record<number, Item[]>>({}), [busy, setBusy] = useState(false), [error, setError] = useState('');
  const baseline = useRef<Record<number, string>>({});
  const draftsRef = useRef(drafts); draftsRef.current = drafts;
  const items = drafts[number] ?? [];
  const setItems = (update: Item[] | ((current: Item[]) => Item[])) => setDrafts(current => ({ ...current, [number]: typeof update === 'function' ? update(current[number] ?? []) : update }));
  const valid = Number.isSafeInteger(count) && count > 0 && count <= 1000 && (style === 'bid' || style === 'std');
  const savedCount = Number(latest().variables?.find(v => v.key === 'lotes')?.value ?? 0);
  const savedStyle = latest().variables?.find(v => v.key === 'stylo')?.value;
  const configChanged = count !== savedCount || style !== savedStyle;
  const result = useLoad(signal => valid && number <= savedCount ? api.request<Lot>(`/api/v1/licitaciones/${record.id}/lotes/${number}`, { signal }) : Promise.resolve(null), [record.id, number, record.version, valid, savedCount]);
  useEffect(() => { if (result.data) { const lot = result.data; const previous = baseline.current[lot.numero]; if (previous === undefined) { baseline.current[lot.numero] = JSON.stringify(lot.items); if (draftsRef.current[lot.numero] === undefined) setDrafts(current => ({ ...current, [lot.numero]: structuredClone(lot.items) })); } else if (JSON.stringify(draftsRef.current[lot.numero] ?? []) === previous) { baseline.current[lot.numero] = JSON.stringify(lot.items); setDrafts(current => ({ ...current, [lot.numero]: structuredClone(lot.items) })); } } }, [result.data]);
  useEffect(() => { if (Number.isSafeInteger(count) && count > 0 && number > count) setNumber(1); }, [count, number]);
  const dirty = Object.entries(drafts).some(([n, value]) => JSON.stringify(value) !== (baseline.current[Number(n)] ?? '[]'));
  useEffect(() => { onDraftInfo?.(Object.entries(drafts).filter(([n, value]) => JSON.stringify(value) !== (baseline.current[Number(n)] ?? '[]')).map(([n, value]) => ({ numero:Number(n), items:value.length }))); }, [drafts, onDraftInfo]);
  const save = async () => { setBusy(true); setError(''); try { const current = latest(); const maximum = Number(current.variables?.find(v => v.key === 'lotes')?.value ?? 0), currentStyle = current.variables?.find(v => v.key === 'stylo')?.value; const kept: Record<number, Item[]> = {}; for (const [n, value] of Object.entries(drafts)) { if (Number(n) > maximum) { delete baseline.current[Number(n)]; continue; } const normalized = currentStyle === 'std' ? value.map(item => ({ ...item, pais_origen:'', transporte:'0.00' })) : value; kept[Number(n)] = normalized; if (JSON.stringify(normalized) === (baseline.current[Number(n)] ?? '[]')) continue; const updated = await api.request<Expedient>(`/api/v1/licitaciones/${record.id}/lotes/${n}`, { method:'PUT', ifMatch:latest().version, body:{ items:normalized } }); baseline.current[Number(n)] = JSON.stringify(normalized); saved(updated); } setDrafts(kept); return true; } catch (reason) { setError(message(reason)); return false; } finally { setBusy(false); } };
  useUnsavedChanges(dirty, busy, save, 40);
  if (!valid) return <p>Define la cantidad de lotes y el estilo en la pestaña Variables para preparar la oferta.</p>;
  const change = (index: number, key: keyof Item, value: string) => setItems(current => current.map((item, i) => i === index ? { ...item, [key]: value } : item));
  return <section className="quote-section"><div className="quote-section-heading"><h3>Oferta por lote</h3><label>Lote<select value={number} disabled={busy} onChange={event => setNumber(Number(event.target.value))}>{Array.from({ length: count }, (_, i) => <option value={i + 1} key={i + 1}>Lote {i + 1}</option>)}</select></label></div>
    <Notice>{error || result.error}</Notice>{number <= savedCount && result.data?.numero !== number && !result.error && <Loading />}
    {(result.data?.numero === number || number > savedCount) && <form onSubmit={event => { event.preventDefault(); if (editable && !busy && !configChanged) void save(); }}>
      <fieldset disabled={busy || !editable}><div className="quote-items-wrap"><table className="quote-items-table"><thead><tr><th>Ítem</th><th>Descripción</th>{style === 'bid' && <th>País de origen</th>}<th>Entrega</th><th>Cantidad</th><th>Unidad</th>{style === 'bid' && <th>Transporte</th>}<th>Precio unitario</th><th>Acción</th></tr></thead><tbody>{items.map((item, index) => <tr key={index}><td>{index + 1}</td><td><textarea required value={item.descripcion} onChange={e => change(index,'descripcion',e.target.value)} /></td>{style === 'bid' && <td><input required value={item.pais_origen} onChange={e => change(index,'pais_origen',e.target.value)} /></td>}<td><input required value={item.fecha_entrega} onChange={e => change(index,'fecha_entrega',e.target.value)} /></td><td><input required inputMode="decimal" value={item.cantidad} onChange={e => change(index,'cantidad',e.target.value)} /></td><td><input required value={item.unidad} onChange={e => change(index,'unidad',e.target.value)} /></td>{style === 'bid' && <td><input required inputMode="decimal" value={item.transporte} onChange={e => change(index,'transporte',e.target.value)} /></td>}<td><input required inputMode="decimal" value={item.precio_unitario} onChange={e => change(index,'precio_unitario',e.target.value)} /></td><td><button type="button" onClick={() => setItems(current => current.filter((_, i) => i !== index))}>Quitar</button></td></tr>)}</tbody></table></div>
      <div className="actions"><button type="button" onClick={() => setItems(current => [...current,emptyItem()])}>Añadir ítem</button><button className="primary" type="submit" disabled={!dirty || configChanged}>{busy ? 'Guardando…' : 'Guardar lote'}</button></div>{configChanged && <p>Usa «Guardar expediente» para guardar el estilo, la cantidad de lotes y los ítems juntos.</p>}<p>Total guardado del lote: L {result.data?.numero === number ? result.data.total : '0.00'} · ISV 15 %</p></fieldset></form>}
  </section>;
}
