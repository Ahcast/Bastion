import { lazy, Suspense, useCallback, useEffect, useRef, useState } from 'react';
import type { ApiClient } from './api.js';
import { message } from './api.js';
import Auth, { authFailure } from './Auth.js';
import type { AuthFlow } from './Auth.js';
import { Brand, CoreContext, Form, Heading, Loading, Notice, useFocusTrap } from './ui.js';
import type { Access, Page, User } from './types.js';
import { can } from './types.js';
const Dashboard = lazy(() => import('./pages/Dashboard.js'));
const People = lazy(() => import('./pages/People.js'));
const Users = lazy(() => import('./pages/Users.js'));
const Roles = lazy(() => import('./pages/Roles.js'));
const Settings = lazy(() => import('./pages/Settings.js'));
const Procesos = lazy(() => import('./pages/Procesos.js'));
const Account = lazy(() => import('./pages/Account.js'));
const labels: Record<Page, string> = { inicio: 'Inicio', personas: 'Personas', usuarios: 'Usuarios', roles: 'Roles y permisos', ajustes: 'Ajustes', cuenta: 'Mi cuenta', procesos: 'Procesos' };
export default function App({ api }: { api: ApiClient }) {
  const [user, setUser] = useState<User | null>(null), [access, setAccess] = useState<Access | null>(null), [page, setPage] = useState<Page>('inicio');
  const [flow, setFlow] = useState<AuthFlow>({ kind: 'login' }), [authMessage, setAuthMessage] = useState(''), [loading, setLoading] = useState(true), [toast, setToast] = useState(''), [reauth, setReauth] = useState(false);
  const loggedIn = useRef(false), initialized = useRef(false);
  const clear = useCallback((text: string) => { loggedIn.current = false; setUser(null); setAccess(null); setReauth(false); setFlow({ kind: 'login' }); setAuthMessage(text); }, []);
  const authenticated = useCallback(async () => {
    const current = await api.request<{ user: User }>('/api/v1/auth/me');
    const catalog = await api.request<Access>('/api/v1/auth/access');
    loggedIn.current = true; setUser(current.user); setAccess(catalog); setPage('inicio'); setReauth(false);
  }, [api]);
  useEffect(() => {
    api.hooks = { unauthorized: () => { if (loggedIn.current) clear('La sesión terminó. Inicia de nuevo.'); }, reauthenticate: () => setReauth(true) };
    if (initialized.current) return; initialized.current = true;
    void (async () => {
      try {
        const url = new URL(location.href), token = url.searchParams.get('token');
        if (token) history.replaceState(null, '', url.pathname);
        await api.refreshCsrf();
        if (token) {
          const kind = url.searchParams.get('flow') ?? (url.pathname.endsWith('/activate') ? 'activate' : url.pathname.endsWith('/reset-password') ? 'reset-password' : '');
          if (kind === 'activate' || kind === 'reset-password') setFlow({ kind: kind === 'activate' ? 'activate' : 'reset-complete', token });
          else setAuthMessage('El enlace no es válido.');
          return;
        }
        const bootstrap = await api.request<{ setup_required: boolean }>('/api/v1/bootstrap/status');
        if (bootstrap.setup_required) setFlow({ kind: 'bootstrap' }); else await authenticated();
      } catch (error) { setAuthMessage(authFailure(error)); } finally { setLoading(false); }
    })();
  }, [api, authenticated, clear]);
  useEffect(() => { if (!toast) return; const timer = setTimeout(() => setToast(''), 4500); return () => clearTimeout(timer); }, [toast]);
  if (loading) return <div className="boot">BASTION / CORE <span>conectando…</span></div>;
  return <>{user && access ? <CoreContext.Provider value={{ api, access, notify: setToast, refreshAccess: async () => setAccess(await api.request<Access>('/api/v1/auth/access')) }}><Shell user={user} access={access} page={page} navigate={setPage} logout={async () => { try { await api.request('/api/v1/auth/logout', { method: 'POST' }); clear('Sesión cerrada.'); } catch (error) { setToast(message(error)); } }}>{reauth ? <><Heading title="Verifica tu identidad" description="Se requiere TOTP para continuar con esta operación." /><section className="card"><Form fields={[{ name: 'code', label: 'Código del autenticador', required: true, inputMode: 'numeric', autocomplete: 'one-time-code' }]} submit="Verificar" onSubmit={async values => { await api.request('/api/v1/auth/reauth-totp', { method: 'POST', body: values }); setReauth(false); setToast('Identidad verificada. Repite la operación.'); }} /></section></> : <Suspense fallback={<Loading />}>{page === 'inicio' ? <Dashboard /> : page === 'procesos' ? <Procesos /> : page === 'personas' ? <People /> : page === 'usuarios' ? <Users /> : page === 'roles' ? <Roles /> : page === 'ajustes' ? <Settings /> : <Account />}</Suspense>}</Shell></CoreContext.Provider> : <Auth key={authMessage + flow.kind} api={api} initial={flow} initialMessage={authMessage} authenticated={authenticated} />}<div className={`toast${toast ? ' show' : ''}`} role="status">{toast}</div></>;
}
function Shell({ user, access, page, navigate, logout, children }: { user: User; access: Access; page: Page; navigate: (page: Page) => void; logout: () => Promise<void>; children: React.ReactNode }) {
  const [collapsed, setCollapsed] = useState(false), [open, setOpen] = useState(false), [mobile, setMobile] = useState(() => matchMedia('(max-width:800px)').matches);
  const menu = useRef<HTMLButtonElement>(null), side = useRef<HTMLElement>(null);
  const close = () => { setOpen(false); menu.current?.focus(); };
  useFocusTrap(side, mobile && open, close);
  useEffect(() => {
    const media = matchMedia('(max-width:800px)');
    const resize = () => { setMobile(media.matches); setOpen(false); if (side.current?.contains(document.activeElement)) menu.current?.focus(); };
    media.addEventListener('change', resize); return () => media.removeEventListener('change', resize);
  }, []);
  const expanded = mobile ? open : !collapsed;
  const entries = (Object.entries(labels) as [Page, string][]).filter(([id]) => id === 'inicio' || id === 'cuenta' || (id === 'procesos' ? access.modules.some(m => m.module_id === 'procesos') : id === 'personas' ? can(access, 'personas', 'ver') : access.is_system_administrator));
  return <div className={`shell${!mobile && collapsed ? ' sidebar-collapsed' : ''}`}><button className="nav-scrim" aria-label="Cerrar navegación" hidden={!mobile || !open} onClick={close} /><aside className={`sidebar${mobile && open ? ' open' : ''}`} id="side" ref={side} inert={!expanded} aria-hidden={!expanded} aria-label="Navegación del Core"><Brand /><button className="nav-close" aria-label="Cerrar navegación" onClick={close}>×</button><nav aria-label="Principal">{entries.map(([id, label]) => <button key={id} className={`navbtn${page === id ? ' active' : ''}`} aria-current={page === id ? 'page' : undefined} onClick={() => { navigate(id); if (mobile) close(); }}>{label}</button>)}</nav><div className="sidebar-footer">{user.name}<br />{user.role}</div></aside><section className="main" inert={mobile && open}><header className="topbar"><div className="topbar-location"><button ref={menu} className="menu-toggle" aria-label={expanded ? 'Ocultar menú lateral' : 'Mostrar menú lateral'} aria-controls="side" aria-expanded={expanded} onClick={() => mobile ? setOpen(!open) : setCollapsed(!collapsed)}>☰</button><strong>{labels[page].toUpperCase()}</strong></div><div className="topbar-account"><span className="topbar-email">{user.email}</span><button className="ghost fit" onClick={() => void logout()}>Salir</button></div></header><div className="content">{children}</div></section></div>;
}
export function ConfigurationError({ error }: { error: unknown }) { return <section className="auth-wrap"><div className="auth-card"><Brand /><h1>Configura la API</h1><Notice>{message(error)}</Notice></div></section>; }
