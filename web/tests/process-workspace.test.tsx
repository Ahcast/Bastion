import { test } from 'node:test';
import assert from 'node:assert/strict';
import { renderToStaticMarkup } from 'react-dom/server';
import { ProcessDetailContent, ProcessFilterBar, ProcessInsights, ProcessTable } from '../src/components/ProcessWorkspace.js';
import { initialProcessFilters, nextProcessExpiry, processQuery } from '../src/processes.js';
import type { ProcessRecord } from '../src/processes.js';

const attack = '<img src=x onerror=alert(1)>';
const row: ProcessRecord = { proceso_id: 'hc:' + 'a'.repeat(64), expediente: 'EXP-001', entidad: 'Secretaría de Salud', fecha_recepcion: '2026-10-23T05:59:59Z', puntaje: 87, rubro: 'Instrumentación', resumen_corto: 'Equipos para laboratorio', source: { descripcion: attack, items: '1. Multímetro\n2. Cable', url: 'https://example.test' }, marked: true, note: 'Compartida', version: 2 };
test('process detail expands directly below its row without a dialog and escapes source text', () => {
  const render = (selected: string | null) => renderToStaticMarkup(<ProcessTable rows={[row]} selected={selected} select={() => {}} mark={async () => {}} canMark={true} busy={false} detail={record => <ProcessDetailContent row={record} />} />);
  const expanded = render(row.proceso_id), closed = render(null);
  assert.ok(expanded.includes('<tr class="process-detail-row"><td colSpan="7">'));
  for (const label of ['Calificación IA', 'Rubro', 'Resumen', 'Descripción / especificaciones', 'Ítems analizados', 'Multímetro', 'Cable', 'Compartida']) assert.ok(expanded.includes(label));
  assert.ok(expanded.includes('aria-expanded="true"'));
  assert.ok(expanded.includes('&lt;img'));
  assert.ok(!expanded.includes('<img'));
  assert.ok(!expanded.includes('role="dialog"'));
  assert.ok(!closed.includes('process-detail-row'));
  assert.ok(closed.includes('aria-expanded="false"'));
  // UTC next-day 05:59 is still the previous Honduras day.
  assert.ok(expanded.includes('22 oct'));
});
test('compact filters share the same exact query for list and export', () => {
  const values = { ...initialProcessFilters, entidad: 'Salud & Bienestar', q: '  50%_equipo  ', marked: true, hide_municipalities: true, sort: 'puntaje_desc' as const };
  const query = new URLSearchParams(processQuery(values));
  assert.equal(query.get('entidad'), 'Salud & Bienestar');
  assert.equal(query.get('q'), '50%_equipo');
  assert.equal(query.get('marked'), 'true');
  assert.equal(query.get('hide_municipalities'), 'true');
  assert.equal(query.get('sort'), 'puntaje_desc');
  assert.equal(new URLSearchParams(processQuery(initialProcessFilters)).get('marked'), null);
  const markup = renderToStaticMarkup(<ProcessFilterBar values={values} entities={['Salud & Bienestar']} change={() => {}} />);
  assert.equal((markup.match(/type="checkbox"/g) ?? []).length, 2);
  assert.equal((markup.match(/<select/g) ?? []).length, 2);
  assert.ok(markup.includes('Especificaciones o expediente'));
  assert.ok(!markup.includes('Recepción desde'));
});
test('statistics show explicit zero counts, calendar dates and the three delivery days', () => {
  const markup = renderToStaticMarkup(<ProcessInsights status={{ total: 0, last_import_at: null, as_of: '2026-10-22', timezone: 'America/Tegucigalpa', weekly: [{ week_start: '2026-10-19', count: 0 }], due: [{ date: '2026-10-22', count: 0 }, { date: '2026-10-23', count: 2 }, { date: '2026-10-24', count: 1 }] }} />);
  for (const label of ['Procesos nuevos por semana', 'Hoy', 'Mañana', 'Pasado mañana', '22 oct', '23 oct', '24 oct']) assert.ok(markup.includes(label));
  assert.ok(markup.includes('<strong>0</strong>'));
  assert.ok(!markup.includes('NaN'));
  assert.ok(!markup.includes('Infinity'));
  assert.ok(markup.includes('Primera importación en Bastion'));
  assert.ok(markup.includes('0 procesos importados'));
  assert.ok(!markup.includes('Resultados filtrados'));
});
test('expiry uses the server instant, including equality, and accounts for request delay', () => {
  const server_time = '2026-08-01T19:30:00Z'; // 13:30 Honduras.
  assert.equal(nextProcessExpiry({ server_time, items: [{ ...row, fecha_recepcion: '2026-08-01T13:29:00-06:00' }] }, 0), 0);
  assert.equal(nextProcessExpiry({ server_time, items: [{ ...row, fecha_recepcion: server_time }] }, 0), 0);
  assert.equal(nextProcessExpiry({ server_time, items: [{ ...row, fecha_recepcion: '2026-08-01T13:31:00-06:00' }] }, 0), 60_000);
  assert.equal(nextProcessExpiry({ server_time, items: [{ ...row, fecha_recepcion: '2026-08-01T13:31:00-06:00' }] }, 1000), 59_000);
  assert.equal(nextProcessExpiry({ server_time, items: [] }, 0), null);
});
