import type { ScoutDiagnostics, ScoutReport } from '../scout-status.js';
import { scoutAvailabilityNames, scoutDate, scoutMetricName, scoutReasons, scoutStageNames, scoutStateNames } from '../scout-status.js';

export function ScoutStatusPanel({ data }: { data: ScoutDiagnostics }) {
  return <section className="scout-status" aria-label="Estado informado por Scout"><div className="scout-status-heading"><h2>Última ejecución reportada</h2><p>Consultado: {scoutDate(data.checked_at)} · hora de Honduras</p></div>
    <p className="muted">Información de los reportes publicados por Scout. Se consulta cada 60 segundos.</p>
    <div className="scout-report-grid">{data.reports.map(report => <Report key={report.stage} report={report} />)}</div>
    <p className="muted">Los retiros del maestro corresponden a Scout; los procesos ya importados en Bastion conservan su histórico.</p>
  </section>;
}

function Report({ report }: { report: ScoutReport }) {
  const run = report.run;
  return <section className="scout-report" aria-label={scoutStageNames[report.stage]}><header><h3>{scoutStageNames[report.stage]}</h3>{run && <span className={`status ${run.status === 'error' ? 'failed' : run.status === 'exitoso' ? 'active' : ''}`}>{scoutStateNames[run.status]}</span>}</header>
    {run ? <>
      {report.matches_current_run === false && <p className="scout-other-cycle">Este reporte corresponde a otro ciclo.</p>}
      <dl className="scout-run-facts"><div><dt>Ciclo</dt><dd>{run.run_id}</dd></div><div><dt>Inicio</dt><dd>{scoutDate(run.started_at)}</dd></div><div><dt>Fin</dt><dd>{scoutDate(run.finished_at)}</dd></div><div><dt>Duración</dt><dd>{run.duration_seconds === null ? 'No informada' : `${Math.floor(run.duration_seconds / 60)} min ${run.duration_seconds % 60} s`}</dd></div>
        {run.deadline && <div><dt>Límite horario</dt><dd>{scoutDate(run.deadline)}</dd></div>}
        {run.output_ready !== null && <div><dt>Entrega informada</dt><dd>{run.output_ready ? 'Lista para incorporar' : 'Todavía no está lista'}</dd></div>}
        <div><dt>Resultado / motivo</dt><dd>{run.reason ? scoutReasons[run.reason] ?? scoutReasons.other : 'No informado'}</dd></div>
        {run.retry_stages.length > 0 && <div><dt>Etapas para reintentar</dt><dd>{run.retry_stages.map(stage => scoutStageNames[stage]).join(', ')}</dd></div>}
      </dl>
      {run.error_reported && <p className="scout-incidents">Scout registró incidencias en esta ejecución.</p>}
      {Object.keys(run.metrics).length ? <dl className="scout-metrics">{Object.entries(run.metrics).map(([key, value]) => <div key={key}><dt>{scoutMetricName(report.stage, key)}</dt><dd>{value.toLocaleString('es-HN')}</dd></div>)}</dl> : <p className="muted">No hay contadores publicados para esta etapa.</p>}
    </> : <p className="muted">{scoutAvailabilityNames[report.availability]}. No hay un resultado confirmado para mostrar.</p>}
  </section>;
}
