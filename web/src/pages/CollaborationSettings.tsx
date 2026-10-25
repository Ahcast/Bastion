import { useState } from 'react';
import { message } from '../api.js';
import type { FilePolicy } from '../collaboration.js';
import { useUnsavedChanges } from '../navigationGuard.js';
import { moduleCan } from '../types.js';
import { Loading, Notice, useCore, useLoad } from '../ui.js';
export default function CollaborationSettings() {
  const { api } = useCore();
  const result = useLoad(signal => api.request<FilePolicy>('/api/v1/colaboracion/settings', { signal }));
  return <section className="card"><h2>Colaboración · archivos y papelera</h2><p>Los cambios se aplican a nuevas cargas y nuevas entradas en papelera. Los archivos vigentes no caducan.</p><Notice>{result.error}</Notice>{result.data ? <PolicyForm key={JSON.stringify(result.data)} initial={result.data} saved={result.reload} /> : !result.error && <Loading />}</section>;
}
function PolicyForm({ initial, saved }: { initial: FilePolicy; saved: () => void }) {
  const { api, access, notify } = useCore();
  const [policy, setPolicy] = useState(initial), [allowed, setAllowed] = useState(initial.allowed_extensions.join(', ')), [blocked, setBlocked] = useState(initial.blocked_extensions.join(', '));
  const [dirty, setDirty] = useState(false), [busy, setBusy] = useState(false), [error, setError] = useState('');
  useUnsavedChanges(dirty, busy);
  const editable = moduleCan(access, 'colaboracion', 'settings', 'editar');
  return <form className="stack" onSubmit={async event => {
    event.preventDefault(); if (busy || !editable) return; setBusy(true); setError('');
    const list = (text: string) => [...new Set(text.split(',').map(value => value.trim().toLowerCase().replace(/^\./, '')).filter(Boolean))];
    try { await api.request('/api/v1/colaboracion/settings', { method: 'PUT', body: { ...policy, allowed_extensions: list(allowed), blocked_extensions: list(blocked) } }); setDirty(false); notify('Política de archivos guardada.'); saved(); }
    catch (reason) { setError(message(reason)); } finally { setBusy(false); }
  }}><Notice>{error}</Notice><fieldset className="form-fields" disabled={busy || !editable}><label>Máximo por archivo (MB)<input required type="number" min={1} max={100} value={policy.max_file_mb} onChange={event => { setPolicy({ ...policy, max_file_mb: Number(event.target.value) }); setDirty(true); }} /><small>Entre 1 y 100 MB; por defecto 50 MB.</small></label><label>Retención de papelera (días)<input required type="number" min={1} max={365} value={policy.trash_retention_days} onChange={event => { setPolicy({ ...policy, trash_retention_days: Number(event.target.value) }); setDirty(true); }} /></label><label>Extensiones permitidas<input required value={allowed} onChange={event => { setAllowed(event.target.value); setDirty(true); }} /><small>Separadas por comas. Ofimática, imágenes y ZIP; el servidor comprueba el MIME real.</small></label><label>Extensiones bloqueadas<input value={blocked} onChange={event => { setBlocked(event.target.value); setDirty(true); }} /><small>El bloqueo base de ejecutables, scripts y macros se aplica siempre. Puedes añadir restricciones.</small></label>{editable && <button className="primary" type="submit">{busy ? 'Guardando…' : 'Guardar política'}</button>}</fieldset><p>Restaurar conserva la carpeta original. El borrado definitivo requiere confirmación y queda auditado; la purga automática se ejecuta al vencer la retención registrada.</p></form>;
}
