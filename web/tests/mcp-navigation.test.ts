import { test } from 'node:test';
import assert from 'node:assert/strict';
import { takeMcpQuery } from '../src/mcpNavigation.js';

test('consent fragment is retained only in memory and removed from history', () => {
  let replaced = '';
  const original = { pathname: '/oauth/consent', search: '', hash: '#client_id=test&state=opaque' };
  assert.equal(takeMcpQuery(false, original, path => { replaced = path; }), null);
  assert.equal(replaced, '');
  assert.equal(takeMcpQuery(true, original, path => { replaced = path; }), 'client_id=test&state=opaque');
  assert.equal(replaced, '/oauth/consent');
  assert.equal(takeMcpQuery(true, { pathname: '/oauth/consent', search: '?legacy=value', hash: '' }, () => {}), 'legacy=value');
  assert.equal(takeMcpQuery(true, { pathname: '/oauth/consent', search: '?a=b', hash: '#c=d' }, () => {}), '');
  assert.equal(takeMcpQuery(true, { pathname: '/oauth/consent', search: '', hash: '#' + 'x'.repeat(8193) }, () => {}), '');
  assert.equal(takeMcpQuery(true, { pathname: '/other', search: '?x=y' }, () => {}), null);
});
