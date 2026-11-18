import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState } from 'react';
import type { ReactNode } from 'react';
import { Dialog } from './ui.js';

type Guard = () => 'unsaved' | 'saving' | null;
type Save = () => Promise<boolean>;
type NavigationGuard = { register: (guard: Guard, save?: Save, priority?: number) => () => void; navigate: (action: () => void) => Promise<boolean>; saveAll: () => Promise<boolean>; saveError: string; blocked: () => boolean; isNavigating: () => boolean };
const GuardContext = createContext<NavigationGuard | null>(null);

export function NavigationGuardProvider({ children }: { children: ReactNode }) {
  const guards = useRef(new Map<symbol, { guard: Guard; save?: Save; priority: number }>());
  const running = useRef(false), awaiting = useRef(false);
  const run = useCallback((action: () => void) => { const previous = running.current; running.current = true; try { action(); } finally { running.current = previous; } }, []);
  const blocked = useCallback(() => [...guards.current.values()].some(item => item.guard() !== null), []);
  const isNavigating = useCallback(() => running.current, []);
  const [pending, setPending] = useState<{ action: () => void; resolve: (accepted: boolean) => void } | null>(null);
  const [saving, setSaving] = useState(false);
  const [saveError, setSaveError] = useState('');
  const register = useCallback((guard: Guard, save?: Save, priority = 0) => {
    const key = Symbol();
    guards.current.set(key, { guard, save, priority });
    return () => { guards.current.delete(key); };
  }, []);
  const saveAll = useCallback(async () => {
    setSaveError('');
    const active = [...guards.current.values()].filter(item => item.guard()).sort((a, b) => a.priority - b.priority);
    if (active.some(item => item.guard() === 'saving' || !item.save)) { setSaveError('Termina las operaciones pendientes antes de guardar.'); return false; }
    for (const item of active) if (!await item.save!()) { setSaveError('No se guardaron todos los cambios. Revisa las secciones del expediente y vuelve a intentarlo.'); return false; }
    return true;
  }, []);
  const navigate = useCallback((action: () => void) => {
    if (running.current) { action(); return Promise.resolve(true); }
    if (awaiting.current) return Promise.resolve(false);
    const reasons = [...guards.current.values()].map(item => item.guard());
    if (reasons.includes('saving')) return Promise.resolve(false);
    if (reasons.includes('unsaved')) return new Promise<boolean>(resolve => { awaiting.current = true; setPending({ action: () => run(action), resolve: accepted => { awaiting.current = false; resolve(accepted); } }); });
    run(action);
    return Promise.resolve(true);
  }, [run]);
  const value = useMemo(() => ({ register, navigate, saveAll, saveError, blocked, isNavigating }), [register, navigate, saveAll, saveError, blocked, isNavigating]);
  const cancel = () => { pending?.resolve(false); setPending(null); };
  const canSave = [...guards.current.values()].filter(item => item.guard() === 'unsaved').every(item => item.save);
  return <GuardContext.Provider value={value}>{children}{pending && <Dialog title="Cambios sin guardar" close={cancel}><p>Hay cambios sin guardar.</p>{saveError && <p role="alert">{saveError}</p>}<div className="actions"><button disabled={saving} onClick={cancel}>Seguir editando</button>{canSave && <button className="primary" disabled={saving} onClick={async () => { setSaving(true); try { if (await saveAll()) { pending.action(); pending.resolve(true); setPending(null); } } finally { setSaving(false); } }}>Guardar y salir</button>}<button className="danger" disabled={saving} onClick={() => { pending.action(); pending.resolve(true); setPending(null); }}>Descartar y continuar</button></div></Dialog>}</GuardContext.Provider>;
}

export function useNavigationGuard() {
  const guard = useContext(GuardContext);
  if (!guard) throw new Error('Navigation guard provider required');
  return guard;
}

export function useUnsavedChanges(dirty: boolean, saving = false, save?: Save, priority = 0) {
  const register = useContext(GuardContext)?.register;
  const saveRef = useRef(save);
  saveRef.current = save;
  useEffect(() => {
    if (!dirty && !saving) return;
    const unregister = register?.(() => saving ? 'saving' : dirty ? 'unsaved' : null, save ? () => saveRef.current!() : undefined, priority);
    const beforeUnload = (event: BeforeUnloadEvent) => { event.preventDefault(); event.returnValue = ''; };
    window.addEventListener('beforeunload', beforeUnload);
    return () => { unregister?.(); window.removeEventListener('beforeunload', beforeUnload); };
  }, [dirty, saving, register, Boolean(save), priority]);
}
