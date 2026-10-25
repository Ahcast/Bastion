import { test } from 'node:test';
import assert from 'node:assert/strict';
import { renderToStaticMarkup } from 'react-dom/server';
import { ScoutStatusPanel } from '../src/components/ScoutStatusPanel.js';
import { settingsSections } from '../src/navigation.js';
import type { ScoutDiagnostics, ScoutRun } from '../src/scout-status.js';
import { scoutIssueText, scoutMetricName } from '../src/scout-status.js';
import type { Access } from '../src/types.js';

const run: ScoutRun = { run_id: '2026-10-22', status: 'parcial', started_at: '2026-10-23T01:00:00Z', finished_at: null, deadline: null, duration_seconds: null, output_ready: false, reason: 'analisis_pendiente', retry_stages: ['clasificador'], metrics: { analyzed: 0, reused: 3, pending: 2, input_tokens: 120 }, error_reported: true };
const data: ScoutDiagnostics = { checked_at: '2026-10-23T01:10:00Z', reports: [{ stage: 'main', availability: 'available', matches_current_run: true, run }, { stage: 'scout', availability: 'available', matches_current_run: false, run: { ...run, run_id: '2026-10-21', status: 'exitoso', metrics: {} } }, { stage: 'combiner', availability: 'missing', matches_current_run: null, run: null }, { stage: 'clasificador', availability: 'available', matches_current_run: true, run }] };

test('Scout status shows missing reports, previous cycles, IA counts and unknown times without faking success', () => {
  assert.equal(scoutMetricName('clasificador', 'total'), 'Procesos revisados');
  assert.equal(scoutMetricName('combiner', 'total'), 'Procesos en el maestro');
  const markup = renderToStaticMarkup(<ScoutStatusPanel data={data} />);
  for (const text of ['Última ejecución reportada', 'Análisis IA', 'Tokens de entrada', 'Analizados con IA', 'Pendientes', 'otro ciclo', 'Sin reporte publicado', 'No informado', 'No informada', '19:00', '22 oct', 'se pueden incorporar las filas ya listas']) assert.ok(markup.includes(text), text);
  assert.ok(markup.includes('<dd>0</dd>'));
  assert.ok(!markup.includes('type="submit"'));
  assert.ok(!markup.includes('<input'));
  assert.ok(!markup.includes('<form'));
});
test('Scout report identifiers are rendered as text and unknown reason text is not echoed', () => {
  const attack = '<img src=x onerror=alert(1)>';
  const markup = renderToStaticMarkup(<ScoutStatusPanel data={{ ...data, reports: [{ ...data.reports[0]!, run: { ...run, run_id: attack, reason: attack } }] }} />);
  assert.ok(markup.includes('&lt;img'));
  assert.ok(!markup.includes('<img'));
  assert.ok(markup.includes('Otro motivo registrado por Scout'));
});
test('Ajustes > Procesos requires the active module and its existing read permission', () => {
  const access: Access = { is_system_administrator: false, modules: [{ module_id: 'procesos', display_name: 'Procesos' }], permissions: [{ module_id: 'procesos', scope_key: 'consulta', capability: 'ver' }] };
  assert.ok(settingsSections(access).some(entry => entry.id === 'procesos'));
  assert.ok(!settingsSections({ ...access, modules: [] }).some(entry => entry.id === 'procesos'));
  assert.ok(!settingsSections({ ...access, permissions: [] }).some(entry => entry.id === 'procesos'));
});

test('extraction explains the pause, failed page and per-attempt counts separately from the round', () => {
  const extraction: ScoutRun['extraction'] = {
    next_page: 6, discovery_complete: false, failure: { code: 'discovery_budget', http_status: null },
    phases: [{ phase: 'discovery', status: 'pausado', next_retry: '2026-10-25T06:00:00Z', issue: null }],
    invocation: { last_attempted_page: 6, last_saved_page: 5, last_failed_page: 6, last_failure: { code: 'http', http_status: 500 }, metrics: { extracted: 9, failed: 2, incomplete: 0 } },
    detail_issues: [{ issue: { code: 'incomplete_detail', http_status: null }, count: 66 }],
  };
  const markup = renderToStaticMarkup(<ScoutStatusPanel data={{ ...data, reports: [{ stage: 'scout', availability: 'available', matches_current_run: true, run: { ...run, reason: 'fases_pendientes', metrics: { pages: 5, extracted: 501 }, extraction } }] }} />);
  for (const text of ['Quedan páginas o detalles pendientes', 'Se agotó el tiempo asignado', 'Página para continuar', 'Página del último fallo', 'HTTP 500', 'Próximo reintento desde', 'Procesos vigentes completos extraídos en este intento', 'Procesos vigentes guardados en la ronda', 'Fallos en los detalles pendientes', '66', '<dd>9</dd>', '<dd>501</dd>', '<dd>0</dd>']) assert.ok(markup.includes(text), text);
  assert.ok(!markup.includes('Otro motivo registrado'));
  assert.ok(!markup.includes('Scout registró incidencias en esta ejecución'));
});

test('legacy extraction does not claim its round total was extracted in this invocation', () => {
  const markup = renderToStaticMarkup(<ScoutStatusPanel data={{ ...data, reports: [{ stage: 'scout', availability: 'available', matches_current_run: true, run: { ...run, metrics: { pages: 5, extracted: 501 }, extraction: { next_page: 6, discovery_complete: null, failure: null, phases: [], invocation: null, detail_issues: [] } } }] }} />);
  assert.ok(markup.includes('no incluye contadores por intento'));
  assert.ok(markup.includes('acumuladas de la ronda'));
  assert.ok(!markup.includes('Página del último fallo'));
  assert.ok(!markup.includes('Resultado de este intento'));
  assert.ok(!scoutIssueText({ code: '<script>SECRET</script>', http_status: null }).includes('SECRET'));
  assert.ok(!scoutIssueText({ code: 'http', http_status: 999 }).includes('999'));
});
