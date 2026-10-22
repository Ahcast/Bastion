import { createRoot } from 'react-dom/client';
import { Component } from 'react';
import type { ReactNode } from 'react';
import App, { ConfigurationError } from './App.js';
import { NavigationGuardProvider } from './navigationGuard.js';
import { ApiClient, apiOrigin } from './api.js';
import './styles/panel.css';
import './styles/cotizaciones.css';
declare global { interface Window { BASTION_API_BASE?: string } }
class ErrorBoundary extends Component<{ children: ReactNode }, { failed: boolean }> {
  state = { failed: false };
  static getDerivedStateFromError() { return { failed: true }; }
  render() { return this.state.failed ? <section className="auth-wrap"><div className="auth-card"><h1>No se pudo cargar la pantalla</h1><p>Recarga la página para recuperar la interfaz.</p><button onClick={() => location.reload()}>Recargar</button></div></section> : this.props.children; }
}
const root = createRoot(document.getElementById('app')!);
try { const api = new ApiClient(apiOrigin(window.BASTION_API_BASE)); root.render(<ErrorBoundary><NavigationGuardProvider><App api={api} /></NavigationGuardProvider></ErrorBoundary>); }
catch (error) { root.render(<ConfigurationError error={error} />); }
