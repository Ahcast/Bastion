import { test } from 'node:test';
import assert from 'node:assert/strict';
import { renderToStaticMarkup } from 'react-dom/server';
import { ScoutStatusPanel } from '../src/components/ScoutStatusPanel.js';
import { settingsSections } from '../src/navigation.js';
import type { ScoutDiagnostics, ScoutRun } from '../src/scout-status.js';
import type { Access } from '../src/types.js';

const run: ScoutRun = { run_id: '2026-10-22', status: 'parcial', started_at: '2026-10-23T01:00:00Z', finished_at: null, deadline: null, duration_seconds: null, output_ready: false, reason: 'analisis_pendiente', retry_stages: ['clasificador'], metrics: { analyzed: 0, reused: 3, pending: 2, input_tokens: 120 }, error_reported: true };
const data: ScoutDiagnostics = { checked_at: '2026-10-23T01:10:00Z', reports: [{ stage: 'main', availability: 'available', matches_current_run: true, run }, { stage: 'scout', availability: 'available', matches_current_run: false, run: { ...run, run_id: '2026-10-21', status: 'exitoso', metrics: {} } }, { stage: 'combiner', availability: 'missing', matches_current_run: null, run: null }, { stage: 'clasificador', availability: 'available', matches_current_run: true, run }] };

test('Scout status shows missing reports, previous cycles, IA counts and unknown times without faking success', () => {
  const markup = renderToStaticMarkup(<ScoutStatusPanel data={data} />);
  for (const text of ['Última ejecución reportada', 'Análisis IA', 'Tokens de entrada', 'Analizados con IA', 'Pendientes', 'otro ciclo', 'Sin reporte publicado', 'No informado', 'No informada', '19:00', '22 oct', 'Todavía no está lista']) assert.ok(markup.includes(text), text);
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
