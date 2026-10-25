import type { ScoutDiagnostics, ScoutExtraction, ScoutReport } from '../scout-status.js';
import { scoutAttemptMetrics, scoutAvailabilityNames, scoutDate, scoutIssueText, scoutMetricName, scoutReasons, scoutStageNames, scoutStateNames } from '../scout-status.js';

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
        {run.output_ready !== null && <div><dt>Ciclo completo</dt><dd>{run.output_ready ? 'Finalizado' : 'Todavía no está completo; se pueden incorporar las filas ya listas'}</dd></div>}
        <div><dt>Resultado / motivo</dt><dd>{run.reason ? scoutReasons[run.reason] ?? scoutReasons.other : 'No informado'}</dd></div>
        {run.retry_stages.length > 0 && <div><dt>Etapas para reintentar</dt><dd>{run.retry_stages.map(stage => scoutStageNames[stage]).join(', ')}</dd></div>}
      </dl>
      {run.extraction ? <ExtractionDetail data={run.extraction} /> : run.error_reported && <p className="scout-incidents">Scout registró incidencias en esta ejecución.</p>}
      {Object.keys(run.metrics).length ? <dl className="scout-metrics">{Object.entries(run.metrics).map(([key, value]) => <div key={key}><dt>{scoutMetricName(report.stage, key)}</dt><dd>{value.toLocaleString('es-HN')}</dd></div>)}</dl> : <p className="muted">No hay contadores publicados para esta etapa.</p>}
    </> : <p className="muted">{scoutAvailabilityNames[report.availability]}. No hay un resultado confirmado para mostrar.</p>}
  </section>;
}

function ExtractionDetail({ data }: { data: ScoutExtraction }) {
  const attempt = data.invocation;
  const phaseNames = { discovery: 'Listado de procesos', extraction: 'Detalles de procesos' };
  const phaseStates = { completo: 'Completado', pausado: 'Pausado', pendiente: 'Pendiente', desconocido: 'No informado' };
  return <div className="scout-extraction"><h4>Detalle de extracción</h4>
    {data.failure && <p className="scout-incidents">{scoutIssueText(data.failure)}</p>}
    <dl className="scout-run-facts">
      {data.discovery_complete !== null && <div><dt>Recorrido del listado</dt><dd>{data.discovery_complete ? 'Finalizado' : 'Todavía quedan páginas por recorrer'}</dd></div>}
      {data.next_page !== null && data.discovery_complete !== true && <div><dt>Página para continuar</dt><dd>{data.next_page}</dd></div>}
      {attempt?.last_saved_page != null && <div><dt>Última página guardada en este intento</dt><dd>{attempt.last_saved_page}</dd></div>}
      {attempt?.last_attempted_page != null && <div><dt>Última página solicitada</dt><dd>{attempt.last_attempted_page}</dd></div>}
      {attempt?.last_failed_page != null && <div><dt>Página del último fallo de listado</dt><dd>{attempt.last_failed_page}{attempt.last_failure && ` · ${scoutIssueText(attempt.last_failure)}`}</dd></div>}
      {data.phases.map(phase => <div key={phase.phase}><dt>{phaseNames[phase.phase]}</dt><dd>{phaseStates[phase.status]}{phase.issue && <> · {scoutIssueText(phase.issue)}</>}{phase.next_retry && <> · Próximo reintento desde {scoutDate(phase.next_retry)}</>}</dd></div>)}
    </dl>
    {attempt ? <><h4>Resultado de este intento</h4><dl className="scout-metrics">{Object.entries(scoutAttemptMetrics).flatMap(([key, label]) => attempt.metrics[key] === undefined ? [] : [<div key={key}><dt>{label}</dt><dd>{attempt.metrics[key]!.toLocaleString('es-HN')}</dd></div>])}</dl><p className="muted">Incluye detalles de la ronda actual e históricos consultados en este intento. Los acumulados de la ronda aparecen debajo.</p></> : <p className="muted">Este reporte anterior no incluye contadores por intento. Las cifras de abajo son acumuladas de la ronda.</p>}
    {data.detail_issues.length > 0 && <><h4>Fallos en los detalles pendientes</h4><ul>{data.detail_issues.map(({ issue, count }) => <li key={`${issue.code}:${issue.http_status}`}>{count.toLocaleString('es-HN')} · {scoutIssueText(issue)}</li>)}</ul></>}
  </div>;
}
