import { useState } from 'react';
import { message } from '../api.js';
import { Form, Heading, Loading, Notice, Table, useCore, useLoad } from '../ui.js';
import type { Audit, Mail, Module, PageResult, Setting } from '../types.js';
export default function Settings() {
  const [tab, setTab] = useState('modules');
  return <><Heading title="Ajustes del Core" description="Módulos, correo global, bitácora y reintentos." /><div className="row section-tabs" aria-label="Ajustes del Core">{[['modules', 'Módulos'], ['smtp', 'SMTP global'], ['audit', 'Bitácora'], ['mail', 'Envíos']].map(([id, title]) => <button key={id} aria-pressed={tab === id} onClick={() => setTab(id!)}>{title}</button>)}</div><section key={tab} className="card">{tab === 'modules' ? <Modules /> : tab === 'smtp' ? <Smtp /> : tab === 'audit' ? <AuditLog /> : <MailOutbox />}</section></>;
}
function Modules() {
  const { api, notify } = useCore(), result = useLoad(signal => api.request<Module[]>('/api/v1/settings/modules', { signal }));
  return <><Notice>{result.error}</Notice>{result.data ? <Table columns={['Módulo', 'Incluido', 'Ambiente', 'Estado', 'Acción']} rows={result.data.map(mod => [mod.display_name, mod.included_in_release ? 'Sí' : 'No', mod.environment_allowed ? 'Permitido' : 'No', mod.enabled ? 'Activo' : 'Inactivo', mod.module_id === 'core' ? 'Protegido' : <button onClick={async () => { try { await api.request(`/api/v1/settings/modules/${encodeURIComponent(mod.module_id)}`, { method: 'PATCH', body: { enabled: !mod.enabled } }); result.reload(); } catch (error) { notify(message(error)); } }}>{mod.enabled ? 'Desactivar' : 'Activar'}</button>])} /> : !result.error && <Loading />}</>;
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
