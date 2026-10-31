import { useEffect, useState } from 'react';
import { Heading, Notice, Table, useCore, useLoad } from '../ui.js';
import { message } from '../api.js';

type Item = { key: string; date: string; size: number; status: string };
type Job = { status: string; progress: string; error?: string; action?: string; started?: string };
type Prepared = { token: string; summary: string; expires_at: string };
type Status = { available: boolean; busy: boolean; next_run: string; job?: Job; jobs?: Job[]; history: Item[]; history_error?: string; prepared?: Prepared };
const endpoint = '/api/v1/settings/backups';

export default function BackupSettings() {
  const { api, access, notify } = useCore();
  const data = useLoad(signal => api.request<Status>(endpoint, { signal }));
  const [busy, setBusy] = useState(false);
  useEffect(() => { const timer = setInterval(data.reload, data.data?.busy ? 15_000 : 60_000); return () => clearInterval(timer); }, [data.data?.busy]);
  async function run(body: Record<string, unknown>) {
    setBusy(true);
    try { await api.request(endpoint, { method: 'POST', body }); data.reload(); }
    catch (error) { notify(message(error)); }
    finally { setBusy(false); }
  }
  async function download(key: string) {
    try {
      const value = await api.request<{ url: string }>(endpoint, { method: 'POST', body: { action: 'download', key } });
      const url = new URL(value.url);
      if (url.protocol !== 'https:' || !(url.hostname === 's3.amazonaws.com' || url.hostname.endsWith('.amazonaws.com'))) throw new Error('Enlace de descarga inválido.');
      window.location.assign(url.href);
    } catch (error) { notify(message(error)); }
  }
  async function upload(file: File) {
    if (!file.name.toLowerCase().endsWith('.zip')) { notify('Selecciona un archivo ZIP de Bastion.'); return; }
    setBusy(true);
    let uploadToken = '';
    try {
      const result = await api.request<{ token: string }>(endpoint, { method: 'POST', body: { action: 'upload_begin', size: file.size } });
      uploadToken = result.token;
      for (let offset = 0; offset < file.size; offset += 393216) {
        const bytes = new Uint8Array(await file.slice(offset, offset + 393216).arrayBuffer());
        let encoded = ''; for (const byte of bytes) encoded += String.fromCharCode(byte);
        await api.request(endpoint, { method: 'POST', body: { action: 'upload_chunk', token: result.token, offset, data: btoa(encoded) } });
      }
      await api.request(endpoint, { method: 'POST', body: { action: 'upload_finish', token: result.token } });
      uploadToken = '';
      data.reload();
    } catch (error) { if (uploadToken) await api.request(endpoint, { method: 'POST', body: { action: 'upload_abort', token: uploadToken } }).catch(() => {}); notify(message(error)); }
    finally { setBusy(false); }
  }
  if (!access.is_system_administrator) return <Notice>Solo administradores pueden administrar respaldos.</Notice>;
  const status = data.data, disabled = busy || Boolean(status?.busy) || !status?.available;
  return <><Heading title="Respaldos y restauración" description="Copias de datos persistentes, documentos y recuperación administrativa." />
    <Notice>{data.error}</Notice>
    <Notice>{status?.history_error}</Notice>
    <section className="card"><p>Servicio: {status?.available ? 'Disponible' : 'No disponible'}</p><p>Programación: todos los días a las 5:00 p. m. de Honduras.</p><p>Último ZIP: {status?.history[0]?.date ?? 'Sin respaldo integral disponible'}</p><p>Próxima ejecución: {status?.next_run ?? 'Consultando…'}</p><p>Durante la copia o restauración se activa mantenimiento y puede interrumpirse la conexión. El proceso continúa en el servidor.</p>
      <button disabled={disabled} onClick={() => void run({ action: 'backup' })}>Crear respaldo ahora</button>
      <button disabled={busy} onClick={data.reload}>Actualizar</button>
      {status?.job && <p role="status">{status.job.status}: {status.job.progress} {status.job.error}</p>}
    </section>
    <section className="card"><h2>Historial</h2><Table columns={['Fecha', 'Tamaño', 'Estado', 'Acciones']} rows={(status?.history ?? []).map(item => [item.date, `${(item.size / 1024 / 1024).toFixed(2)} MB`, item.status, <span><button disabled={busy} onClick={() => void download(item.key)}>Descargar ZIP</button> <button disabled={disabled} onClick={() => void run({ action: 'prepare_s3', key: item.key })}>Preparar restauración</button></span>])} /></section>
    <section className="card"><h2>Restaurar desde ZIP</h2><p>El archivo se valida antes de solicitar confirmación. Una restauración reemplaza datos y puede interrumpir el acceso.</p><input aria-label="ZIP de respaldo" type="file" accept=".zip" disabled={disabled} onChange={event => { const file = event.target.files?.[0]; if (file) void upload(file); event.target.value = ''; }} />
      {status?.prepared && <><p>{status.prepared.summary}</p><p>Confirmación válida hasta {status.prepared.expires_at}.</p><button disabled={disabled} onClick={() => { if (window.confirm('¿Confirmas reemplazar los datos indicados? Se activará mantenimiento y se creará un punto de recuperación previo.')) void run({ action: 'restore', token: status.prepared!.token, confirmed: true }); }}>Confirmar restauración</button><button disabled={disabled} onClick={() => void run({ action: 'discard', token: status.prepared!.token })}>Cancelar preparación</button></>}
    </section><section className="card"><h2>Operaciones recientes</h2><Table columns={['Fecha', 'Operación', 'Estado', 'Resultado']} rows={(status?.jobs ?? []).map(job => [job.started ?? '', job.action ?? '', job.status, job.error || job.progress])} /></section></>;
}
