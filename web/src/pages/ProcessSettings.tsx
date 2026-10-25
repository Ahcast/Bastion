import { useEffect } from 'react';
import { Heading, Loading, Notice, useCore, useLoad } from '../ui.js';
import { moduleCan } from '../types.js';
import type { ScoutDiagnostics } from '../scout-status.js';
import { ScoutRunControl } from '../components/ScoutRunControl.js';
import { ScoutStatusPanel } from '../components/ScoutStatusPanel.js';
import '../styles/scout-status.css';

export default function ProcessSettings() {
  const { api, access } = useCore(), view = moduleCan(access, 'procesos', 'consulta', 'ver');
  const report = useLoad(signal => view ? api.request<ScoutDiagnostics>('/api/v1/procesos/scout-status', { signal }) : Promise.resolve(null), [view], true);
  useEffect(() => { if (!view) return; const timer = setInterval(report.reload, 60_000); return () => clearInterval(timer); }, [view]);
  if (!view) return <Notice>No tienes acceso a la información de Procesos.</Notice>;
  return <><Heading title="Ajustes de Procesos" description="Estado de Scout, su última ejecución y el análisis IA." action={<button onClick={report.reload}>Volver a consultar</button>} />
    <section className="card"><h2>Información del módulo</h2><p>Entrada: maestro.csv de bastion-scout después del análisis IA.</p><p>Conservación: todos los procesos permanecen en el histórico; las marcas y notas son compartidas. La depuración sigue pendiente.</p><p className="muted">Consultar esta pantalla no ejecuta Scout. El pipeline solo se inicia con el botón de ejecución manual.</p></section>
    <ScoutRunControl refreshReports={report.reload} />
    <Notice>{report.error}</Notice>{report.data ? <ScoutStatusPanel data={report.data} /> : !report.error && <Loading />}
  </>;
}
