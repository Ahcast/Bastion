import { test } from 'node:test';
import assert from 'node:assert/strict';
import { ApiClient } from '../src/api.js';
import { openGeneratedPdf, reserveGeneratedPdfViewer } from '../src/generatedPdf.js';

test('generated PDFs open the authenticated API response without a Blob or an extra download action', () => {
  const previous = Object.getOwnPropertyDescriptor(globalThis, 'window');
  const target = { closed: false, opener: {} as unknown, document: { title: '', body: { textContent: '' } }, location: { href: 'about:blank', replace(value: string) { this.href = value; } }, close() { this.closed = true; } };
  Object.defineProperty(globalThis, 'window', { configurable: true, value: { open: () => target } });
  const api = new ApiClient('https://api.example.test', async () => { throw new Error('PDF navigation must use the browser'); });
  try {
    openGeneratedPdf(api, '/api/v1/cotizaciones/17/pdf/vista?version=3');
    assert.equal(target.opener, null);
    assert.equal(target.location.href, 'https://api.example.test/api/v1/cotizaciones/17/pdf/vista?version=3');
    assert.equal(target.closed, false);
  } finally {
    if (previous) Object.defineProperty(globalThis, 'window', previous); else Reflect.deleteProperty(globalThis, 'window');
  }
});

test('reserved PDF tabs respect user navigation and close only the pending tab on failure', () => {
  const previous = Object.getOwnPropertyDescriptor(globalThis, 'window');
  const target = { closed: false, opener: {} as unknown, document: { title: '', body: { textContent: '' } }, location: { href: 'about:blank', replace(value: string) { this.href = value; } }, close() { this.closed = true; } };
  Object.defineProperty(globalThis, 'window', { configurable: true, value: { open: () => target } });
  const api = new ApiClient('https://api.example.test');
  try {
    const viewer = reserveGeneratedPdfViewer(api);
    target.location.href = 'https://example.test/elsewhere';
    viewer.show('/api/v1/facturas/1/documents/invoice/vista?version=4');
    viewer.close();
    assert.equal(target.location.href, 'https://example.test/elsewhere');
    assert.equal(target.closed, false);
    target.location.href = 'about:blank';
    assert.throws(() => openGeneratedPdf(api, 'https://untrusted.example/pdf'));
    assert.equal(target.closed, true);
    Object.defineProperty(globalThis, 'window', { configurable: true, value: { open: () => null } });
    assert.throws(() => reserveGeneratedPdfViewer(api), /bloqueó/);
  } finally {
    if (previous) Object.defineProperty(globalThis, 'window', previous); else Reflect.deleteProperty(globalThis, 'window');
  }
});
