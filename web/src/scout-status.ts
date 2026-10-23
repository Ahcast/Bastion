export type ScoutStage = 'main' | 'scout' | 'combiner' | 'clasificador';
export type ScoutAvailability = 'available' | 'missing' | 'unreadable' | 'invalid' | 'changed';
export interface ScoutRun {
  run_id: string; status: 'ejecutando' | 'exitoso' | 'parcial' | 'error' | 'desconocido';
  started_at: string | null; finished_at: string | null; deadline: string | null;
  duration_seconds: number | null; output_ready: boolean | null; reason: string | null;
  retry_stages: ScoutStage[]; metrics: Record<string, number>; error_reported: boolean;
}
export interface ScoutReport { stage: ScoutStage; availability: ScoutAvailability; matches_current_run: boolean | null; run: ScoutRun | null }
export interface ScoutDiagnostics { checked_at: string; reports: ScoutReport[] }
export const scoutStageNames: Record<ScoutStage, string> = { main: 'Ejecución general', scout: 'Extracción · Scout', combiner: 'Consolidación · Combiner', clasificador: 'Análisis IA · Clasificador' };
export const scoutStateNames: Record<ScoutRun['status'], string> = { ejecutando: 'En ejecución', exitoso: 'Completado', parcial: 'Parcial', error: 'Error', desconocido: 'Estado no reconocido' };
export const scoutAvailabilityNames: Record<ScoutAvailability, string> = { available: 'Disponible', missing: 'Sin reporte publicado', unreadable: 'No se pudo leer el reporte', invalid: 'Reporte inválido o fuera de límites', changed: 'Reporte cambiando; vuelve a consultar' };
export const scoutReasons: Record<string, string> = {
  ciclo_diario_completo: 'Ciclo diario completo', etapas_pendientes: 'Quedan etapas pendientes', recuperacion_anterior_pendiente: 'Recuperación del ciclo anterior pendiente', combinacion_pendiente: 'Consolidación pendiente', error_pipeline: 'Error en la ejecución general', fuera_de_ventana: 'Fuera del horario de ejecución', recuperado_desde_journal: 'Recuperado desde el registro de trabajo', '10_vencidos_consecutivos': 'Recorrido detenido tras diez vencidos consecutivos', fin_resultados_verificado: 'Fin de resultados verificado', limite_horario: 'Se alcanzó el límite horario', error_escaneo: 'Error de extracción', maestro_actualizado: 'Maestro actualizado', error_combinacion: 'Error de consolidación', analisis_completo: 'Análisis completo', analisis_pendiente: 'Análisis pendiente', error_clasificacion: 'Error de clasificación', other: 'Otro motivo registrado por Scout',
};
export function scoutMetricName(stage: ScoutStage, key: string): string {
  const labels: Record<string, string> = { total: 'Procesos en el maestro', pages: 'Páginas recorridas', extracted: 'Procesos extraídos', expired: 'Vencidos encontrados', consecutive_expired: 'Vencidos consecutivos', known_rechecked: 'Procesos conocidos revisados', http_requests: 'Consultas al sitio', http_retries: 'Reintentos al sitio', pending: 'Pendientes', detail_errors: 'Detalles con errores', inserted: 'Nuevos en el maestro', changed: 'Actualizados en el maestro', unchanged: stage === 'clasificador' ? 'Ya analizados, sin cambios' : 'Sin cambios', removed_expired: 'Retirados del maestro por vencimiento', removed_retention: 'Retirados del maestro por retención', suppressed_retired: 'Versiones retiradas omitidas', analyzed: 'Analizados con IA', reused: 'Análisis recuperados de caché', errors: 'Errores de análisis', input_tokens: 'Tokens de entrada', output_tokens: 'Tokens de salida' };
  return labels[key] ?? 'Contador';
}
export const scoutDate = (value: string | null) => value ? new Intl.DateTimeFormat('es-HN', { timeZone: 'America/Tegucigalpa', dateStyle: 'medium', timeStyle: 'medium', hourCycle: 'h23' }).format(new Date(value)) : 'No informado';
