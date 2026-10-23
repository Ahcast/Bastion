import { useEffect, useState } from 'react';
import '../styles/procesos.css';
import { ApiError, message } from '../api.js';
import { Form, Heading, Loading, Notice, useCore, useLoad } from '../ui.js';
import { moduleCan } from '../types.js';
import { initialProcessFilters, nextProcessExpiry, processDate, processQuery } from '../processes.js';
import type { ProcessFilters, ProcessPage, ProcessRecord, ProcessReview, ProcessStatus } from '../processes.js';
import { ProcessDetailContent, ProcessFilterBar, ProcessInsights, ProcessTable } from '../components/ProcessWorkspace.js';

interface ImportResult { inserted: number; updated: number; unchanged: number; stale: number; replayed: boolean; deferred?: number }

export default function Procesos() {
  const { api, access, notify } = useCore();
  const [filters, setFilters] = useState<ProcessFilters>(initialProcessFilters), [search, setSearch] = useState('');
  const [page, setPage] = useState(1), [refreshing, setRefreshing] = useState(false), [busy, setBusy] = useState(false), [selected, setSelected] = useState<string | null>(null);
  // Keep confirmed local reviews over an older response already in flight.
  const [reviews, setReviews] = useState<Record<string, ProcessReview>>({});
  const reviewed = (row: ProcessRecord): ProcessRecord => { const review = reviews[row.proceso_id]; return review && review.version >= row.version ? { ...row, ...review } : row; };
  const saved = (id: string, review: ProcessReview) => setReviews(current => (current[id]?.version ?? -1) > review.version ? current : { ...current, [id]: { marked: review.marked, note: review.note, version: review.version } });
  useEffect(() => { const timer = setTimeout(() => setSearch(filters.q), 350); return () => clearTimeout(timer); }, [filters.q]);
  const query = processQuery({ ...filters, q: search });
  const view = moduleCan(access, 'procesos', 'consulta', 'ver');
  const active = access.modules.some(module => module.module_id === 'procesos');
  const result = useLoad(async signal => {
    const requestedAt = performance.now();
    const data = view ? await api.request<ProcessPage>(`/api/v1/procesos/rows?${query}&page=${page}&page_size=25`, { signal }) : { items: [], total: 0, server_time: new Date().toISOString(), page, page_size: 25, has_more: false };
    return { ...data, requestedAt };
  }, [view, query, page], true);
  const status = useLoad(signal => view ? api.request<ProcessStatus>('/api/v1/procesos/status', { signal }) : Promise.resolve(null), [view]);
  const entities = useLoad(signal => view ? api.request<{ entities: string[] }>('/api/v1/procesos/entities', { signal }) : Promise.resolve({ entities: [] }), [view]);
  const reload = () => { result.reload(); };
  useEffect(() => {
    if (!result.data || !view) return;
    let requested = false;
    const refresh = () => {
      if (requested) return;
      requested = true;
      // Remove only rows that expired; keep the rest of the table mounted.
      const now = Date.parse(result.data!.server_time) + Math.max(0, performance.now() - result.data!.requestedAt);
      result.setData(current => {
        if (!current) return current;
        const items = current.items.filter(row => Date.parse(row.fecha_recepcion) > now);
        return { ...current, items, total: current.total - (current.items.length - items.length) };
      });
      result.reload();
    };
    const delay = nextProcessExpiry(result.data, performance.now() - result.data.requestedAt);
    const timer = delay === null ? undefined : setTimeout(refresh, delay);
    const resume = () => { if (document.visibilityState === 'visible' && nextProcessExpiry(result.data!, performance.now() - result.data!.requestedAt) === 0) refresh(); };
    document.addEventListener('visibilitychange', resume);
    return () => { clearTimeout(timer); document.removeEventListener('visibilitychange', resume); };
  }, [result.data, view]);
  const updateReview = async (row: ProcessRecord, changes: Partial<Pick<ProcessReview, 'marked' | 'note'>>) => {
    const path = `/api/v1/procesos/rows/${encodeURIComponent(row.proceso_id)}`;
    try {
      const review = await api.request<ProcessReview>(`${path}/review`, { method: 'PATCH', body: changes, ifMatch: row.version });
      saved(row.proceso_id, review);
      if (filters.marked && !review.marked) {
        result.setData(current => current ? { ...current, items: current.items.filter(item => item.proceso_id !== row.proceso_id), total: Math.max(0, current.total - 1) } : current);
        result.reload();
      }
    } catch (error) {
      if (error instanceof ApiError && error.status === 412) {
        // Refresh only the conflicting review and retain any note draft.
        try { saved(row.proceso_id, await api.request<ProcessRecord>(path)); } catch { /* Keep the original conflict visible. */ }
      }
      throw error;
    }
  };
  const changeFilters = (values: ProcessFilters) => { setFilters(values); setPage(1); setSelected(null); };
  if (!active) return <Notice>Procesos no está habilitado. Actualiza la sesión para consultar los módulos disponibles.</Notice>;
  return <div className="processes">
    <Heading title="Procesos" description="Oportunidades analizadas por bastion-scout." action={<div className="actions"><button disabled={busy || !moduleCan(access, 'procesos', 'importacion', 'editar')} onClick={async () => {
      if (busy) return;
      setBusy(true); setRefreshing(true);
      try {
        const imported = await api.request<ImportResult>('/api/v1/procesos/refresh', { method: 'POST', body: {} });
        const summary = imported.replayed ? 'Los procesos listos de Scout ya estaban cargados.' : `${imported.inserted} nuevos · ${imported.updated} actualizados · ${imported.unchanged} sin cambios · ${imported.stale} antiguos omitidos.`;
        notify(summary + (imported.deferred ? ` ${imported.deferred} pendientes de completar extracción o análisis.` : ''));
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
        <Notice>{result.error}</Notice>{result.error && <button onClick={reload}>Reintentar consulta</button>}{result.data ? <><ProcessTable rows={result.data.items.map(reviewed)} selected={selected} select={setSelected} canMark={moduleCan(access, 'procesos', 'marcas', 'editar')} busy={busy} mark={async (row, marked) => {
          setBusy(true);
          try { await updateReview(row, { marked }); }
          catch (error) { notify(message(error)); } finally { setBusy(false); }
        }} detail={row => <Detail key={row.proceso_id} summary={row} synchronize={saved} save={updateReview} />} />
          <div className="process-pagination"><p>Abre una fila para ver el detalle aquí mismo.</p><div><button disabled={page === 1} onClick={() => { setPage(page - 1); setSelected(null); }}>Anterior</button><span>Página {page}</span><button disabled={!result.data.has_more} onClick={() => { setPage(page + 1); setSelected(null); }}>Siguiente</button></div></div></> : !result.error && <Loading />}
      </section>
    </>}
  </div>;
}

function Detail({ summary, synchronize, save }: { summary: ProcessRecord; synchronize: (id: string, review: ProcessReview) => void; save: (row: ProcessRecord, changes: { note: string }) => Promise<void> }) {
  const id = summary.proceso_id;
  const { api, access, notify } = useCore(), result = useLoad(signal => api.request<ProcessRecord>(`/api/v1/procesos/rows/${encodeURIComponent(id)}`, { signal }), [id]);
  useEffect(() => { if (result.data && result.data.version > summary.version) synchronize(id, result.data); }, [result.data, summary.version]);
  const row = result.data && (summary.version >= result.data.version ? { ...result.data, marked: summary.marked, note: summary.note, version: summary.version } : result.data);
  return <><Notice>{result.error}</Notice>{result.error && <button onClick={result.reload}>Reintentar detalle</button>}{row ? <ProcessDetailContent row={row} noteEditor={moduleCan(access, 'procesos', 'notas', 'editar') ? <Form fields={[{ name: 'note', label: 'Nota para todos los usuarios', multiline: true, maxLength: 10000 }]} initial={{ note: row.note }} submit="Guardar nota" onSubmit={async values => {
    await save(row, { note: values.note ?? '' }); notify('Nota compartida guardada.');
  }} /> : undefined} /> : !result.error && <Loading />}</>;
}
