import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState } from 'react';
import type { ReactNode } from 'react';
import { Dialog } from './ui.js';

type Guard = () => 'unsaved' | 'saving' | null;
type NavigationGuard = { register: (guard: Guard) => () => void; navigate: (action: () => void) => Promise<boolean> };
const GuardContext = createContext<NavigationGuard | null>(null);

export function NavigationGuardProvider({ children }: { children: ReactNode }) {
  const guard = useRef<Guard | null>(null);
  const [pending, setPending] = useState<{ action: () => void; resolve: (accepted: boolean) => void } | null>(null);
  const register = useCallback((next: Guard) => {
    guard.current = next;
    return () => { if (guard.current === next) guard.current = null; };
  }, []);
  const navigate = useCallback((action: () => void) => {
    const reason = guard.current?.();
    if (reason === 'saving') return Promise.resolve(false);
    if (reason === 'unsaved') return new Promise<boolean>(resolve => setPending({ action, resolve }));
    action();
    return Promise.resolve(true);
  }, []);
  const value = useMemo(() => ({ register, navigate }), [register, navigate]);
  const cancel = () => { pending?.resolve(false); setPending(null); };
  return <GuardContext.Provider value={value}>{children}{pending && <Dialog title="Cambios sin guardar" close={cancel}><p>Hay cambios sin guardar. Si continúas, se descartarán.</p><div className="actions"><button onClick={cancel}>Seguir editando</button><button className="danger" onClick={() => { pending.action(); pending.resolve(true); setPending(null); }}>Descartar y continuar</button></div></Dialog>}</GuardContext.Provider>;
}

export function useNavigationGuard() {
  const guard = useContext(GuardContext);
  if (!guard) throw new Error('Navigation guard provider required');
  return guard;
}

export function useUnsavedChanges(dirty: boolean, saving = false) {
  const { register } = useNavigationGuard();
  useEffect(() => {
    if (!dirty && !saving) return;
    const unregister = register(() => saving ? 'saving' : dirty ? 'unsaved' : null);
    const beforeUnload = (event: BeforeUnloadEvent) => { event.preventDefault(); event.returnValue = ''; };
    window.addEventListener('beforeunload', beforeUnload);
    return () => { unregister(); window.removeEventListener('beforeunload', beforeUnload); };
  }, [dirty, saving, register]);
}
