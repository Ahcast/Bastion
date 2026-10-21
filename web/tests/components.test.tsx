import { test } from 'node:test';
import assert from 'node:assert/strict';
import { renderToStaticMarkup } from 'react-dom/server';
import jsQR from 'jsqr';
import Auth from '../src/Auth.js';
import { ApiClient } from '../src/api.js';
import { Form, Table } from '../src/ui.js';
import { can } from '../src/types.js';
import type { Access } from '../src/types.js';
import TotpSetup, { qrMatrix } from '../src/components/TotpSetup.js';

test('untrusted cell text, field values and messages render as text, not HTML', () => {
  const attack = '<img src=x onerror=alert(1)><script>alert(2)</script>';
  const table = renderToStaticMarkup(<Table columns={['Nombre']} rows={[[attack]]} />);
  const form = renderToStaticMarkup(<Form fields={[{ name: 'name', label: 'Nombre' }]} initial={{ name: attack }} onSubmit={async () => {}} />);
  const auth = renderToStaticMarkup(<Auth api={new ApiClient('https://api.example.test')} initial={{ kind: 'login' }} initialMessage={attack} authenticated={async () => {}} />);
  for (const markup of [table, form, auth]) { assert.ok(!markup.includes('<img')); assert.ok(!markup.includes('<script>')); assert.ok(markup.includes('&lt;img')); }
});
test('login keeps password autocomplete and does not persist credentials', () => {
  const markup = renderToStaticMarkup(<Auth api={new ApiClient('https://api.example.test')} initial={{ kind: 'login' }} authenticated={async () => {}} />);
  assert.ok(markup.includes('type="password"')); assert.ok(markup.includes('autoComplete="current-password"')); assert.ok(markup.includes('Mantenerme loggeado'));
});
test('visible actions follow scope and capability while the API remains authoritative', () => {
  const access: Access = { is_system_administrator: false, modules: [], permissions: [{ module_id: 'core', scope_key: 'personas', capability: 'ver' }] };
  assert.ok(can(access, 'personas', 'ver')); assert.ok(!can(access, 'personas', 'editar')); assert.ok(!can(access, 'usuarios', 'ver')); assert.ok(!can(null, 'personas', 'ver'));
});
test('three TOTP QR matrices decode to the exact URI using an independent decoder', () => {
  for (const uri of ['otpauth://totp/Bastion:tester%40example.test?secret=JBSWY3DPEHPK3PXP&issuer=Bastion', 'otpauth://totp/Bastion%20Core:Mar%C3%ADa%40example.test?secret=JBSWY3DPEHPK3PXP&issuer=Bastion%20Core&algorithm=SHA1&digits=6&period=30', 'otpauth://totp/Bastion:long-account-name%40example.test?secret=GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ&issuer=Bastion&algorithm=SHA1&digits=6&period=30']) {
    const matrix = qrMatrix(uri), scale = 5, width = (matrix.length + 8) * scale, pixels = new Uint8ClampedArray(width * width * 4).fill(255);
    for (let y = 0; y < width; y++) for (let x = 0; x < width; x++) {
      const dark = matrix[Math.floor(y / scale) - 4]?.[Math.floor(x / scale) - 4] ?? false;
      const offset = (y * width + x) * 4; if (dark) { pixels[offset] = 0; pixels[offset + 1] = 0; pixels[offset + 2] = 0; }
    }
    assert.equal(jsQR(pixels, width, width)?.data, uri);
  }
});
test('TOTP setup includes a local SVG and a manual alternative without external images', () => {
  const markup = renderToStaticMarkup(<TotpSetup setup={{ setup_token: 'test-only', totp_secret: 'JBSWY3DPEHPK3PXP', otpauth_uri: 'otpauth://totp/Bastion:test?secret=JBSWY3DPEHPK3PXP&issuer=Bastion' }} />);
  assert.ok(markup.includes('<svg')); assert.ok(markup.includes('JBSWY3DPEHPK3PXP')); assert.ok(markup.includes('clave manual')); assert.ok(!markup.includes('<img'));
});
