import { useEffect, useState } from 'react';
import { message } from '../api.js';
import { Dialog, Form, Heading, Loading, Notice, useCore, useLoad } from '../ui.js';
import type { Module, Permission, Role } from '../types.js';
import { PermissionMatrix } from '../components/PermissionMatrix.js';
import { moduleIsConfigurable, permissionModuleCatalog } from '../permissionCatalog.js';
import { additionalPermissions, permissionKey, permissionSignature, removedAdditionalPermissions, setModulePermission, setPermissionColumn, setPermissionRow } from '../rolePermissions.js';
import { useNavigationGuard, useUnsavedChanges } from '../navigationGuard.js';
import '../styles/permissions.css';

export default function Roles() {
  const { access } = useCore();
  return access.is_system_administrator ? <RoleEditor /> : <><Heading title="Permisos por rol" description="Controla el acceso por módulo: ver, crear y editar, o eliminar." /><Notice>Esta pantalla está reservada al Administrador del sistema.</Notice></>;
}

function RoleEditor() {
  const { api, notify } = useCore(), { navigate } = useNavigationGuard();
  const result = useLoad(signal => Promise.all([api.request<Role[]>('/api/v1/admin/roles', { signal }), api.request<Module[]>('/api/v1/settings/modules', { signal })]));
  const [roleId, setRoleId] = useState<number | null>(null), [loadedId, setLoadedId] = useState<number | null>(null);
  const [original, setOriginal] = useState<Permission[] | null>(null), [draft, setDraft] = useState<Permission[]>([]);
  const [saving, setSaving] = useState(false), [creating, setCreating] = useState(false), [error, setError] = useState(''), [success, setSuccess] = useState('');
  const [loadVersion, setLoadVersion] = useState(0);
  const [confirmation, setConfirmation] = useState<{ permissions: Permission[]; text: string; label: string } | null>(null);
  const roles = result.data?.[0] ?? [], registry = result.data?.[1] ?? [], catalog = permissionModuleCatalog(registry);
  const role = roles.find(item => item.id === roleId), ready = loadedId === roleId && original !== null && !!role;
  const dirty = ready && permissionSignature(original) !== permissionSignature(draft);
  useUnsavedChanges(dirty, saving);

  useEffect(() => {
    if (result.data && !result.data[0].some(item => item.id === roleId)) setRoleId((result.data[0].find(item => !item.protected) ?? result.data[0][0])?.id ?? null);
  }, [result.data, roleId]);
  useEffect(() => {
    const controller = new AbortController();
    setOriginal(null); setDraft([]); setLoadedId(null); setError(''); setSuccess('');
    if (roleId !== null) void api.request<Permission[]>(`/api/v1/admin/roles/${roleId}/permissions`, { signal: controller.signal }).then(permissions => {
      if (!controller.signal.aborted) { setOriginal(permissions); setDraft(permissions); setLoadedId(roleId); }
    }).catch(reason => { if (!controller.signal.aborted) setError(message(reason)); });
    return () => controller.abort();
  }, [api, roleId, loadVersion]);

  const describe = (permission: Permission) => `${catalog.find(module => module.id === permission.module_id)?.label ?? permission.module_id} · ${permission.scope_key} · ${permission.capability}`;
  const change = (next: Permission[]) => {
    const removed = removedAdditionalPermissions(draft, next, catalog);
    if (removed.length) { setConfirmation({ permissions: next, text: `También se retirarán estos permisos específicos o adicionales:\n\n${removed.map(describe).join('\n')}\n\nLos cambios se aplicarán al guardar.`, label: 'Retirar permisos' }); return; }
    setDraft(next); setSuccess(''); setError('');
  };
  const save = async () => {
    if (!ready || !role || role.protected || saving || !dirty) return;
    setSaving(true); setError(''); setSuccess('');
    const permissions = draft.map(permission => ({ ...permission }));
    try {
      await api.request(`/api/v1/admin/roles/${role.id}`, { method: 'PATCH', body: { permissions } });
      setOriginal(permissions); setDraft(permissions); setSuccess('Permisos guardados correctamente.'); notify('Permisos guardados.');
    } catch (reason) { setError(message(reason)); }
    finally { setSaving(false); }
  };
  const extras = additionalPermissions(draft, catalog), protectedRole = role?.protected ?? false;
  return <>
    <Heading title="Permisos por rol" description="Controla el acceso por módulo: ver, crear y editar, o eliminar." action={<label className="permission-role-selector">Rol<select aria-label="Rol" value={roleId ?? ''} disabled={!result.data || saving} onChange={event => { const id = Number(event.target.value); if (id !== roleId) navigate(() => setRoleId(id)); }}><option value="" disabled>Selecciona un rol</option>{roles.map(item => <option key={item.id} value={item.id}>{item.name}{item.protected ? ' · Integrado' : ''}</option>)}</select></label>} />
    <div className="permission-role-tools"><p>{role?.description || 'Los roles personalizados comienzan sin permisos.'}</p><button disabled={saving} onClick={() => { void navigate(() => { setDraft(original ?? []); setError(''); setSuccess(''); setCreating(true); }); }}>Nuevo rol</button>{role && !role.protected && <button className="text-button" disabled={saving} onClick={async () => {
      if (!await navigate(() => {}) || !window.confirm(`¿Eliminar el rol «${role.name}»?`)) return;
      setSaving(true); setError('');
      try { await api.request(`/api/v1/admin/roles/${role.id}`, { method: 'DELETE' }); setOriginal(null); setDraft([]); setRoleId(null); result.reload(); notify('Rol eliminado.'); }
      catch (reason) { setError(message(reason)); } finally { setSaving(false); }
    }}>Eliminar rol</button>}</div>
    <Notice>{result.error || error}</Notice>{success && <p className="permission-success" role="status">{success}</p>}
    {protectedRole && <p className="notice">Rol integrado: tiene acceso completo a las funciones disponibles. Sus permisos están protegidos y no se pueden modificar.</p>}
    {!result.data ? !result.error && <Loading /> : !roles.length ? <p className="empty">No hay roles disponibles. Crea un rol para configurar sus permisos.</p> : !ready ? !error ? <Loading /> : <button onClick={() => setLoadVersion(value => value + 1)}>Reintentar carga</button> : <>
      <PermissionMatrix catalog={catalog} registry={registry} permissions={draft} protectedRole={protectedRole} disabled={saving} cell={(module, column, enabled) => change(setModulePermission(draft, module, column, enabled))} row={(module, enabled) => change(setPermissionRow(draft, module, enabled))} column={(column, enabled) => change(setPermissionColumn(draft, catalog.filter(module => moduleIsConfigurable(module, registry)).map(module => module.id), column, enabled))} />
      {extras.length > 0 && <details className="permission-additional"><summary>{extras.length} permisos específicos o adicionales conservados</summary><p>Incluyen alcances específicos y permisos fuera de la matriz. Se conservarán salvo que confirmes su retirada.</p><ul>{extras.map(permission => <li key={permissionKey(permission)}>{describe(permission)}</li>)}</ul></details>}
      {!protectedRole && <div className="permission-footer"><p role="status">{saving ? 'Guardando permisos…' : dirty ? 'Tienes cambios sin guardar.' : 'Sin cambios pendientes.'}</p><div className="actions"><button className="primary" disabled={saving || !dirty} onClick={() => void save()}>Guardar cambios</button><button disabled={saving || !dirty} onClick={() => setConfirmation({ permissions: original, text: 'Se descartarán los cambios sin guardar de este rol.', label: 'Descartar cambios' })}>Descartar cambios</button><button className="danger" disabled={saving || !draft.length} onClick={() => {
        const detail = extras.length ? `\n\nTambién se quitarán estos permisos adicionales:\n${extras.map(describe).join('\n')}` : '';
        setConfirmation({ permissions: [], text: `Se limpiarán todos los permisos del rol «${role?.name}».${detail}\n\nSe aplicará cuando pulses Guardar cambios.`, label: 'Limpiar borrador' });
      }}>Limpiar todos los permisos</button></div></div>}
    </>}
    {creating && <Dialog title="Crear rol" close={() => setCreating(false)}><Form fields={[{ name: 'name', label: 'Nombre', maxLength: 100, required: true }, { name: 'description', label: 'Descripción', maxLength: 500 }]} onSubmit={async values => {
      const created = await api.request<Role>('/api/v1/admin/roles', { method: 'POST', body: values });
      setOriginal(null); setDraft([]); setRoleId(created.id); setCreating(false); result.reload(); notify('Rol creado.');
    }} /></Dialog>}
    {confirmation && <Dialog title="Confirmar cambios de permisos" close={() => setConfirmation(null)}><p className="permission-confirmation">{confirmation.text}</p><div className="actions"><button onClick={() => setConfirmation(null)}>Cancelar</button><button className="danger" onClick={() => { setDraft(confirmation.permissions); setConfirmation(null); setError(''); setSuccess(''); }}>{confirmation.label}</button></div></Dialog>}
  </>;
}
