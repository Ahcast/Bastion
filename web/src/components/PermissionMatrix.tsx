import { useEffect, useRef } from 'react';
import type { Module, Permission } from '../types.js';
import { moduleIsConfigurable } from '../permissionCatalog.js';
import type { PermissionModule } from '../permissionCatalog.js';
import { PERMISSION_COLUMNS, permissionState } from '../rolePermissions.js';
import type { PermissionColumn } from '../rolePermissions.js';

type State = boolean | 'mixed';
export function Check({ label, name, state, disabled, change }: { label: string; name?: string; state: State; disabled?: boolean; change: (checked: boolean) => void }) {
  const input = useRef<HTMLInputElement>(null);
  useEffect(() => { if (input.current) input.current.indeterminate = state === 'mixed'; }, [state]);
  return <label className="permission-check"><input ref={input} type="checkbox" aria-label={name ?? label} checked={state === true} aria-checked={state} disabled={disabled} onChange={event => change(event.target.checked)} /><span>{label}</span></label>;
}
function combined(states: State[]): State {
  return states.length && states.every(state => state === true) ? true : states.some(state => state !== false) ? 'mixed' : false;
}
export function PermissionMatrix({ catalog, registry, permissions, protectedRole, disabled, cell, row, column }: {
  catalog: PermissionModule[]; registry: Module[]; permissions: Permission[]; protectedRole: boolean; disabled: boolean;
  cell: (module: string, column: PermissionColumn, enabled: boolean) => void;
  row: (module: string, enabled: boolean) => void;
  column: (column: PermissionColumn, enabled: boolean) => void;
}) {
  const editable = catalog.filter(module => moduleIsConfigurable(module, registry));
  const state = (module: PermissionModule, col: PermissionColumn): State => module.kind === 'session' ? col === 'ver' : module.kind === 'administration' ? protectedRole : protectedRole && moduleIsConfigurable(module, registry) ? true : permissionState(permissions, module.id, col);
  return <div className="permission-matrix-wrap"><table className="permission-matrix">
    <caption>Crear y editar comparten un permiso; Eliminar se configura por separado. Ambos requieren Ver. Una casilla parcial indica acceso a recursos específicos.</caption>
    <thead><tr><th scope="col">Módulo</th>{PERMISSION_COLUMNS.map(col => <th key={col.id} scope="col"><span className="permission-column-title">{col.label}</span><Check label="Marcar todo" name={`Marcar todo en ${col.label}`} state={combined(editable.map(module => state(module, col.id)))} disabled={disabled || protectedRole || !editable.length} change={enabled => column(col.id, enabled)} /></th>)}</tr></thead>
    <tbody>{catalog.map(module => {
      const configurable = moduleIsConfigurable(module, registry), available = registry.find(entry => entry.module_id === module.id);
      const hint = module.kind === 'session' ? 'Disponible para todas las cuentas.' : module.kind === 'administration' ? 'Reservado al Administrador del sistema.' : !available?.included_in_release ? 'Próximamente.' : !configurable ? 'No disponible en este ambiente.' : !available.enabled ? 'Desactivado; puedes preparar sus permisos.' : '';
      return <tr key={module.id}><th scope="row"><strong>{module.label}</strong>{hint && <small>{hint}</small>}<Check label="Marcar todo" name={`Marcar todo · ${module.label}`} state={combined(PERMISSION_COLUMNS.map(col => state(module, col.id)))} disabled={disabled || protectedRole || !configurable} change={enabled => row(module.id, enabled)} /></th>{PERMISSION_COLUMNS.map(col => <td key={col.id} data-label={col.label}><Check label={col.label} name={`${col.label} · ${module.label}`} state={state(module, col.id)} disabled={disabled || protectedRole || !configurable} change={enabled => cell(module.id, col.id, enabled)} /></td>)}</tr>;
    })}</tbody>
  </table></div>;
}
