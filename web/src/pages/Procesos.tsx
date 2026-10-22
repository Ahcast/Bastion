import { useEffect, useState } from 'react';
import '../styles/procesos.css';
import { message } from '../api.js';
import { Form, Heading, Loading, Notice, useCore, useLoad } from '../ui.js';
import { moduleCan } from '../types.js';
import { initialProcessFilters, nextProcessExpiry, processDate, processQuery } from '../processes.js';
import type { ProcessFilters, ProcessPage, ProcessRecord, ProcessStatus } from '../processes.js';
import { ProcessDetailContent, ProcessFilterBar, ProcessInsights, ProcessTable } from '../components/ProcessWorkspace.js';

interface ImportResult { inserted: number; updated: number; unchanged: number; stale: number; replayed: boolean }

export default function Procesos() {
  const { api, access, notify } = useCore();
  const [filters, setFilters] = useState<ProcessFilters>(initialProcessFilters), [search, setSearch] = useState('');
  const [page, setPage] = useState(1), [refreshing, setRefreshing] = useState(false), [busy, setBusy] = useState(false), [selected, setSelected] = useState<string | null>(null);
  useEffect(() => { const timer = setTimeout(() => setSearch(filters.q), 350); return () => clearTimeout(timer); }, [filters.q]);
  const query = processQuery({ ...filters, q: search });
  const view = moduleCan(access, 'procesos', 'consulta', 'ver');
  const active = access.modules.some(module => module.module_id === 'procesos');
  const result = useLoad(async signal => {
    const requestedAt = performance.now();
    const data = view ? await api.request<ProcessPage>(`/api/v1/procesos/rows?${query}&page=${page}&page_size=25`, { signal }) : { items: [], total: 0, server_time: new Date().toISOString(), page, page_size: 25, has_more: false };
    return { ...data, requestedAt };
  }, [view, query, page]);
  const status = useLoad(signal => view ? api.request<ProcessStatus>('/api/v1/procesos/status', { signal }) : Promise.resolve(null), [view]);
  const entities = useLoad(signal => view ? api.request<{ entities: string[] }>('/api/v1/procesos/entities', { signal }) : Promise.resolve({ entities: [] }), [view]);
  const reload = () => { result.reload(); };
  useEffect(() => {
    if (!result.data || !view) return;
    const refresh = () => { setSelected(null); result.reload(); };
    const delay = nextProcessExpiry(result.data, performance.now() - result.data.requestedAt);
    const timer = delay === null ? undefined : setTimeout(refresh, delay);
    const resume = () => { if (document.visibilityState === 'visible') refresh(); };
    document.addEventListener('visibilitychange', resume);
    return () => { clearTimeout(timer); document.removeEventListener('visibilitychange', resume); };
  }, [result.data, view]);
  const changeFilters = (values: ProcessFilters) => { setFilters(values); setPage(1); setSelected(null); };
  if (!active) return <Notice>Procesos no está habilitado. Actualiza la sesión para consultar los módulos disponibles.</Notice>;
  return <div className="processes">
    <Heading title="Procesos" description="Oportunidades analizadas por bastion-scout." action={<div className="actions"><button disabled={busy || !moduleCan(access, 'procesos', 'importacion', 'editar')} onClick={async () => {
      if (busy) return;
      setBusy(true); setRefreshing(true);
      try {
        const imported = await api.request<ImportResult>('/api/v1/procesos/refresh', { method: 'POST', body: {} });
        notify(imported.replayed ? 'Los resultados de Scout ya estaban cargados.' : `${imported.inserted} nuevos · ${imported.updated} actualizados · ${imported.unchanged} sin cambios · ${imported.stale} antiguos omitidos.`);
        setPage(1); reload(); status.reload(); entities.reload();
      } catch (error) { notify(message(error)); } finally { setBusy(false); setRefreshing(false); }
    }}>{refreshing ? 'Actualizando…' : 'Actualizar'}</button>{moduleCan(access, 'procesos', 'exportacion', 'ver') && <button disabled={busy} onClick={async () => {
      setBusy(true);
      try {
        const data = await api.request<{ csv_text: string }>(`/api/v1/procesos/export?${query}`);
        const url = URL.createObjectURL(new Blob([data.csv_text], { type: 'text/csv;charset=utf-8' }));
        const link = document.createElement('a'); link.href = url; link.download = 'procesos.csv'; document.body.append(link); link.click(); link.remove(); setTimeout(() => URL.revokeObjectURL(url), 10_000);
      } catch (error) { notify(message(error)); } finally { setBusy(false); }
    }}>Exportar CSV</button>}</div>} />
    {view && <>
      <ProcessInsights status={status.data} /><Notice>{status.error}</Notice>
      <section className="process-workspace" aria-label="Consulta de procesos"><div className="process-list-heading"><h2>Explorar procesos vigentes</h2><p>{result.data ? `${result.data.total} resultados` : 'Consultando…'}<span> · </span>Última importación: {status.data?.last_import_at ? processDate(status.data.last_import_at) : '—'}</p></div>
        <ProcessFilterBar values={filters} entities={entities.data?.entities ?? []} change={changeFilters} /><Notice>{entities.error}</Notice>
        <Notice>{result.error}</Notice>{result.data ? <><ProcessTable rows={result.data.items} selected={selected} select={setSelected} canMark={moduleCan(access, 'procesos', 'marcas', 'editar')} busy={busy} mark={async (row, marked) => {
          setBusy(true);
          try { await api.request(`/api/v1/procesos/rows/${encodeURIComponent(row.proceso_id)}/review`, { method: 'PATCH', body: { marked }, ifMatch: row.version }); }
          catch (error) { notify(message(error)); } finally { setBusy(false); reload(); }
        }} detail={row => <Detail key={row.proceso_id} id={row.proceso_id} saved={reload} />} />
          <div className="process-pagination"><p>Abre una fila para ver el detalle aquí mismo.</p><div><button disabled={page === 1} onClick={() => { setPage(page - 1); setSelected(null); }}>Anterior</button><span>Página {page}</span><button disabled={!result.data.has_more} onClick={() => { setPage(page + 1); setSelected(null); }}>Siguiente</button></div></div></> : !result.error && <Loading />}
      </section>
    </>}
  </div>;
}

function Detail({ id, saved }: { id: string; saved: () => void }) {
  const { api, access, notify } = useCore(), result = useLoad(signal => api.request<ProcessRecord>(`/api/v1/procesos/rows/${encodeURIComponent(id)}`, { signal }), [id]);
  const row = result.data;
  return <><Notice>{result.error}</Notice>{result.error && <button onClick={result.reload}>Reintentar detalle</button>}{row ? <ProcessDetailContent row={row} noteEditor={moduleCan(access, 'procesos', 'notas', 'editar') ? <Form key={row.version} fields={[{ name: 'note', label: 'Nota para todos los usuarios', multiline: true, maxLength: 10000 }]} initial={{ note: row.note }} submit="Guardar nota" onSubmit={async values => {
    await api.request(`/api/v1/procesos/rows/${encodeURIComponent(id)}/review`, { method: 'PATCH', body: { note: values.note ?? '' }, ifMatch: row.version }); notify('Nota compartida guardada.'); saved();
  }} /> : undefined} /> : !result.error && <Loading />}</>;
}
