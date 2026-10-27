import { after, test } from 'node:test';
import assert from 'node:assert/strict';
import { Window } from 'happy-dom';
import { renderToStaticMarkup } from 'react-dom/server';
import { ApiClient } from '../src/api.js';
import { CoreContext } from '../src/ui.js';
import { NavigationGuardProvider } from '../src/navigationGuard.js';
import OrdenesCompra, { OrderEditor, OrderSettings, OrderSummaryTotals } from '../src/pages/OrdenesCompra.js';
import { OrderDocuments } from '../src/components/OrderDocuments.js';
import { newOrder } from '../src/orders.js';
import type { Destination, OrderCalculation, PurchaseOrder } from '../src/orders.js';
import type { Access } from '../src/types.js';
import { mainNavigation, settingsSections } from '../src/navigation.js';

const access: Access = { is_system_administrator: false, modules: [{ module_id: 'ordenes_compra', display_name: 'Órdenes de compra' }], permissions: [{ module_id: 'ordenes_compra', scope_key: 'documentos', capability: 'ver' }] };
const editor: Access = { ...access, permissions: [...access.permissions, { module_id: 'ordenes_compra', scope_key: 'documentos', capability: 'editar' }] };
const input = { ...newOrder(new Date('2026-10-23T02:00:00Z')), n_orders: 'OC-1', client_name: 'Cliente sintético', id_destination: 1 };
const calculation: OrderCalculation = { data: input, status: 'Pendiente', totals: { subtotal: '100.00', subtotal_exonerado: '0.00', tax: '15.00', total: '115.00', products_investment: '120.00', other_expenses: '5.00', investment_total: '125.00', profit: '-10.00' } };
const order: PurchaseOrder = { id: 7, version: 3, status: 'Pendiente', currency: 'HNL', data: input, calculation };
const destinations: Destination[] = [{ id: 1, version: 2, destination: 'Oficina' }];

test('orders use Honduras date and activation and separate destination permissions', () => {
  assert.equal(input.order_date, '2026-10-22');
  for (const key of ['total', 'status', 'id_client', 'id_supplier']) assert.ok(!(key in input));
  for (const active of [true, false]) {
    const context = { ...editor, modules: active ? access.modules : [] };
    const html = renderToStaticMarkup(<NavigationGuardProvider><CoreContext.Provider value={{ api: new ApiClient('https://example.test'), access: context, notify: () => {} }}><OrdenesCompra /></CoreContext.Provider></NavigationGuardProvider>);
    assert.equal(html.includes('Nueva orden'), active);
    assert.equal(mainNavigation(context).some(page => page.id === 'ordenes_compra'), active);
    assert.equal(settingsSections(context).some(section => section.id === 'ordenes_compra'), active);
  }
  const catalogOnly: Access = { ...access, permissions: [{ module_id: 'ordenes_compra', scope_key: 'destinos', capability: 'ver' }] };
  assert.ok(settingsSections(catalogOnly).some(section => section.id === 'ordenes_compra'));
  assert.ok(!mainNavigation(catalogOnly).some(page => page.id === 'ordenes_compra'));
  assert.ok(renderToStaticMarkup(<OrderSummaryTotals calculation={calculation} />).includes('L -10.00'));
});

const window = new Window();
for (const name of ['document', 'HTMLElement', 'HTMLInputElement', 'HTMLTextAreaElement', 'FormData', 'Event', 'MouseEvent', 'KeyboardEvent', 'navigator'] as const) Object.defineProperty(globalThis, name, { configurable: true, value: window[name] });
Object.assign(globalThis, { window, IS_REACT_ACT_ENVIRONMENT: true });
const { act } = await import('react');
const { createRoot } = await import('react-dom/client');
after(() => window.happyDOM.abort());
const submit = async (container: HTMLElement) => { await act(async () => container.querySelector('form')!.dispatchEvent(new Event('submit', { bubbles: true, cancelable: true }))); };
const click = async (text: string) => { const button = [...document.querySelectorAll('button')].find(b => b.textContent?.replace(/^[＋+]\s*/, '').trim() === text); assert.ok(button, text); await act(async () => button.click()); };

test('order conflicts retain changes and navigation asks before discarding', async () => {
  let writes = 0, closes = 0;
  const api = new ApiClient('https://example.test', async (url, init = {}) => {
    if (init.method === 'PUT') { writes++; assert.equal(new Headers(init.headers).get('If-Match'), '"3"'); return Response.json({ detail: 'La orden cambió.' }, { status: 412 }); }
    return Response.json(String(url).endsWith('/destinations') ? destinations : order);
  });
  const container = document.createElement('div'); document.body.append(container); const root = createRoot(container);
  try {
    await act(async () => root.render(<NavigationGuardProvider><CoreContext.Provider value={{ api, access: editor, notify: () => {} }}><OrderEditor initial={order} editable close={() => closes++} /></CoreContext.Provider></NavigationGuardProvider>));
    await click('Agregar gasto'); await submit(container); assert.equal(writes, 1);
    assert.ok(container.textContent?.includes('Tus cambios se conservan'));
    assert.ok(container.querySelector<HTMLFieldSetElement>('fieldset')?.disabled);
    assert.ok(container.textContent?.includes('Gasto 1'));
    await click('Abrir versión actual'); assert.ok(document.querySelector('[role="dialog"]'));
    await click('Seguir editando'); assert.ok(container.textContent?.includes('Gasto 1'));
    await click('Volver'); await click('Descartar y continuar'); assert.equal(closes, 1);
  } finally { await act(async () => root.unmount()); container.remove(); }
});

test('ambiguous order creation freezes input and retries the same body and key', async () => {
  const calls: { key: string | null; body: string }[] = [];
  const api = new ApiClient('https://example.test', async (url, init = {}) => {
    if (String(url).endsWith('/destinations')) return Response.json(destinations);
    if (init.method === 'POST' && new URL(String(url)).pathname === '/api/v1/ordenes-compra') {
      calls.push({ key: new Headers(init.headers).get('Idempotency-Key'), body: String(init.body) });
      if (calls.length === 1) throw new TypeError('synthetic network failure');
      return Response.json(order);
    }
    return Response.json(calculation);
  });
  const container = document.createElement('div'); document.body.append(container); const root = createRoot(container);
  try {
    await act(async () => root.render(<NavigationGuardProvider><CoreContext.Provider value={{ api, access: editor, notify: () => {} }}><OrderEditor initial={null} editable close={() => {}} /></CoreContext.Provider></NavigationGuardProvider>));
    await submit(container); assert.ok(container.textContent?.includes('No se pudo confirmar la creación'));
    assert.ok(container.querySelector<HTMLFieldSetElement>('fieldset')?.disabled);
    await submit(container); assert.equal(calls.length, 2); assert.ok(calls[0]?.key); assert.deepEqual(calls[0], calls[1]);
    assert.ok(!container.querySelector<HTMLFieldSetElement>('fieldset')?.disabled);
  } finally { await act(async () => root.unmount()); container.remove(); }
});

test('empty destination catalog blocks saving and read-only order cannot mutate', async () => {
  let writes = 0;
  const api = new ApiClient('https://example.test', async (url, init = {}) => { if (init.method === 'PUT') writes++; return Response.json(String(url).endsWith('/destinations') ? [] : calculation); });
  const container = document.createElement('div'); document.body.append(container); const root = createRoot(container);
  try {
    await act(async () => root.render(<NavigationGuardProvider><CoreContext.Provider value={{ api, access: editor, notify: () => {} }}><OrderEditor initial={order} editable close={() => {}} /></CoreContext.Provider></NavigationGuardProvider>));
    assert.ok(container.textContent?.includes('Agrega un destino en Ajustes'));
    assert.ok(container.querySelector<HTMLButtonElement>('button[type="submit"]')?.disabled); await submit(container); assert.equal(writes, 0);
    await act(async () => root.render(<NavigationGuardProvider><CoreContext.Provider value={{ api, access, notify: () => {} }}><OrderEditor key="view" initial={order} editable={false} close={() => {}} /></CoreContext.Provider></NavigationGuardProvider>));
    assert.ok(container.querySelector<HTMLFieldSetElement>('fieldset')?.disabled); assert.equal(container.querySelector('button[type="submit"]'), null);
  } finally { await act(async () => root.unmount()); container.remove(); }
});

test('destination settings require their own editing capability and send current version', async () => {
  const calls: RequestInit[] = [];
  const api = new ApiClient('https://example.test', async (_url, init = {}) => { if (init.method !== 'GET') calls.push(init); return Response.json(init.method === 'GET' ? destinations : destinations[0]); });
  const container = document.createElement('div'); document.body.append(container); const root = createRoot(container);
  try {
    await act(async () => root.render(<CoreContext.Provider value={{ api, access: editor, notify: () => {} }}><OrderSettings /></CoreContext.Provider>));
    assert.equal(container.querySelector('form'), null);
    const settingsAccess = { ...editor, permissions: [...editor.permissions, { module_id: 'ordenes_compra', scope_key: 'destinos', capability: 'editar' }] };
    await act(async () => root.render(<CoreContext.Provider value={{ api, access: settingsAccess, notify: () => {} }}><OrderSettings /></CoreContext.Provider>));
    await click('Editar destino'); await submit(container);
    assert.equal(calls.length, 1); assert.equal(calls[0]?.method, 'PUT'); assert.equal(new Headers(calls[0]?.headers).get('If-Match'), '"2"');
    assert.deepEqual(JSON.parse(String(calls[0]?.body)), { destination: 'Oficina' });
  } finally { await act(async () => root.unmount()); container.remove(); }
});

test('order menu enables documents for viewers and checks invoice destination permissions',async()=>{
  const api=new ApiClient('https://example.test',async(url)=>{
    const path=new URL(String(url)).pathname;
    if(path.endsWith('/document-readiness'))return Response.json({storage:true});
    if(path.endsWith('/documents'))return Response.json({items:[],order_version:3});
    if(path.endsWith('/7'))return Response.json(order);
    return Response.json({items:[{id:7,version:3,n_orders:'OC-1',n_bill:'',client_name:'Cliente',order_date:'2026-10-23',total:'115',status:'Pendiente'}],has_more:false});
  });
  const container=document.createElement('div');document.body.append(container);const root=createRoot(container);
  try{
    await act(async()=>root.render(<NavigationGuardProvider><CoreContext.Provider value={{api,access,notify:()=>{}}}><OrdenesCompra/></CoreContext.Provider></NavigationGuardProvider>));
    await click('Acciones ▾');
    const menu=(label:string)=>[...document.querySelectorAll<HTMLButtonElement>('[role="menuitem"]')].find(b=>b.textContent?.startsWith(label))!;
    assert.equal(menu('Documentos').disabled,false);assert.equal(menu('Clonar').disabled,true);assert.equal(menu('Convertir a factura').disabled,true);
    await click('Documentos');assert.ok(container.textContent?.includes('Documentos de la orden'));
  }finally{await act(async()=>root.unmount());container.remove();}
});
test('clone and invoice retries reuse their original version and key and open their destination',async()=>{
  const confirm=Object.getOwnPropertyDescriptor(window,'confirm');Object.defineProperty(window,'confirm',{configurable:true,value:()=>true});
  const full:Access={...editor,modules:[...editor.modules,{module_id:'facturas',display_name:'Facturas'}],permissions:[...editor.permissions,{module_id:'facturas',scope_key:'documentos',capability:'ver'},{module_id:'facturas',scope_key:'documentos',capability:'editar'}]};
  try{for(const kind of ['clone','convert/invoice']){
    const calls:{key:string|null;version:string|null}[]=[];let opened=0;
    const api=new ApiClient('https://example.test',async(url,init={})=>{
      const pathname=new URL(String(url)).pathname;
      if(pathname.endsWith('/'+kind)){
        calls.push({key:new Headers(init.headers).get('Idempotency-Key'),version:new Headers(init.headers).get('If-Match')});
        if(calls.length===1)throw new TypeError('ambiguous network failure');
        return Response.json(kind==='clone'?{...order,id:8,version:1}:{target_id:9});
      }
      if(pathname.endsWith('/destinations'))return Response.json(destinations);
      return Response.json({items:[{id:7,version:3,n_orders:'OC-1',n_bill:'',client_name:'Cliente',order_date:'2026-10-23',total:'115',status:'Pendiente'}],has_more:false});
    });
    const container=document.createElement('div');document.body.append(container);const root=createRoot(container);
    try{
      await act(async()=>root.render(<NavigationGuardProvider><CoreContext.Provider value={{api,access:full,notify:()=>{}}}><OrdenesCompra openInvoice={id=>{opened=id;}}/></CoreContext.Provider></NavigationGuardProvider>));
      await click('Acciones ▾');await click(kind==='clone'?'Clonar':'Convertir a factura');
      assert.ok(container.textContent?.includes('No se pudo confirmar la operación'));
      await click('Acciones ▾');await click(kind==='clone'?'Clonar':'Convertir a factura');
      assert.equal(calls.length,2);assert.deepEqual(calls[0],calls[1]);assert.equal(calls[0]?.version,'"3"');assert.ok(calls[0]?.key);
      if(kind==='clone')assert.ok(container.textContent?.includes('Editor de orden de compra'));else assert.equal(opened,9);
    }finally{await act(async()=>root.unmount());container.remove();}
  }}finally{if(confirm)Object.defineProperty(window,'confirm',confirm);else Reflect.deleteProperty(window,'confirm');}
});
test('order files retain a selection and key on ambiguous retry and hide destructive actions without permission',async()=>{
  const line='a'.repeat(48),attachment={id:'b'.repeat(48),line_id:line,filename:'private.pdf',mime:'application/pdf',byte_size:7};
  const source={...order,data:{...order.data,products:[{...order.data.products[0]!,line_id:line}]}};
  const calls:{key:string|null;version:string|null;body:string}[]=[];let stored=false;
  const api=new ApiClient('https://example.test',async(url,init={})=>{
    const pathname=new URL(String(url)).pathname;
    if(init.method==='POST'&&pathname.endsWith('/documents/'+line)){
      calls.push({key:new Headers(init.headers).get('Idempotency-Key'),version:new Headers(init.headers).get('If-Match'),body:String(init.body)});
      if(calls.length===1)throw new TypeError('synthetic timeout');
      stored=true;return Response.json({attachment,order_version:4});
    }
    if(pathname.endsWith('/document-readiness'))return Response.json({storage:true});
    if(pathname.endsWith('/documents'))return Response.json({items:stored?[attachment]:[],order_version:stored?4:3});
    return Response.json({...source,version:stored?4:3});
  });
  const container=document.createElement('div');document.body.append(container);const root=createRoot(container);
  try{
    await act(async()=>root.render(<NavigationGuardProvider><CoreContext.Provider value={{api,access:editor,notify:()=>{}}}><OrderDocuments id={7} close={()=>{}}/></CoreContext.Provider></NavigationGuardProvider>));
    const fileInput=container.querySelector<HTMLInputElement>('input[type="file"]')!;assert.ok(fileInput);
    Object.defineProperty(fileInput,'files',{configurable:true,value:[new window.File(['%PDF-12'],'private.pdf')]});
    await act(async()=>fileInput.dispatchEvent(new Event('change',{bubbles:true})));
    await click('Guardar archivo');assert.ok(container.textContent?.includes('Reintentar guardar el mismo archivo'));
    await click('Volver');await click('Seguir editando');assert.ok(container.textContent?.includes('private.pdf'));
    await click('Reintentar guardar el mismo archivo');assert.equal(calls.length,2);assert.deepEqual(calls[0],calls[1]);assert.equal(calls[0]?.version,'"3"');
    assert.ok(container.textContent?.includes('Necesitas permiso de eliminación'));assert.equal(container.querySelector('input[type="file"]'),null);
    assert.ok(!container.textContent?.includes('Retirar archivo'));
  }finally{await act(async()=>root.unmount());container.remove();}
});
