import { createContext, useContext, useEffect, useRef, useState } from 'react';
import type { ReactNode, RefObject } from 'react';
import { createPortal } from 'react-dom';
import type { ApiClient } from './api.js';
import { message } from './api.js';
import type { Access } from './types.js';

export const CoreContext = createContext<{ api: ApiClient; access: Access; notify: (text: string) => void; refreshAccess?: () => Promise<void> } | null>(null);
export function useCore() { const context = useContext(CoreContext); if (!context) throw new Error('Core context required'); return context; }
export function Brand() { return <div className="brand">BASTION <b>/</b> CORE <span className="muted"> · v1</span></div>; }
export function Notice({ children }: { children?: ReactNode }) { return children ? <div className="notice" role="alert">{children}</div> : null; }
export function Heading({ title, description, action }: { title: string; description: string; action?: ReactNode }) { return <div className="pagehead"><div><div className="eyebrow">Bastion / Core</div><h1 tabIndex={-1}>{title}</h1><p>{description}</p></div>{action}</div>; }
export function Loading() { return <p role="status">Cargando…</p>; }
export function Table({ columns, rows }: { columns: string[]; rows: ReactNode[][] }) {
  if (!rows.length) return <div className="empty">Sin registros.</div>;
  return <div className="table-wrap" tabIndex={0} role="region" aria-label="Tabla de registros"><table className="core-table"><thead><tr>{columns.map(c => <th key={c} scope="col">{c}</th>)}</tr></thead><tbody>{rows.map((row, index) => <tr key={index}>{row.map((cell, i) => <td key={i} data-label={columns[i]}>{cell ?? '—'}</td>)}</tr>)}</tbody></table></div>;
}
export interface Field { name: string; label: string; type?: string; required?: boolean; minLength?: number; maxLength?: number; inputMode?: 'numeric'; autocomplete?: string; options?: { value: string | number; label: string }[]; multiline?: boolean; help?: string }
export function Form({ fields, initial = {}, submit = 'Guardar', onSubmit, danger = false, children }: { fields: Field[]; initial?: Record<string, string | number | null>; submit?: string; danger?: boolean; onSubmit: (values: Record<string, string>) => Promise<void>; children?: ReactNode }) {
  const [busy, setBusy] = useState(false), [error, setError] = useState('');
  return <form className="stack" onSubmit={async event => {
    event.preventDefault(); if (busy) return;
    const values = Object.fromEntries(Array.from(new FormData(event.currentTarget)).map(([key, value]) => [key, String(value)]));
    setError(''); setBusy(true); try { await onSubmit(values); } catch (reason) { setError(message(reason)); } finally { setBusy(false); }
  }}><Notice>{error}</Notice><fieldset disabled={busy} className="form-fields">{fields.map(field => <label key={field.name}>{field.label}{field.options ? <select name={field.name} defaultValue={initial[field.name] ?? ''} required={field.required}>{field.options.map(o => <option key={o.value} value={o.value}>{o.label}</option>)}</select> : field.multiline ? <textarea name={field.name} rows={10} defaultValue={initial[field.name] ?? ''} required={field.required} maxLength={field.maxLength} /> : <input name={field.name} type={field.type ?? 'text'} defaultValue={initial[field.name] ?? ''} required={field.required} minLength={field.minLength} maxLength={field.maxLength} inputMode={field.inputMode} autoComplete={field.autocomplete} />}{field.help && <small>{field.help}</small>}</label>)}{children}<button className={danger ? 'danger' : 'primary'} type="submit">{busy ? 'Procesando…' : submit}</button></fieldset></form>;
}
export function useFocusTrap(ref: RefObject<HTMLElement | null>, active: boolean, close: () => void) {
  const closeRef = useRef(close); closeRef.current = close;
  useEffect(() => {
    if (!active) return;
    const previous = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    const controls = () => Array.from(ref.current?.querySelectorAll<HTMLElement>('button:not([disabled]),a[href],input:not([disabled]),select:not([disabled]),textarea:not([disabled]),[tabindex="0"]') ?? []).filter(el => !el.hidden);
    controls()[0]?.focus();
    const keydown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') { event.preventDefault(); closeRef.current(); }
      if (event.key !== 'Tab') return;
      const elements = controls(), first = elements[0], last = elements.at(-1);
      if (event.shiftKey && document.activeElement === first) { event.preventDefault(); last?.focus(); }
      else if (!event.shiftKey && document.activeElement === last) { event.preventDefault(); first?.focus(); }
    };
    const element = ref.current; element?.addEventListener('keydown', keydown);
    return () => { element?.removeEventListener('keydown', keydown); if (previous?.isConnected) previous.focus(); };
  }, [active, ref]);
}
export function Dialog({ title, close, children }: { title: string; close: () => void; children: ReactNode }) {
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => { const app = document.getElementById('app'); if (!app) return; const previous = app.inert; app.inert = true; return () => { app.inert = previous; }; }, []);
  useFocusTrap(ref, true, close);
  return createPortal(<div className="modal-backdrop" onMouseDown={e => { if (e.target === e.currentTarget) close(); }}><div className="modal" ref={ref} role="dialog" aria-modal="true" aria-labelledby="dialog-title"><div className="modal-head"><h2 id="dialog-title">{title}</h2><button onClick={close} aria-label="Cerrar diálogo" type="button">×</button></div>{children}</div></div>, document.body);
}
export function useLoad<T>(load: (signal: AbortSignal) => Promise<T>, dependencies: unknown[] = []) {
  const [data, setData] = useState<T | null>(null), [error, setError] = useState(''), [version, setVersion] = useState(0);
  useEffect(() => {
    const controller = new AbortController(); setError(''); setData(null);
    void load(controller.signal).then(value => { if (!controller.signal.aborted) setData(value); }).catch(reason => { if (!controller.signal.aborted) setError(message(reason)); });
    return () => controller.abort();
  // Dependencies describe the request; changing tabs cancels stale responses.
  }, [...dependencies, version]);
  return { data, error, reload: () => setVersion(v => v + 1) };
}
