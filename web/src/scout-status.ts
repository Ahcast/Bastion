export type ScoutStage = 'main' | 'scout' | 'combiner' | 'clasificador';
export type ScoutAvailability = 'available' | 'missing' | 'unreadable' | 'invalid' | 'changed';
export interface ScoutRun {
  run_id: string; status: 'ejecutando' | 'exitoso' | 'parcial' | 'error' | 'desconocido';
  started_at: string | null; finished_at: string | null; deadline: string | null;
  duration_seconds: number | null; output_ready: boolean | null; reason: string | null;
  retry_stages: ScoutStage[]; metrics: Record<string, number>; error_reported: boolean;
  extraction?: ScoutExtraction | null;
}
export interface ScoutIssue { code: string; http_status: number | null }
export interface ScoutAttempt {
  last_attempted_page: number | null; last_saved_page: number | null; last_failed_page: number | null;
  last_failure: ScoutIssue | null; metrics: Record<string, number>;
}
export interface ScoutExtraction {
  next_page: number | null; discovery_complete: boolean | null; failure: ScoutIssue | null;
  phases: { phase: 'discovery' | 'extraction'; status: 'completo' | 'pausado' | 'pendiente' | 'desconocido'; next_retry: string | null; issue: ScoutIssue | null }[];
  invocation: ScoutAttempt | null; detail_issues: { issue: ScoutIssue; count: number }[];
}
export interface ScoutReport { stage: ScoutStage; availability: ScoutAvailability; matches_current_run: boolean | null; run: ScoutRun | null }
export interface ScoutDiagnostics { checked_at: string; reports: ScoutReport[] }
export const scoutStageNames: Record<ScoutStage, string> = { main: 'Ejecución general', scout: 'Extracción · Scout', combiner: 'Consolidación · Combiner', clasificador: 'Análisis IA · Clasificador' };
export const scoutStateNames: Record<ScoutRun['status'], string> = { ejecutando: 'En ejecución', exitoso: 'Completado', parcial: 'Parcial', error: 'Error', desconocido: 'Estado no reconocido' };
export const scoutAvailabilityNames: Record<ScoutAvailability, string> = { available: 'Disponible', missing: 'Sin reporte publicado', unreadable: 'No se pudo leer el reporte', invalid: 'Reporte inválido o fuera de límites', changed: 'Reporte cambiando; vuelve a consultar' };
export const scoutReasons: Record<string, string> = {
  fases_pendientes: 'Quedan páginas o detalles pendientes', heuristica_10_vencidos: 'Recorrido detenido por diez vencidos consecutivos; cobertura parcial', heuristica_10_vencidos_auditada: 'Recorrido detenido por diez vencidos y auditoría adicional; cobertura parcial',
  ciclo_diario_completo: 'Ciclo diario completo', etapas_pendientes: 'Quedan etapas pendientes', recuperacion_anterior_pendiente: 'Recuperación del ciclo anterior pendiente', combinacion_pendiente: 'Consolidación pendiente', error_pipeline: 'Error en la ejecución general', fuera_de_ventana: 'Fuera del horario de ejecución', recuperado_desde_journal: 'Recuperado desde el registro de trabajo', '10_vencidos_consecutivos': 'Recorrido detenido tras diez vencidos consecutivos', fin_resultados_verificado: 'Fin de resultados verificado', limite_horario: 'Se alcanzó el límite horario', error_escaneo: 'Error de extracción', maestro_actualizado: 'Maestro actualizado', error_combinacion: 'Error de consolidación', analisis_completo: 'Análisis completo', analisis_pendiente: 'Análisis pendiente', error_clasificacion: 'Error de clasificación', other: 'Otro motivo registrado por Scout',
};
export function scoutMetricName(stage: ScoutStage, key: string): string {
  if (stage === 'scout' && key === 'extracted') return 'Procesos vigentes guardados en la ronda';
  if (stage === 'scout' && key === 'pages') return 'Páginas guardadas en la ronda';
  if (stage === 'clasificador' && key === 'total') return 'Procesos revisados';
  const labels: Record<string, string> = { total: 'Procesos en el maestro', pages: 'Páginas recorridas', extracted: 'Procesos extraídos', expired: 'Vencidos encontrados', consecutive_expired: 'Vencidos consecutivos', known_rechecked: 'Procesos conocidos revisados', http_requests: 'Consultas al sitio', http_retries: 'Reintentos al sitio', pending: 'Pendientes', detail_errors: 'Detalles con errores', inserted: 'Nuevos en el maestro', changed: 'Actualizados en el maestro', unchanged: stage === 'clasificador' ? 'Ya analizados, sin cambios' : 'Sin cambios', removed_expired: 'Retirados del maestro por vencimiento', removed_retention: 'Retirados del maestro por retención', suppressed_retired: 'Versiones retiradas omitidas', analyzed: 'Analizados con IA', reused: 'Análisis recuperados de caché', errors: 'Errores de análisis', input_tokens: 'Tokens de entrada', output_tokens: 'Tokens de salida' };
  return labels[key] ?? 'Contador';
}
export function scoutIssueText(issue: ScoutIssue): string {
  const labels: Record<string, string> = {
    discovery_budget: 'Se agotó el tiempo asignado a recorrer el listado. La extracción continuó con los enlaces ya guardados.',
    invocation_budget: 'Se alcanzó el límite horario o de duración de este intento. El trabajo guardado se conserva para continuar.',
    timeout: 'HonduCompras no respondió dentro del tiempo de espera.',
    connection: 'No se pudo completar la conexión con HonduCompras.',
    invalid_listing: 'La respuesta de HonduCompras no contenía un listado de procesos válido.',
    invalid_detail: 'La respuesta de HonduCompras no contenía el detalle del proceso.',
    incomplete_detail: 'El detalle carece de campos obligatorios, especificaciones útiles o grilla de productos.',
    repeated_page: 'HonduCompras devolvió una página repetida y la paginación no avanzó.',
    consecutive_details: 'La extracción se pausó tras varios fallos consecutivos al consultar detalles.',
    unrecognized: 'Scout registró un fallo cuyo tipo no está identificado en este reporte.',
  };
  if (issue.code === 'http' && issue.http_status !== null && Number.isInteger(issue.http_status) && issue.http_status >= 400 && issue.http_status <= 599) return `HonduCompras respondió con HTTP ${issue.http_status}.`;
  return labels[issue.code] ?? labels.unrecognized!;
}
export const scoutAttemptMetrics: Record<string, string> = {
  extracted: 'Procesos vigentes completos extraídos en este intento', incomplete: 'Detalles incompletos recibidos',
  failed: 'Consultas de detalles fallidas', interrupted: 'Consultas interrumpidas por tiempo',
  details_attempted: 'Detalles intentados en esta ejecución', expired: 'Detalles vencidos encontrados',
  pages_read: 'Lecturas de páginas en este intento, incluidas relecturas',
};
export const scoutDate = (value: string | null) => value ? new Intl.DateTimeFormat('es-HN', { timeZone: 'America/Tegucigalpa', dateStyle: 'medium', timeStyle: 'medium', hourCycle: 'h23' }).format(new Date(value)) : 'No informado';
