import { useEffect, useId, useLayoutEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';

export interface QuoteAction { label: string; run?: () => void; disabled?: boolean; reason?: string; danger?: boolean }

export function QuoteActions({ number, actions, busy }: { number: string; actions: QuoteAction[]; busy: boolean }) {
  const [open, setOpen] = useState(false), [position, setPosition] = useState({ left: 0, top: 0 });
  const trigger = useRef<HTMLButtonElement>(null), menu = useRef<HTMLDivElement>(null), id = useId();
  const close = () => { setOpen(false); trigger.current?.focus(); };
  useLayoutEffect(() => {
    if (!open) return;
    const place = () => {
      const rect = trigger.current!.getBoundingClientRect(), height = menu.current?.offsetHeight ?? 420;
      setPosition({ left: Math.max(8, Math.min(rect.right - 270, window.innerWidth - 278)), top: Math.max(8, Math.min(rect.bottom + 6, window.innerHeight - height - 8)) });
    };
    place(); menu.current?.querySelector<HTMLButtonElement>('button:not(:disabled)')?.focus();
    const outside = (event: PointerEvent) => { if (!trigger.current?.contains(event.target as Node) && !menu.current?.contains(event.target as Node)) setOpen(false); };
    document.addEventListener('pointerdown', outside); window.addEventListener('resize', place); window.addEventListener('scroll', place, true);
    return () => { document.removeEventListener('pointerdown', outside); window.removeEventListener('resize', place); window.removeEventListener('scroll', place, true); };
  }, [open]);
  useEffect(() => { if (busy) setOpen(false); }, [busy]);
  return <><button ref={trigger} disabled={busy} aria-label={`Acciones de ${number}`} aria-haspopup="menu" aria-expanded={open} aria-controls={open ? id : undefined} onClick={() => setOpen(value => !value)} onKeyDown={event => { if (event.key === 'ArrowDown') { event.preventDefault(); setOpen(true); } }}>Acciones ▾</button>{open && createPortal(<div id={id} ref={menu} className="quote-actions-menu" role="menu" aria-label={`Acciones de ${number}`} style={position} onKeyDown={event => {
    if (event.key === 'Escape') { event.preventDefault(); close(); }
    if (event.key === 'Tab') setOpen(false);
    if (!['ArrowDown', 'ArrowUp', 'Home', 'End'].includes(event.key)) return;
    event.preventDefault();
    const buttons = Array.from(menu.current!.querySelectorAll<HTMLButtonElement>('button:not(:disabled)'));
    const index = buttons.indexOf(document.activeElement as HTMLButtonElement);
    buttons[event.key === 'Home' ? 0 : event.key === 'End' ? buttons.length - 1 : (index + (event.key === 'ArrowDown' ? 1 : -1) + buttons.length) % buttons.length]?.focus();
  }}>{actions.map(action => <button key={action.label} role="menuitem" disabled={action.disabled} className={action.danger ? 'danger' : undefined} title={action.reason} onClick={() => { close(); action.run?.(); }}>{action.label}{action.disabled && action.reason && <small>{action.reason}</small>}</button>)}</div>, document.body)}</>;
}
