// Real Chromium rendering with synthetic API responses; no production writes.
const assert = require('node:assert/strict');
const fs = require('node:fs/promises');
const http = require('node:http');
const path = require('node:path');
const { chromium } = require('playwright');
const root = path.resolve(__dirname, '../dist');
const output = path.resolve(__dirname, '../../output/android-qa');
const types = { '.html': 'text/html', '.js': 'text/javascript', '.css': 'text/css', '.svg': 'image/svg+xml', '.woff2': 'font/woff2' };
const server = http.createServer(async (req, res) => {
  const name = decodeURIComponent(new URL(req.url, 'http://localhost').pathname);
  const file = path.resolve(root, '.' + (name === '/' ? '/index.html' : name));
  if (!file.startsWith(root + path.sep)) { res.writeHead(403).end(); return; }
  try { res.setHeader('Content-Type', types[path.extname(file)] || 'application/octet-stream'); res.end(await fs.readFile(file)); }
  catch { res.writeHead(404).end(); }
});
async function main() {
  await fs.mkdir(output, { recursive: true });
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  const browser = await chromium.launch();
  const evidence = [];
  try {
    for (const width of [320, 360, 390, 430, 1280]) {
      const page = await browser.newPage({ viewport: { width, height: 800 }, isMobile: width < 640, hasTouch: width < 640 });
      const errors = [];
      page.on('pageerror', error => errors.push(error.message));
      let authenticated = false;
      await page.route('https://api.bastion.castilloit.net/**', async route => {
        const endpoint = new URL(route.request().url()).pathname;
        let status = 200, data;
        if (endpoint.endsWith('/auth/csrf')) data = { csrf_token: 'synthetic-csrf' };
        else if (endpoint.endsWith('/bootstrap/status')) data = { setup_required: false };
        else if (endpoint.endsWith('/auth/me')) { status = authenticated ? 200 : 401; data = authenticated ? { user: { id: 1, name: 'Prueba móvil', email: 'qa@example.invalid', role: 'Pruebas' } } : { error: { message: 'No session' } }; }
        else if (endpoint.endsWith('/auth/login')) { authenticated = true; data = { status: 'authenticated' }; }
        else if (endpoint.endsWith('/auth/access')) data = { is_system_administrator: true, modules: [{ module_id: 'clientes', display_name: 'Clientes' }], permissions: [] };
        else if (endpoint === '/api/v1/clientes') data = { items: [], page: 1, page_size: 25, total: 0 };
        else { status = 404; data = { error: { message: 'No synthetic response for ' + endpoint } }; }
        await route.fulfill({ status, contentType: 'application/json', body: JSON.stringify(data), headers: { 'Access-Control-Allow-Origin': `http://127.0.0.1:${server.address().port}`, 'Access-Control-Allow-Credentials': 'true' } });
      });
      const check = async name => {
        const layout = await page.evaluate(() => ({ viewport: innerWidth, width: document.documentElement.scrollWidth }));
        assert.ok(layout.width <= layout.viewport + 1, `${name} overflows at ${width}: ${JSON.stringify(layout)}`);
        assert.deepEqual(errors, []);
        await page.screenshot({ path: path.join(output, `${width}-${name}.png`), fullPage: true });
        evidence.push({ width, screen: name, ...layout });
      };
      await page.goto(`http://127.0.0.1:${server.address().port}/`);
      await page.locator('input[name=email]').waitFor();
      await check('login');
      await page.locator('input[name=email]').fill('qa@example.invalid');
      await page.locator('input[name=password]').fill('synthetic-password');
      await page.locator('form button[type=submit]').click();
      await page.locator('.topbar').waitFor();
      if (width < 640) {
        await page.getByRole('button', { name: 'Mostrar menú lateral' }).click();
        await check('navigation');
        assert.equal(await page.locator('.main').evaluate(el => el.inert), true);
      }
      await page.locator('.sidebar').getByRole('button', { name: 'Clientes', exact: true }).click();
      await page.getByRole('heading', { name: 'Clientes', exact: true }).waitFor();
      await check('clients');
      await page.getByRole('button', { name: 'Nuevo', exact: true }).click();
      await page.locator('input[name=cliente_name]').waitFor();
      await check('client-form');
      if (width < 640) {
        const controls = await page.locator('input:not([type=checkbox]),select,textarea').evaluateAll(nodes => nodes.filter(el => el.getBoundingClientRect().height).map(el => ({ height: el.getBoundingClientRect().height, font: parseFloat(getComputedStyle(el).fontSize) })));
        assert.ok(controls.every(el => el.height >= 44 && el.font >= 16), JSON.stringify(controls));
        await page.setViewportSize({ width, height: 400 });
        await check('keyboard-height');
      }
      await page.close();
    }
    await fs.writeFile(path.join(output, 'layout.json'), JSON.stringify(evidence, null, 2));
    console.log(`Verified ${evidence.length} layouts, navigation and touch inputs.`);
  } finally { await browser.close(); server.close(); }
}
main().catch(error => { console.error(error); server.close(); process.exitCode = 1; });
