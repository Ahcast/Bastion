import { useState } from 'react';
import { ApiError, message } from '../api.js';
import { Loading, Notice, useCore, useLoad } from '../ui.js';
import { useNavigationGuard, useUnsavedChanges } from '../navigationGuard.js';

export interface SarValues { limit_range: string; limit_date: string | null; current_invoice: string; sar_text: string }
export interface SarRecord { settings: SarValues; version: string; status: { days_remaining: number | null; invoices_remaining: number | null; range_expired: boolean; date_expired: boolean } }

export default function SarSettings() {
  const { access } = useCore();
  if (!access.is_system_administrator) return <p className="muted">La numeración SAR es un ajuste del Core. Solo el administrador del sistema puede consultarla y modificarla.</p>;
  return <SarSettingsLoader />;
}

function SarSettingsLoader() {
  const { api } = useCore();
  const result = useLoad(signal => api.request<SarRecord>('/api/v1/settings/sar', { signal }));
  return <section className="stack" aria-labelledby="sar-settings-title"><h2 id="sar-settings-title">Numeración SAR</h2>
    <p>Rango autorizado, fecha límite y factura actual compartidos por la empresa.</p>
    <Notice>{result.error}</Notice>{result.data ? <SarEditor key={result.data.version} initial={result.data} saved={result.setData} reload={result.reload} /> : !result.error && <Loading />}
    {result.error && <button onClick={result.reload}>Reintentar consulta SAR</button>}
  </section>;
}

function SarEditor({ initial, saved, reload }: { initial: SarRecord; saved: (value: SarRecord) => void; reload: () => void }) {
  const { api, notify } = useCore(), { navigate } = useNavigationGuard();
  const [values, setValues] = useState(initial.settings), [busy, setBusy] = useState(false), [error, setError] = useState(''), [conflict, setConflict] = useState(false);
  const dirty = JSON.stringify(values) !== JSON.stringify(initial.settings);
  useUnsavedChanges(dirty, busy);
  const change = (key: keyof SarValues, value: string | null) => setValues(current => ({ ...current, [key]: value }));
  return <><div className="row" aria-label="Estado SAR guardado"><p>Días restantes: <strong>{initial.status.days_remaining ?? 'Sin fecha'}</strong></p><p>Facturas restantes: <strong>{initial.status.invoices_remaining ?? 'Sin numeración'}</strong></p></div>
    {initial.status.date_expired && <Notice>La fecha límite registrada está vencida.</Notice>}
    {initial.status.range_expired && <Notice>El rango registrado está agotado.</Notice>}
    <form className="stack" aria-label="Configuración SAR" onSubmit={async event => {
      event.preventDefault(); if (busy || conflict || !dirty) return;
      setBusy(true); setError('');
      try { const result = await api.request<SarRecord>('/api/v1/settings/sar', { method: 'PUT', body: { settings: values, version: initial.version } }); saved(result); notify('Configuración SAR guardada.'); }
      catch (reason) { if (reason instanceof ApiError && reason.status === 412) { setConflict(true); setError('La configuración cambió. Tus cambios se conservan; recarga la versión actual antes de guardar.'); } else setError(message(reason)); }
      finally { setBusy(false); }
    }}><Notice>{error}</Notice><fieldset className="form-fields" disabled={busy}>
      <label>Rango autorizado<input name="limit_range" maxLength={255} value={values.limit_range} onChange={event => change('limit_range', event.target.value)} /><small>Copia el rango tal como aparece en tu autorización.</small></label>
      <label>Fecha límite de emisión<input name="limit_date" type="date" value={values.limit_date ?? ''} onChange={event => change('limit_date', event.target.value || null)} /></label>
      <label>Factura actual<input name="current_invoice" maxLength={80} value={values.current_invoice} onChange={event => change('current_invoice', event.target.value)} /></label>
      <label>Texto SAR<textarea name="sar_text" rows={4} maxLength={3000} value={values.sar_text} onChange={event => change('sar_text', event.target.value)} /></label>
      <p className="muted">El estado corresponde a los datos guardados. Guardar configura la información SAR; todavía no emite facturas ni asigna números a los borradores.</p>
      <div className="actions"><button type="submit" className="primary" disabled={!dirty || conflict}>{busy ? 'Guardando…' : 'Guardar SAR'}</button><button type="button" disabled={!dirty} onClick={() => { setValues(initial.settings); setError(''); }}>Cancelar cambios</button><button type="button" onClick={() => { void navigate(reload); }}>Recargar configuración SAR</button></div>
    </fieldset></form></>;
}

