import { createContext, useCallback, useContext, useEffect, useId, useMemo, useRef, useState } from 'react';
import type { Dispatch, ReactNode, SetStateAction } from 'react';
import { useBlocker, useLocation, useNavigate } from 'react-router';
import { useNavigationGuard } from './navigationGuard.js';
import { parseRoute, routeArea, routeCatalog, routeHref } from './routes.js';
import type { AppRoute } from './routes.js';
import { message } from './api.js';
import type { ApiClient } from './api.js';
import { Notice } from './ui.js';

type Routing = {
  route: AppRoute | null;
  go: (route: AppRoute, replace?: boolean) => void;
  patch: (key: string, value: string | null, replace?: boolean) => void;
  report: (id: string, error: string) => void;
};
const RouteContext = createContext<Routing | null>(null);
export const useAppRoute = () => useContext(RouteContext);

export function RoutingProvider({ children }: { children: ReactNode }) {
  const location = useLocation(), navigate = useNavigate(), guard = useNavigationGuard();
  const route = useMemo(() => parseRoute(location.pathname, location.search), [location.pathname, location.search]);
  const current = useRef(route); current.current = route;
  const bypass = useRef(false);
  const blocker = useBlocker(() => !bypass.current && !guard.isNavigating() && guard.blocked());
  const handled = useRef<string | null>(null);
  useEffect(() => {
    if (blocker.state !== 'blocked') { handled.current = null; return; }
    if (handled.current === blocker.location.key) return;
    handled.current = blocker.location.key;
    void guard.navigate(() => blocker.proceed()).then(ok => { if (!ok) blocker.reset(); });
  }, [blocker, guard.navigate]);
  const [errors, setErrors] = useState<Record<string, string>>({});
  const report = useCallback((id: string, error: string) => setErrors(old => old[id] === error ? old : { ...old, [id]: error }), []);
  const queued = useRef<{ route: AppRoute; replace: boolean } | null>(null);
  const go = useCallback((next: AppRoute, replace = false) => {
    const href = routeHref(next);
    if (!parseRoute(href.split('?')[0]!, href.includes('?') ? href.slice(href.indexOf('?')) : '')) return;
    queued.current = null;
    current.current = next;
    bypass.current = true;
    try { void navigate(href, { replace }); } finally { bypass.current = false; }
  }, [navigate]);
  // A click can set both record and mode. Commit a single history entry for it.
  const patch = useCallback((key: string, value: string | null, replace = false) => {
    const base = queued.current?.route ?? current.current;
    if (!base) return;
    const schema = routeCatalog[routeArea(base)];
    if (!schema || !(key === 'registro' && schema.record || key === 'vista' && schema.views || schema.fields?.includes(key))) return;
    const next = { ...base, values: { ...base.values } };
    if (key === 'registro' && value !== base.values.registro) {
      for (const child of ['crear', 'adjudicacion', 'factura', 'lote', 'correo', 'envios']) delete next.values[child];
      if (value === null && base.page === 'licitaciones') delete next.values.vista;
    }
    if (value === null || value === '') delete next.values[key]; else next.values[key] = value;
    if (routeHref(next) === routeHref(base)) return;
    const first = queued.current === null;
    queued.current = { route: next, replace: replace && (queued.current?.replace ?? true) };
    if (first) queueMicrotask(() => { const entry = queued.current; queued.current = null; if (entry) go(entry.route, entry.replace); });
  }, [go]);
  useEffect(() => { document.title = route ? `Bastion · ${route.page === 'ajustes' ? route.section : route.page}${route.values.registro ? ' · ' + route.values.registro : ''}` : 'Bastion · Página no encontrada'; }, [route]);
  const boundPatch: Routing['patch'] = (key, value, replace) => { if (route && current.current && routeArea(route) === routeArea(current.current)) patch(key, value, replace); };
  return <RouteContext.Provider value={{ route, go, patch: boundPatch, report }}><Notice>{Object.values(errors).filter(Boolean).join(' ')}</Notice>{children}</RouteContext.Provider>;
}

type Codec<T> = { read: (raw: string | undefined, fallback: T) => T; write: (value: T) => string | null };
export const numberRoute: Codec<number | null> = { read: raw => raw && /^\d+$/.test(raw) ? Number(raw) : null, write: value => value === null ? null : String(value) };
export const pageRoute: Codec<number> = { read: (raw, fallback) => raw ? Number(raw) : fallback, write: value => String(value) };
export const booleanRoute: Codec<boolean> = { read: (raw, fallback) => raw === undefined ? fallback : raw === '1', write: value => value ? '1' : null };
export function enumRoute<T extends string>(values: readonly T[]): Codec<T> { return { read: (raw, fallback) => values.includes(raw as T) ? raw as T : fallback, write: value => value }; }
export const nullableStringRoute: Codec<string | null> = { read: raw => raw ?? null, write: value => value };
export function useRouteState<T>(key: string, initial: T | (() => T), codec: Codec<T>): [T, Dispatch<SetStateAction<T>>] {
  const context = useAppRoute();
  const fallback = useRef<T>(undefined as T), initialized = useRef(false);
  if (!initialized.current) { fallback.current = typeof initial === 'function' ? (initial as () => T)() : initial; initialized.current = true; }
  const [local, setLocal] = useState(fallback.current);
  const value = context?.route ? codec.read(context.route.values[key], fallback.current) : local;
  const latest = useRef(value); latest.current = value;
  const set = (next: SetStateAction<T>) => {
    const resolved = typeof next === 'function' ? (next as (previous: T) => T)(latest.current) : next;
    const apply = () => { latest.current = resolved; setLocal(resolved); context?.patch(key, codec.write(resolved)); };
    apply();
  };
  return [value, set];
}

type RecordOptions<T> = { id?: (value: T) => string | null; empty?: () => T; restored?: (value: T) => void };
export function useRouteRecord<T>(key: string, initial: T, load: (id: string, signal: AbortSignal) => Promise<T>, options: RecordOptions<T> = {}): [T, Dispatch<SetStateAction<T>>] {
  const context = useAppRoute(), id = useId();
  const raw = context?.route?.values[key];
  const [value, setValue] = useState(initial);
  const refs = useRef({ load, options, value, context }); refs.current = { load, options, value, context };
  const loaded = useRef<string | undefined>(undefined);
  const mounted = useRef(true);
  useEffect(() => { mounted.current = true; return () => { mounted.current = false; }; }, []);
  useEffect(() => {
    if (!context || raw === loaded.current) return;
    const controller = new AbortController();
    loaded.current = raw;
    const apply = (record: T) => { if (!controller.signal.aborted) { setValue(record); refs.current.options.restored?.(record); } };
    context.report(id, '');
    if (!raw) apply(initial);
    else if (raw === 'nuevo') apply(refs.current.options.empty?.() ?? 'new' as T);
    else {
      setValue(initial);
      void refs.current.load(raw, controller.signal).then(apply).catch(error => { if (!controller.signal.aborted) context.report(id, `No se pudo abrir el registro: ${message(error)}`); });
    }
    return () => controller.abort();
  }, [raw]);
  useEffect(() => () => context?.report(id, ''), [id]);
  const set: Dispatch<SetStateAction<T>> = next => {
    if (!mounted.current) return;
    const record = typeof next === 'function' ? (next as (previous: T) => T)(refs.current.value) : next;
    const encoded = options.id ? options.id(record) : record === null ? null : record === 'new' ? 'nuevo' : String((record as { id: number }).id);
    loaded.current = encoded ?? undefined;
    setValue(record); refs.current.value = record;
    context?.patch(key, encoded, raw === 'nuevo' && encoded !== null && encoded !== 'nuevo');
  };
  return [context && raw !== loaded.current ? initial : value, set];
}

export async function findPagedRecord<T extends { id: number }>(api: ApiClient, path: string, id: string, signal: AbortSignal): Promise<T> {
  // These legacy catalogs expose authorized paged lists, not GET by ID.
  for (let page = 1; page <= 100; page++) {
    const result = await api.request<{ items: T[]; has_more?: boolean; page_size?: number }>(`${path}?page=${page}&page_size=100`, { signal });
    const found = result.items.find(item => String(item.id) === id);
    if (found) return found;
    if (!result.items.length || result.has_more === false || result.has_more === undefined && result.items.length < (result.page_size ?? 100)) break;
  }
  throw new Error('El registro no está disponible en el catálogo autorizado.');
}

export function useSavedRoute(id: number | undefined, clean = true) {
  const context = useAppRoute();
  useEffect(() => {
    if (id && clean && context?.route?.values.registro === 'nuevo') context.patch('registro', String(id), true);
  }, [id, clean, context?.route?.values.registro]);
}
