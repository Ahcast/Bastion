import CollaborationSettings from './CollaborationSettings.js';
import SarSettings from './SarSettings.js';
import { useNavigationGuard } from '../navigationGuard.js';
import { lazy, Suspense, useState } from 'react';
import { message } from '../api.js';
import { Form, Heading, Loading, Notice, Table, useCore, useLoad } from '../ui.js';
import type { Audit, Mail, Module, PageResult, Setting } from '../types.js';
import type { Access } from '../types.js';
import { settingsSections } from '../navigation.js';
import type { SettingsSection } from '../navigation.js';
const Account = lazy(() => import('./Account.js'));
const People = lazy(() => import('./People.js'));
const Users = lazy(() => import('./Users.js'));
const Roles = lazy(() => import('./Roles.js'));
const ProcessSettings = lazy(() => import('./ProcessSettings.js'));
const SupplierSettings = lazy(() => import('./Proveedores.js').then(module => ({ default: module.SupplierSettings })));
const TenderSettings = lazy(() => import('./Licitaciones.js').then(module => ({ default: module.TenderSettings })));
export function SettingsNavigation({ access, section, select }: { access: Access; section: SettingsSection; select: (section: SettingsSection) => void }) {
  return <nav className="settings-navigation" aria-label="Secciones de Ajustes">{settingsSections(access).map(entry => <button key={entry.id} aria-current={section === entry.id ? 'page' : undefined} onClick={() => select(entry.id)}>{entry.label}</button>)}</nav>;
}
const OrderSettings = lazy(() => import('./OrdenesCompra.js').then(module => ({ default: module.OrderSettings })));
const ExpenseSettings = lazy(() => import('./Gastos.js').then(module => ({ default: module.ExpenseSettings })));
export default function Settings({ section, select }: { section: SettingsSection; select: (section: SettingsSection) => void }) {
  const { access } = useCore();
  const selected = settingsSections(access).find(entry => entry.id === section) ?? { id: 'cuenta', label: 'Mi cuenta' };
  return <section aria-label="Ajustes"><div className="settings-location"><span>Ajustes</span><span aria-hidden="true"> / </span><strong>{selected.label}</strong></div><SettingsNavigation access={access} section={selected.id} select={select} /><div key={selected.id}><Suspense fallback={<Loading />}>{selected.id === 'colaboracion' ? <CollaborationSettings /> : selected.id === 'personas' ? <People /> : selected.id === 'usuarios' ? <Users /> : selected.id === 'roles' ? <Roles /> : selected.id === 'core' ? <CoreSettings /> : selected.id === 'clientes' ? <ClientSettings /> : selected.id === 'proveedores' ? <SupplierSettings /> : selected.id === 'ordenes_compra' ? <OrderSettings /> : selected.id === 'cotizaciones' ? <QuoteSettings /> : selected.id === 'facturas' ? <InvoiceSettings /> : selected.id === 'gastos' ? <ExpenseSettings /> : selected.id === 'reportes' ? <ReportSettings /> : selected.id === 'licitaciones' ? <TenderSettings /> : selected.id === 'procesos' ? <ProcessSettings /> : <Account />}</Suspense></div></section>;
}
function CoreSettings() {
  const { navigate } = useNavigationGuard();
  const [tab, setTab] = useState('modules');
  return <><Heading title="Ajustes del Core" description="Empresa y SAR, módulos, correo global, bitácora y reintentos." /><div className="row section-tabs" aria-label="Ajustes del Core">{[['modules', 'Módulos'], ['company', 'Empresa y SAR'], ['smtp', 'SMTP global'], ['audit', 'Bitácora'], ['mail', 'Envíos']].map(([id, title]) => <button key={id} aria-pressed={tab === id} onClick={() => { if (id !== tab) void navigate(() => setTab(id!)); }}>{title}</button>)}</div><section key={tab} className="card">{tab === 'modules' ? <Modules /> : tab === 'company' ? <><Company /><SarSettings /></> : tab === 'smtp' ? <Smtp /> : tab === 'audit' ? <AuditLog /> : <MailOutbox />}</section></>;
}
function Modules() {
  const { api, notify, refreshAccess } = useCore(), result = useLoad(signal => api.request<Module[]>('/api/v1/settings/modules', { signal }));
  return <><Notice>{result.error}</Notice>{result.data ? <Table columns={['Módulo', 'Incluido', 'Ambiente', 'Estado', 'Acción']} rows={result.data.map(mod => [mod.display_name, mod.included_in_release ? 'Sí' : 'No', mod.environment_allowed ? 'Permitido' : 'No', mod.enabled ? 'Activo' : 'Inactivo', mod.module_id === 'core' ? 'Protegido' : <button disabled={!mod.enabled && (!mod.included_in_release || !mod.environment_allowed)} onClick={async () => { try { await api.request(`/api/v1/settings/modules/${encodeURIComponent(mod.module_id)}`, { method: 'PATCH', body: { enabled: !mod.enabled } }); await refreshAccess?.(); result.reload(); } catch (error) { notify(message(error)); } }}>{mod.enabled ? 'Desactivar' : 'Activar'}</button>])} /> : !result.error && <Loading />}</>;
}
function Company() {
  const { api, notify } = useCore(), result = useLoad(signal => api.request<Setting[]>('/api/v1/settings/core', { signal }));
  const profile = result.data?.find(setting => setting.setting_key === 'company.profile')?.value_json as Record<string, string> | undefined;
  return <><h2>Empresa</h2><p>Datos compartidos para la cabecera de cotizaciones y otros documentos.</p><Notice>{result.error}</Notice>{result.data ? <Form fields={[{ name: 'name', label: 'Nombre de empresa', required: true, maxLength: 255 }, { name: 'rtn', label: 'RTN', maxLength: 40 }, { name: 'email', label: 'Correo de empresa', type: 'email', maxLength: 254 }, { name: 'phone', label: 'Teléfono', maxLength: 60 }, { name: 'address', label: 'Dirección', multiline: true, maxLength: 1000 }]} initial={profile ?? {}} submit="Guardar empresa" onSubmit={async fields => { await api.request('/api/v1/settings/core/company.profile', { method: 'PUT', body: { value: fields } }); notify('Datos de empresa guardados.'); result.reload(); }} /> : !result.error && <Loading />}</>;
}
function Smtp() {
  const { api, notify } = useCore(), result = useLoad(signal => api.request<Setting[]>('/api/v1/settings/core', { signal }));
  const values = Object.fromEntries(result.data?.map(setting => [setting.setting_key, setting.value_json]) ?? []);
  return <><h2>SMTP global</h2><p>Host/puerto/TLS del servidor; las credenciales se configuran por usuario.</p><Notice>{result.error}</Notice>{result.data ? <Form fields={[{ name: 'host', label: 'Host', maxLength: 253, required: true }, { name: 'port', label: 'Puerto', options: [{ value: 465, label: '465 · TLS' }, { value: 587, label: '587 · STARTTLS' }] }, { name: 'tls', label: 'TLS', options: [{ value: 'implicit_tls', label: 'TLS implícito' }, { value: 'starttls_required', label: 'STARTTLS requerido' }] }]} initial={{ host: String(values['smtp.host'] ?? ''), port: Number(values['smtp.port'] ?? 465), tls: String(values['smtp.tls_mode'] ?? 'implicit_tls') }} submit="Guardar SMTP" onSubmit={async fields => { for (const [key, value] of [['smtp.host', fields.host], ['smtp.port', Number(fields.port)], ['smtp.tls_mode', fields.tls]] as const) await api.request(`/api/v1/settings/core/${key}`, { method: 'PUT', body: { value } }); notify('Ajustes SMTP guardados.'); }} /> : !result.error && <Loading />}</>;
}
function AuditLog() {
  const { api } = useCore(), result = useLoad(signal => api.request<PageResult<Audit>>('/api/v1/settings/audit?page=1&page_size=50', { signal }));
  return <><h2>Bitácora</h2><Notice>{result.error}</Notice>{result.data ? <Table columns={['Fecha', 'Actor', 'Módulo', 'Acción', 'Registro', 'Resultado']} rows={result.data.items.map(event => [event.created_at, event.actor_name ?? 'Sistema', event.module_id, event.action, `${event.target_type} ${event.target_id ?? ''}`, event.result])} /> : !result.error && <Loading />}</>;
}
function MailOutbox() {
  const { api, notify } = useCore(), result = useLoad(signal => api.request<PageResult<Mail>>('/api/v1/settings/mail-outbox?page=1&page_size=50', { signal }));
  return <><h2>Envíos</h2><Notice>{result.error}</Notice>{result.data ? <Table columns={['Fecha', 'Tipo', 'Destino', 'Estado', 'Intentos', 'Error', 'Acción']} rows={result.data.items.map(mail => [mail.created_at, mail.kind, mail.recipient_email, mail.delivery_status, mail.attempts, mail.last_error_code, mail.delivery_status === 'failed' ? <button onClick={async () => { try { await api.request(`/api/v1/settings/mail-outbox/${mail.id}/retry`, { method: 'POST' }); result.reload(); } catch (error) { notify(message(error)); } }}>Reintentar</button> : '—'])} /> : !result.error && <Loading />}</>;
}

function QuoteSettings() { return <section className="card"><h2>Cotizaciones</h2><p>Primera etapa: cotizaciones con cálculo, numeración y estados cotejados con CastilloIT. La validación contable, PDF, adjuntos, envío y conversiones siguen pendientes.</p><p>Los ajustes propios del módulo aún requieren definición. Empresa y servidor SMTP se administran en el Core; cada usuario configura sus credenciales de correo en Mi cuenta.</p></section>; }

function ClientSettings() { return <section className="card"><h2>Clientes</h2><p>Catálogo de clientes y contactos con búsqueda, alta y edición.</p><p>Esta versión no tiene ajustes editables. Los ajustes propios del módulo siguen pendientes de definición.</p></section>; }

function InvoiceSettings() { return <section className="card stack"><h2>Facturas</h2><p>Facturas con número SAR al marcar como enviado, pagos y saldos, clonación y documentos privados. Las facturas enviadas y pagadas no se editan ni eliminan. Solo se eliminan borradores sin pagos; se conservan sus relaciones de origen.</p><SarSettings /></section>; }


function ReportSettings() { return <section className="card"><h2>Reportes</h2><p>El módulo no tiene ajustes propios documentados. Los permisos de lectura se asignan por cada dominio en Roles y permisos.</p></section>; }
