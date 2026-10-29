import { useEffect, useRef, useState } from 'react';
import { message } from '../api.js';
import { Loading, Notice, useCore, useLoad } from '../ui.js';
import { useUnsavedChanges } from '../navigationGuard.js';
import type { Expedient } from '../tenders.js';

type Item = { descripcion: string; pais_origen: string; fecha_entrega: string; cantidad: string; unidad: string; transporte: string; precio_unitario: string };
type Lot = { numero: number; items: Item[]; total: string; version: number };
const emptyItem = (): Item => ({ descripcion: '', pais_origen: '', fecha_entrega: '', cantidad: '', unidad: '', transporte: '0.00', precio_unitario: '' });

export default function TenderLots({ record, editable, saved }: { record: Expedient; editable: boolean; saved: (value: Expedient) => void }) {
  const { api } = useCore();
  const count = Number(record.variables?.find(v => v.key === 'lotes')?.value ?? 0);
  const style = record.variables?.find(v => v.key === 'stylo')?.value;
  const [number, setNumber] = useState(1), [items, setItems] = useState<Item[]>([]), [busy, setBusy] = useState(false), [error, setError] = useState('');
  const baseline = useRef('[]');
  const valid = Number.isSafeInteger(count) && count > 0 && count <= 1000 && (style === 'bid' || style === 'std');
  const result = useLoad(signal => valid ? api.request<Lot>(`/api/v1/licitaciones/${record.id}/lotes/${number}`, { signal }) : Promise.resolve(null), [record.id, number, record.version, valid]);
  useEffect(() => { if (result.data) { setItems(structuredClone(result.data.items)); baseline.current = JSON.stringify(result.data.items); } }, [result.data]);
  useEffect(() => { if (Number.isSafeInteger(count) && count > 0 && number > count) setNumber(1); }, [count, number]);
  useUnsavedChanges(JSON.stringify(items) !== baseline.current, busy);
  if (!valid) return <p>Define la cantidad de lotes y el estilo en la pestaña Variables para preparar la oferta.</p>;
  const change = (index: number, key: keyof Item, value: string) => setItems(current => current.map((item, i) => i === index ? { ...item, [key]: value } : item));
  return <section className="quote-section"><div className="quote-section-heading"><h3>Oferta por lote</h3><label>Lote<select value={number} disabled={busy || JSON.stringify(items) !== baseline.current} onChange={event => { setItems([]); baseline.current = '[]'; setNumber(Number(event.target.value)); }}>{Array.from({ length: count }, (_, i) => <option value={i + 1} key={i + 1}>Lote {i + 1}</option>)}</select></label></div>
    <Notice>{error || result.error}</Notice>{!result.data && !result.error && <Loading />}
    {result.data && <form onSubmit={async event => { event.preventDefault(); if (!editable || busy) return; setBusy(true); setError(''); try { const updated = await api.request<Expedient>(`/api/v1/licitaciones/${record.id}/lotes/${number}`, { method:'PUT', ifMatch:record.version, body:{ items } }); baseline.current=JSON.stringify(items); saved(updated); } catch (reason) { setError(message(reason)); } finally { setBusy(false); } }}>
      <fieldset disabled={busy || !editable}><div className="quote-items-wrap"><table className="quote-items-table"><thead><tr><th>Ítem</th><th>Descripción</th>{style === 'bid' && <th>País de origen</th>}<th>Entrega</th><th>Cantidad</th><th>Unidad</th>{style === 'bid' && <th>Transporte</th>}<th>Precio unitario</th><th>Acción</th></tr></thead><tbody>{items.map((item, index) => <tr key={index}><td>{index + 1}</td><td><textarea required value={item.descripcion} onChange={e => change(index,'descripcion',e.target.value)} /></td>{style === 'bid' && <td><input required value={item.pais_origen} onChange={e => change(index,'pais_origen',e.target.value)} /></td>}<td><input required value={item.fecha_entrega} onChange={e => change(index,'fecha_entrega',e.target.value)} /></td><td><input required inputMode="decimal" value={item.cantidad} onChange={e => change(index,'cantidad',e.target.value)} /></td><td><input required value={item.unidad} onChange={e => change(index,'unidad',e.target.value)} /></td>{style === 'bid' && <td><input required inputMode="decimal" value={item.transporte} onChange={e => change(index,'transporte',e.target.value)} /></td>}<td><input required inputMode="decimal" value={item.precio_unitario} onChange={e => change(index,'precio_unitario',e.target.value)} /></td><td><button type="button" onClick={() => setItems(current => current.filter((_, i) => i !== index))}>Quitar</button></td></tr>)}</tbody></table></div>
      <div className="actions"><button type="button" onClick={() => setItems(current => [...current,emptyItem()])}>Añadir ítem</button><button className="primary" type="submit" disabled={JSON.stringify(items) === baseline.current}>{busy ? 'Guardando…' : 'Guardar lote'}</button></div><p>Total guardado del lote: L {result.data.total} · ISV 15 %</p></fieldset></form>}
  </section>;
}
