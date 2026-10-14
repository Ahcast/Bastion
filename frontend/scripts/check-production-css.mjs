import { readFileSync, existsSync } from 'node:fs';
import { resolve } from 'node:path';
const directory = resolve('dist/bastion-ui/browser');
const html = readFileSync(resolve(directory, 'index.html'), 'utf8');
const links = html.match(/<link\b[^>]*rel="stylesheet"[^>]*>/g) || [];
if (!links.length || !links.some(link => !/media="(?!all")/.test(link))) {
  throw new Error('Production CSS must load directly without inline JavaScript under script-src self.');
}
for (const link of links) {
  const href = /href="([^"]+)"/.exec(link)?.[1];
  if (!href || !existsSync(resolve(directory, href))) throw new Error('Missing production CSS asset: ' + href);
}
if (/<script\b(?![^>]*\bsrc=)[^>]*>\s*\S/.test(html) || html.includes('data-beasties-media')) {
  throw new Error('Production index depends on inline JavaScript blocked by the hosting CSP.');
}
console.log('Production CSS loads directly; no inline activation script.');
