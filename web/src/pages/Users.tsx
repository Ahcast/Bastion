import { useState } from 'react';
import { message } from '../api.js';
import { Dialog, Form, Heading, Loading, Notice, Table, useCore, useLoad } from '../ui.js';
import type { PageResult, Person, Role, UserRecord } from '../types.js';
export default function Users() {
  const { api, notify } = useCore(), [inviting, setInviting] = useState(false);
  const result = useLoad(signal => api.request<PageResult<UserRecord>>('/api/v1/admin/users?page=1&page_size=50', { signal }));
  const act = async (id: number, enabled?: boolean) => { try { await api.request(`/api/v1/admin/users/${id}${enabled === undefined ? '/invitation' : ''}`, { method: enabled === undefined ? 'POST' : 'PATCH', body: enabled === undefined ? undefined : { enabled } }); result.reload(); } catch (error) { notify(message(error)); } };
  return <><Heading title="Usuarios" description="Cuentas vinculadas a una Persona y un rol." action={<button className="primary" onClick={() => setInviting(true)}>Invitar usuario</button>} /><Notice>{result.error}</Notice>{result.data ? <Table columns={['Persona', 'Correo', 'Rol', 'Estado', 'Acción']} rows={result.data.items.map(user => [user.person_name, user.email, user.role_name, user.status, <div className="actions"><button onClick={() => void act(user.id, user.status === 'disabled')}>{user.status === 'disabled' ? 'Reactivar' : 'Desactivar'}</button>{user.status === 'invitation_pending' && <button onClick={() => void act(user.id)}>Reenviar</button>}</div>])} /> : !result.error && <Loading />}{inviting && <Invite close={() => setInviting(false)} done={() => { setInviting(false); result.reload(); }} />}</>;
}
function Invite({ close, done }: { close: () => void; done: () => void }) {
  const { api, notify } = useCore();
  const result = useLoad(signal => Promise.all([api.request<PageResult<Person>>('/api/v1/persons?page=1&page_size=100', { signal }), api.request<Role[]>('/api/v1/admin/roles', { signal })]));
  return <Dialog title="Invitar usuario" close={close}><Notice>{result.error}</Notice>{result.data ? <Form fields={[{ name: 'person_id', label: 'Persona', required: true, options: [{ value: '', label: 'Selecciona una persona' }, ...result.data[0].items.map(person => ({ value: person.id, label: person.full_name }))] }, { name: 'role_id', label: 'Rol', required: true, options: [{ value: '', label: 'Selecciona un rol' }, ...result.data[1].map(role => ({ value: role.id, label: role.name }))] }, { name: 'email', label: 'Correo', type: 'email', required: true }]} submit="Crear y enviar" onSubmit={async values => { await api.request('/api/v1/admin/users', { method: 'POST', body: { person_id: Number(values.person_id), role_id: Number(values.role_id), email: values.email } }); notify('Solicitud de invitación procesada. Revisa el estado del envío.'); done(); }} /> : !result.error && <Loading />}</Dialog>;
}
