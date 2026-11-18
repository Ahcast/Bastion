import { useRouteState, pageRoute } from './routing.js';
import { useEffect, useState } from 'react';
import { useCore } from './ui.js';

export const emptyDocumentFilters = { search: '', client: '', date_from: '', date_to: '', status: '', min_total: '', max_total: '' };
export type DocumentFilters = typeof emptyDocumentFilters;
const prefix = 'bastion:document-filters:v1:';

export function clearDocumentFilters() {
  try {
    for (const key of Object.keys(window.sessionStorage)) if (key.startsWith(prefix)) window.sessionStorage.removeItem(key);
  } catch { /* Filtering still works when browser storage is unavailable. */ }
}

export function useDocumentFilters(module: 'ordenes_compra' | 'facturas') {
  const { userId } = useCore();
  const key = userId === undefined ? null : `${prefix}${userId}:${module}`;
  const [applied, setApplied] = useState(() => {
    const initial = { filters: { ...emptyDocumentFilters }, page: 1 };
    try {
      const saved: unknown = key ? JSON.parse(window.sessionStorage.getItem(key) ?? 'null') : null;
      if (!saved || typeof saved !== 'object') return initial;
      const { filters, page } = saved as { filters?: Record<string, unknown>; page?: unknown };
      if (!filters || !Object.keys(emptyDocumentFilters).every(name => typeof filters[name] === 'string' && (filters[name] as string).length <= 200)) return initial;
      return { filters: Object.fromEntries(Object.keys(emptyDocumentFilters).map(name => [name, filters[name]])) as DocumentFilters, page: typeof page === 'number' && Number.isInteger(page) && page >= 1 && page <= 10000 ? page : 1 };
    } catch { return initial; }
  });
  const [page, setRoutePage] = useRouteState('pagina', applied.page, pageRoute);
  const [draft, setDraft] = useState(applied.filters);
  useEffect(() => {
    if (key) try { window.sessionStorage.setItem(key, JSON.stringify({ ...applied, page })); } catch { /* Keep the current in-memory filters. */ }
  }, [key, applied, page]);
  const params = new URLSearchParams();
  for (const [name, value] of Object.entries(applied.filters)) if (value) params.set(name, value);
  return {
    draft, setDraft, query: params.toString(), page,
    setPage: (page: number) => { setRoutePage(page); setApplied(previous => ({ ...previous, page })); },
    apply: () => { const filters = Object.fromEntries(Object.entries(draft).map(([name, value]) => [name, value.trim()])) as DocumentFilters; setDraft(filters); setRoutePage(1); setApplied({ filters, page: 1 }); },
    clear: () => { setRoutePage(1); setDraft({ ...emptyDocumentFilters }); setApplied({ filters: { ...emptyDocumentFilters }, page: 1 }); },
  };
}
