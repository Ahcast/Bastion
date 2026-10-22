import { useState } from 'react';
import { processItems } from '../process-items.js';
import '../styles/procesos.css';
import { message } from '../api.js';
import { Dialog, Form, Heading, Loading, Notice, Table, useCore, useLoad } from '../ui.js';
import { moduleCan } from '../types.js';

interface ProcessRecord {
  marked: boolean; note: string; version: number;
  proceso_id: string; entidad: string; expediente: string; fecha_recepcion: string;
  puntaje: number; rubro: string; resumen_corto: string; source: Record<string, string>;
}
interface ProcessPage { items: ProcessRecord[]; page: number; page_size: number; has_more: boolean }
interface Status { total: number; last_import_at: string | null }
interface ImportResult { inserted: number; updated: number; unchanged: number; stale: number; replayed: boolean }
const date = (value: string) => new Intl.DateTimeFormat('es-HN', { timeZone: 'America/Tegucigalpa', dateStyle: 'medium', timeStyle: 'short' }).format(new Date(value));

export default function Procesos() {
  const { api, access, notify } = useCore();
  const [filters, setFilters] = useState(''), [page, setPage] = useState(1), [importing, setImporting] = useState(false), [busy, setBusy] = useState(false), [selected, setSelected] = useState<string | null>(null);
  const view = moduleCan(access, 'procesos', 'consulta', 'ver');
  const active = access.modules.some(module => module.module_id === 'procesos');
  const result = useLoad(signal => view ? api.request<ProcessPage>(`/api/v1/procesos/rows?${filters}&page=${page}&page_size=25`, { signal }) : Promise.resolve<ProcessPage>({ items: [], page, page_size: 25, has_more: false }), [view, filters, page]);
  const status = useLoad(signal => view ? api.request<Status>('/api/v1/procesos/status', { signal }) : Promise.resolve<Status>({ total: 0, last_import_at: null }), [view]);
  const reload = () => { result.reload(); status.reload(); };
  if (!active) return <Notice>Procesos no está habilitado. Actualiza la sesión para consultar los módulos disponibles.</Notice>;
  return <>
    <Heading title="Procesos" description="Oportunidades analizadas por bastion-scout." action={<div className="actions"><button onClick={reload}>Actualizar</button>{moduleCan(access, 'procesos', 'importacion', 'editar') && <button className="primary" onClick={() => setImporting(true)}>Importar CSV</button>}{moduleCan(access, 'procesos', 'exportacion', 'ver') && <button disabled={busy} onClick={async () => {
      setBusy(true);
      try {
        const data = await api.request<{ csv_text: string }>(`/api/v1/procesos/export?${filters}`);
        const url = URL.createObjectURL(new Blob([data.csv_text], { type: 'text/csv;charset=utf-8' }));
        const link = document.createElement('a'); link.href = url; link.download = 'procesos.csv'; document.body.append(link); link.click(); link.remove(); setTimeout(() => URL.revokeObjectURL(url), 10_000);
      } catch (error) { notify(message(error)); } finally { setBusy(false); }
    }}>Exportar CSV</button>}</div>} />
    {view && <><section className="card"><p>{status.data ? `${status.data.total} procesos · Última importación: ${status.data.last_import_at ? date(status.data.last_import_at) : 'Sin importaciones'}` : 'Consultando estado…'}</p><Notice>{status.error}</Notice><Form fields={[{ name: 'q', label: 'Buscar expediente, resumen o descripción', maxLength: 200 }, { name: 'entidad', label: 'Entidad', maxLength: 200 }, { name: 'desde', label: 'Recepción desde', type: 'date' }, { name: 'hasta', label: 'Recepción hasta', type: 'date' }, { name: 'marked', label: 'De interés', options: [{value:'',label:'Todos'},{value:'true',label:'Solo marcados'},{value:'false',label:'Sin marcar'}] }]} submit="Buscar" onSubmit={async values => { const query = new URLSearchParams(); for (const [key, value] of Object.entries(values)) if (value) query.set(key, value); setFilters(query.toString()); setPage(1); }} /></section>
    <Notice>{result.error}</Notice>{result.data ? <><Table columns={['Expediente', 'Entidad', 'Recepción de ofertas', 'Puntaje IA', 'Rubro', 'Resumen', 'De interés', 'Nota compartida', 'Acciones']} rows={result.data.items.map(row => [row.expediente, row.entidad, date(row.fecha_recepcion), `${row.puntaje}/100`, row.rubro, row.resumen_corto, <input type="checkbox" aria-label={'De interés: '+row.expediente} checked={row.marked} disabled={busy || !moduleCan(access,'procesos','marcas','editar')} onChange={async event => { const marked=event.currentTarget.checked; setBusy(true); try { await api.request('/api/v1/procesos/rows/'+encodeURIComponent(row.proceso_id)+'/review', {method:'PATCH',body:{marked},ifMatch:row.version}); } catch(error) {notify(message(error));} finally {setBusy(false);result.reload();} }} />, row.note || '—', <button onClick={() => setSelected(row.proceso_id)}>Ver todo el proceso</button>])} /><div className="row"><button disabled={page === 1} onClick={() => setPage(page - 1)}>Anterior</button><span>Página {page}</span><button disabled={!result.data.has_more} onClick={() => setPage(page + 1)}>Siguiente</button></div></> : !result.error && <Loading />}</>}
    {importing && <Dialog title="Importar CSV de bastion-scout" close={() => { if (!busy) setImporting(false); }}><p>Selecciona el maestro.csv ya analizado por IA. Hasta 8 MiB y 5.000 filas. Las filas ausentes conservan su histórico.</p><form className="stack" onSubmit={async event => {
      event.preventDefault(); if (busy) return;
      const file = new FormData(event.currentTarget).get('csv');
      if (!(file instanceof File) || !file.size || file.size > 8 * 1024 * 1024) { notify('Selecciona un CSV de hasta 8 MiB.'); return; }
      setBusy(true);
      try {
        const text = new TextDecoder('utf-8', { fatal: true, ignoreBOM: true }).decode(await file.arrayBuffer());
        const imported = await api.request<ImportResult>('/api/v1/procesos/import', { method: 'POST', body: { csv_text: text } });
        notify(`${imported.replayed ? 'Archivo ya importado. ' : ''}${imported.inserted} nuevos · ${imported.updated} actualizados · ${imported.unchanged} sin cambios · ${imported.stale} antiguos omitidos.`);
        setImporting(false); reload();
      } catch (error) { notify(message(error)); } finally { setBusy(false); }
    }}><label>Archivo CSV<input type="file" name="csv" accept=".csv,text/csv" required disabled={busy} /></label><button className="primary" disabled={busy}>{busy ? 'Importando…' : 'Importar CSV'}</button></form></Dialog>}
    {selected && <Detail id={selected} close={() => setSelected(null)} saved={reload} />}
  </>;
}

function Detail({ id, close, saved }: { id: string; close: () => void; saved: () => void }) {
  const { api, access, notify } = useCore(), result = useLoad(signal => api.request<ProcessRecord>(`/api/v1/procesos/rows/${encodeURIComponent(id)}`, { signal }), [id]);
  const row = result.data;
  return <Dialog title={row?.expediente ?? 'Detalle del proceso'} close={close}><Notice>{result.error}</Notice><button onClick={result.reload}>Recargar proceso</button>{row ? <><p>{row.entidad} · {date(row.fecha_recepcion)}</p><h3>{row.puntaje}/100 · {row.rubro}</h3><p className="process-text">{row.resumen_corto}</p><h3>Ítems analizados</h3><ul>{processItems(row.source.items ?? '').map((item, index) => <li key={index}>{item}</li>)}</ul><h3>Nota compartida</h3>{moduleCan(access,'procesos','notas','editar') ? <Form key={row.version} fields={[{name:'note',label:'Nota para todos los usuarios',multiline:true,maxLength:10000}]} initial={{note:row.note}} submit="Guardar nota" onSubmit={async values => { await api.request('/api/v1/procesos/rows/'+encodeURIComponent(id)+'/review',{method:'PATCH',body:{note:values.note ?? ''},ifMatch:row.version}); notify('Nota compartida guardada.'); result.reload(); saved(); }} /> : <p className="process-text">{row.note || 'Sin nota.'}</p>}<h3>Descripción fuente</h3><p className="process-text">{row.source.descripcion}</p><a href={row.source.url} target="_blank" rel="noopener noreferrer">Abrir en HonduCompras</a><details><summary>Datos de extracción y análisis</summary><dl>{Object.entries(row.source).map(([key, value]) => <div key={key}><dt>{key}</dt><dd className="process-text">{value}</dd></div>)}</dl></details></> : !result.error && <Loading />}</Dialog>;
}
