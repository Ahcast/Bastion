import { createContext, useContext, useEffect, useRef, useState } from 'react';
import type { ReactNode } from 'react';
import { moduleCan } from '../types.js';
import { useCore, useLoad } from '../ui.js';
import { readShortlistReceipts } from '../shortlist.js';
import type { ShortlistItem, ShortlistReceipts, TenderShortlist } from '../shortlist.js';

interface ShortlistState {
  data: TenderShortlist | null;
  items: ShortlistItem[];
  receipts: ShortlistReceipts;
  error: string;
  storageFailed: boolean;
  unread: number;
  reload: () => void;
  review: (items: ShortlistItem[]) => void;
}
const ShortlistContext = createContext<ShortlistState | null>(null);
export function useShortlist() {
  const context = useContext(ShortlistContext);
  if (!context) throw new Error('Shortlist context required');
  return context;
}

export function ShortlistProvider({ userId, children }: { userId: number; children: ReactNode }) {
  const { api, access } = useCore();
  const allowed = moduleCan(access, 'procesos', 'consulta', 'ver');
  const storageKey = `bastion.shortlist.${userId}`;
  const [receipts, setReceipts] = useState<ShortlistReceipts>(() => {
    try { return readShortlistReceipts(window.localStorage.getItem(storageKey)); } catch { return {}; }
  });
  const receiptsRef = useRef(receipts);
  const [storageFailed, setStorageFailed] = useState(false);
  const [clock, setClock] = useState(() => performance.now());
  const result = useLoad(async signal => {
    const requestedAt = performance.now();
    const feed = allowed ? await api.request<TenderShortlist>('/api/v1/procesos/lista-corta', { signal }) : null;
    return { feed, requestedAt };
  }, [allowed, api], true);
  useEffect(() => {
    if (!allowed) return;
    const refresh = () => {
      setClock(performance.now());
      if (document.visibilityState === 'visible') result.reload();
    };
    const timer = setInterval(refresh, 60_000);
    document.addEventListener('visibilitychange', refresh);
    const receive = (event: StorageEvent) => {
      if (event.key !== storageKey) return;
      const next = readShortlistReceipts(event.newValue);
      receiptsRef.current = next;
      setReceipts(next);
    };
    window.addEventListener('storage', receive);
    return () => {
      clearInterval(timer);
      document.removeEventListener('visibilitychange', refresh);
      window.removeEventListener('storage', receive);
    };
  }, [allowed, storageKey, result.reload]);
  const data = result.data?.feed ?? null;
  const serverNow = data ? Date.parse(data.checked_at) + Math.max(0, clock - result.data!.requestedAt) : 0;
  const items = allowed ? (data?.items ?? []).filter(item => Date.parse(item.fecha_recepcion) > serverNow) : [];
  const review = (reviewed: ShortlistItem[]) => {
    const next = Object.fromEntries(Object.entries({ ...receiptsRef.current, ...Object.fromEntries(reviewed.map(item => [item.id, item.version])) }).slice(-10_000));
    receiptsRef.current = next;
    setReceipts(next);
    try { window.localStorage.setItem(storageKey, JSON.stringify(next)); setStorageFailed(false); }
    catch { setStorageFailed(true); }
  };
  const value = { data, items, receipts, error: result.error, storageFailed, reload: result.reload, review,
    unread: items.filter(item => receipts[item.id] !== item.version).length };
  return <ShortlistContext.Provider value={value}>{children}</ShortlistContext.Provider>;
}

export function ShortlistAlert({ open }: { open: () => void }) {
  const { access } = useCore();
  const { unread } = useShortlist();
  if (!moduleCan(access, 'procesos', 'consulta', 'ver')) return null;
  return <button className={`shortlist-alert${unread ? ' has-unread' : ''}`} onClick={open}
    aria-label={`Lista corta de licitaciones${unread ? `, ${unread} por revisar` : ''}`}>
    Lista corta{unread > 0 && <strong aria-live="polite">{unread}</strong>}
  </button>;
}
