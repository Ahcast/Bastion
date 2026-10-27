import { useId } from 'react';
import type { ReactNode } from 'react';

/** Shared section rhythm follows the established Cotizaciones workspace. */
export function DocumentSection({ title, metadata, action, children, list = false }: { title: string; metadata?: ReactNode; action?: ReactNode; children: ReactNode; list?: boolean }) {
  const id = useId();
  return <section className={`document-section${list ? ' document-list' : ''}`} aria-labelledby={id}>
    <div className="document-section-heading"><h2 id={id}>{title}</h2>{metadata !== undefined && <span className="document-metadata">{metadata}</span>}{action}</div>
    {children}
  </section>;
}

export function DocumentPagination({ page, hasMore, change, busy = false }: { page: number; hasMore: boolean; change: (page: number) => void; busy?: boolean }) {
  return <nav className="document-pagination" aria-label="Paginación de registros"><button disabled={busy || page === 1} onClick={() => change(page - 1)}>Anterior</button><span>Página {page}</span><button disabled={busy || !hasMore} onClick={() => change(page + 1)}>Siguiente</button></nav>;
}
