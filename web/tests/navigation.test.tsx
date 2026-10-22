import { test } from 'node:test';
import assert from 'node:assert/strict';
import { renderToStaticMarkup } from 'react-dom/server';
import { mainNavigation, settingsSections } from '../src/navigation.js';
import { SettingsNavigation } from '../src/pages/Settings.js';
import type { Access } from '../src/types.js';

const ordinary: Access = { is_system_administrator: false, modules: [], permissions: [] };
test('main navigation groups all account and administration screens under Ajustes', () => {
  const admin = { ...ordinary, is_system_administrator: true };
  assert.deepEqual(mainNavigation(admin).map(entry => entry.id), ['inicio', 'ajustes']);
  assert.deepEqual(settingsSections(admin).map(entry => entry.label), ['Mi cuenta', 'Personas', 'Usuarios', 'Roles y permisos', 'Core']);
  const markup = renderToStaticMarkup(<SettingsNavigation access={admin} section="personas" select={() => {}} />);
  assert.ok(markup.includes('aria-label="Secciones de Ajustes"'));
  assert.ok(markup.includes('aria-current="page">Personas'));
});
test('ordinary users can reach Ajustes and Mi cuenta without administrative sections', () => {
  assert.deepEqual(mainNavigation(ordinary).map(entry => entry.id), ['inicio', 'ajustes']);
  assert.deepEqual(settingsSections(ordinary).map(entry => entry.id), ['cuenta']);
  const markup = renderToStaticMarkup(<SettingsNavigation access={ordinary} section="cuenta" select={() => {}} />);
  for (const restricted of ['Personas', 'Usuarios', 'Roles y permisos', '>Core<']) assert.ok(!markup.includes(restricted));
});
test('Personas keeps its read permission and enabled modules remain in main navigation', () => {
  const access: Access = { ...ordinary, modules: [{ module_id: 'procesos', display_name: 'Procesos' }], permissions: [{ module_id: 'core', scope_key: 'personas', capability: 'ver' }] };
  assert.deepEqual(mainNavigation(access).map(entry => entry.id), ['inicio', 'procesos', 'ajustes']);
  assert.deepEqual(settingsSections(access).map(entry => entry.id), ['cuenta', 'personas']);
  assert.deepEqual(settingsSections({ ...access, permissions: [{ module_id: 'core', scope_key: 'personas', capability: 'editar' }] }).map(entry => entry.id), ['cuenta']);
});

test('Cotizaciones requires its enabled catalog entry and document read permission', () => {
  const access: Access = { ...ordinary, modules: [{ module_id: 'cotizaciones', display_name: 'Cotizaciones' }], permissions: [] };
  assert.ok(!mainNavigation(access).some(entry => entry.id === 'cotizaciones'));
  access.permissions = [{ module_id: 'cotizaciones', scope_key: 'documentos', capability: 'ver' }];
  assert.deepEqual(mainNavigation(access).map(entry => entry.id), ['inicio', 'cotizaciones', 'ajustes']);
  assert.deepEqual(settingsSections(access).map(entry => entry.id), ['cuenta', 'cotizaciones']);
  assert.ok(!mainNavigation({ ...access, modules: [] }).some(entry => entry.id === 'cotizaciones'));
  assert.ok(mainNavigation({ ...access, is_system_administrator: true, permissions: [] }).some(entry => entry.id === 'cotizaciones'));
});
