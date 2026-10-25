import { useEffect, useRef, useState } from 'react';
import { message } from '../api.js';
import { Notice, useCore, useLoad } from '../ui.js';
import { moduleCan } from '../types.js';
import { scoutDate } from '../scout-status.js';

export interface ScoutControlStatus {
  available: boolean; busy: boolean;
  job: { id: string; status: 'running' | 'succeeded' | 'partial' | 'failed' | 'busy' | 'interrupted'; started_at: string; finished_at: string | null } | null;
}
const names = { running: 'En ejecución', succeeded: 'Completado', partial: 'Completado parcialmente', failed: 'Falló la ejecución', busy: 'Scout ya estaba ejecutándose', interrupted: 'Ejecución interrumpida' };

export function ScoutRunControl({ refreshReports }: { refreshReports: () => void }) {
  const { api, access } = useCore();
  const canRun = moduleCan(access, 'procesos', 'importacion', 'editar');
  const status = useLoad(signal => api.request<ScoutControlStatus>('/api/v1/procesos/scout-run', { signal }), [], true);
  const [pending, setPending] = useState(false), [error, setError] = useState('');
  const attempt = useRef<string | null>(null), lock = useRef(false), previous = useRef<string | null>(null);
  const refresh = useRef(refreshReports); refresh.current = refreshReports;
  const job = status.data?.job;
  useEffect(() => {
    const timer = setInterval(status.reload, status.data?.busy ? 5000 : 30_000);
    return () => clearInterval(timer);
  }, [status.data?.busy]);
  useEffect(() => {
    const key = job ? `${job.id}:${job.status}` : null;
    if (previous.current && key !== previous.current) refresh.current();
    previous.current = key;
  }, [job?.id, job?.status]);
  return <section className="card" aria-label="Ejecución manual de Scout"><h2>Ejecutar Scout</h2>
    <p>Inicia el pipeline completo: extracción, combinación y clasificación IA. Puede iniciarse a cualquier hora y continúa en segundo plano.</p>
    <button type="button" className="primary" disabled={pending || !canRun || !status.data?.available || status.data.busy} title={!canRun ? 'Requiere permiso para importar procesos.' : undefined} onClick={async () => {
      if (lock.current || !canRun || !status.data?.available || status.data.busy) return;
      lock.current = true; setPending(true); setError('');
      attempt.current ??= crypto.randomUUID();
      try {
        const result = await api.request<ScoutControlStatus>('/api/v1/procesos/scout-run', { method: 'POST', body: {}, idempotencyKey: attempt.current });
        status.setData(result); attempt.current = null; refresh.current();
      } catch (reason) { setError(message(reason)); status.reload(); }
      finally { lock.current = false; setPending(false); }
    }}>{pending ? 'Iniciando…' : status.data?.busy ? 'Scout está en ejecución' : 'Ejecutar pipeline completo'}</button>
    <Notice>{error || status.error}</Notice>
    {status.data && !status.data.available && <p className="muted" role="status">La ejecución manual no está disponible en este servidor.</p>}
    {status.data?.busy && <p role="status">Hay un pipeline activo. No se iniciará otra ejecución simultánea.</p>}
    {job && <p role="status">Última ejecución manual: {names[job.status]} · inicio {scoutDate(job.started_at)}{job.finished_at && ` · fin ${scoutDate(job.finished_at)}`}</p>}
    <p className="muted">La clasificación utiliza la configuración y la caché IA de Scout. Los reportes de cada etapa se muestran debajo; importar el resultado en Bastion es un paso independiente.</p>
  </section>;
}
