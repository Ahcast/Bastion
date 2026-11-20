import { after, test } from 'node:test';
import assert from 'node:assert/strict';
import { Window } from 'happy-dom';
import { act } from 'react';
import { createRoot } from 'react-dom/client';
import { NavigationLink } from '../src/components/NavigationLink.js';
import { NavigationGuardProvider, useNavigationGuard, useUnsavedChanges } from '../src/navigationGuard.js';

const window = new Window({ url: 'https://app.example.test/' });
for (const name of ['document', 'HTMLElement', 'MouseEvent', 'KeyboardEvent'] as const) Object.defineProperty(globalThis, name, { configurable: true, value: window[name] });
Object.assign(globalThis, { window, IS_REACT_ACT_ENVIRONMENT: true });
after(() => window.happyDOM.abort());

test('navigation links expose canonical destinations and preserve native new-tab gestures without abandoning drafts', async () => {
  let navigated = 0;
  function Fixture() {
    const { navigate } = useNavigationGuard();
    useUnsavedChanges(true);
    return <NavigationLink route={{ page: 'ordenes_compra', values: {} }} onNavigate={() => { void navigate(() => navigated++); }}>Órdenes</NavigationLink>;
  }
  const container = document.createElement('div'); document.body.append(container); const root = createRoot(container);
  try {
    await act(async () => root.render(<NavigationGuardProvider><Fixture /></NavigationGuardProvider>));
    const link = container.querySelector('a')!;
    assert.equal(link.getAttribute('href'), '/ordenes-compra');
    assert.equal(link.getAttribute('target'), null);
    for (const options of [{ ctrlKey: true }, { metaKey: true }, { shiftKey: true }, { altKey: true }, { button: 1 }, { button: 2 }]) {
      const event = new MouseEvent('click', { bubbles: true, cancelable: true, ...options });
      // Capture cancellation before preventing Happy DOM from following the link.
      let prevented = true;
      container.addEventListener('click', e => { prevented = e.defaultPrevented; e.preventDefault(); }, { once: true });
      await act(async () => link.dispatchEvent(event));
      assert.equal(prevented, false);
      assert.equal(navigated, 0);
      assert.equal(document.querySelector('[role=dialog]'), null);
    }
    await act(async () => link.dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true })));
    assert.ok(document.body.textContent?.includes('Cambios sin guardar'));
    assert.equal(navigated, 0);
    const discard = Array.from(document.querySelectorAll('button')).find(b => b.textContent === 'Descartar y continuar')!;
    await act(async () => discard.click());
    assert.equal(navigated, 1);
  } finally { await act(async () => root.unmount()); container.remove(); }
});

test('an explicit new-window target never invokes the same-tab callback', async () => {
  let navigated = 0;
  const container = document.createElement('div'); document.body.append(container); const root = createRoot(container);
  try {
    await act(async () => root.render(<NavigationLink target="_blank" rel="noopener" route={{ page: 'ajustes', section: 'cuenta', values: {} }} onNavigate={() => navigated++}>Mi cuenta</NavigationLink>));
    const link = container.querySelector('a')!;
    assert.equal(link.getAttribute('href'), '/ajustes/cuenta');
    let prevented = true;
    container.addEventListener('click', e => { prevented = e.defaultPrevented; e.preventDefault(); }, { once: true });
    await act(async () => link.dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true })));
    assert.equal(prevented, false); assert.equal(navigated, 0);
  } finally { await act(async () => root.unmount()); container.remove(); }
});
