import { test } from 'node:test';
import assert from 'node:assert/strict';
import { renderToStaticMarkup } from 'react-dom/server';
import { exactCents, formatCents, entryTotals, entryInput, newEntry } from '../src/accounting.js';
import { ApiClient } from '../src/api.js';
import { CoreContext } from '../src/ui.js';
import { NavigationGuardProvider } from '../src/navigationGuard.js';
import Contabilidad from '../src/pages/Contabilidad.js';
import { mainNavigation } from '../src/navigation.js';
import type { Access } from '../src/types.js';

test('accounting preserves decimal cents and rejects ambiguous or overflowing input', () => {
  for (const input of ['-1', '1.001', '1e2', '1,000', ' 1', '1.', '.5', '1000000000000']) assert.equal(exactCents(input), null);
  assert.equal(exactCents('999999999999.99'), 99999999999999n);
  assert.equal(exactCents('0.1'), 10n);
  assert.equal(formatCents(-101n), '-1.01');
  const lines = [{ account:'1102', debit:'0.10', credit:'0' }, { account:'1102', debit:'0.20', credit:'0' }, { account:'3101', debit:'0', credit:'0.30' }];
  assert.equal(entryTotals(lines).valid, true);
  assert.equal(entryTotals([...lines, {account:'1102',debit:'1',credit:'1'}]).valid, false);
  assert.equal(entryTotals(lines.slice(1)).difference, '-0.10');
  assert.equal(entryTotals([{account:'1102',debit:'999999999999.99',credit:'0'},{account:'1102',debit:'0.01',credit:'0'},{account:'3101',debit:'0',credit:'999999999999.99'},{account:'3101',debit:'0',credit:'0.01'}]).valid, false);
});

test('draft serialization excludes server state and preserves reviewed lines', () => {
  const data = { ...newEntry(), id:7, status:'contabilizado', version:3, total:'1.00' };
  const serialized = entryInput(data);
  assert.equal('id' in serialized, false);
  assert.equal('status' in serialized, false);
  assert.equal('total' in serialized, false);
  assert.notEqual(serialized.lines, data.lines);
});

test('accounting navigation and creation require activation and independent read/write permission', () => {
  for (const [active, edit] of [[true, true], [true, false], [false, true]]) {
    const access: Access = { is_system_administrator:false, modules:active ? [{module_id:'contabilidad',display_name:'Contabilidad'}] : [], permissions:[{module_id:'contabilidad',scope_key:'libros',capability:'ver'}, ...(edit ? [{module_id:'contabilidad',scope_key:'libros',capability:'editar'}] : [])] };
    const html = renderToStaticMarkup(<CoreContext.Provider value={{api:new ApiClient('https://example.test'),access,notify:()=>{}}}><NavigationGuardProvider><Contabilidad/></NavigationGuardProvider></CoreContext.Provider>);
    assert.equal(html.includes('Nuevo asiento'), active && edit);
    assert.equal(html.includes('No tienes acceso'), !active);
    assert.equal(mainNavigation(access).some(p => p.id === 'contabilidad'), active);
  }
});
