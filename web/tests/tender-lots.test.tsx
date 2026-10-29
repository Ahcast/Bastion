import { after, test } from 'node:test';
import assert from 'node:assert/strict';
import { Window } from 'happy-dom';
import { ApiClient } from '../src/api.js';
import { CoreContext } from '../src/ui.js';
import { NavigationGuardProvider } from '../src/navigationGuard.js';
import TenderLots from '../src/components/TenderLots.js';
import TenderPdfOrder from '../src/components/TenderPdfOrder.js';
import Licitaciones from '../src/pages/Licitaciones.js';
import type { Access } from '../src/types.js';
import type { Expedient } from '../src/tenders.js';

const window = new Window();
for (const name of ['document', 'HTMLElement', 'HTMLInputElement', 'HTMLTextAreaElement', 'Event', 'MouseEvent', 'navigator'] as const) Object.defineProperty(globalThis, name, { configurable:true, value:window[name] });
Object.assign(globalThis, { window, IS_REACT_ACT_ENVIRONMENT:true });
const { act } = await import('react'); const { createRoot } = await import('react-dom/client');
after(() => window.happyDOM.abort());
const access: Access = { is_system_administrator:true, modules:[], permissions:[] };
const record: Expedient = { id: 7, name:'', version:1, state:'borrador', forms:[], variables:[{key:'lotes', label:'Lotes', kind:'integer', value:'2'}, {key:'stylo', label:'Estilo', kind:'text', value:'bid'}] };
const item = {descripcion:'Equipo', pais_origen:'China', fecha_entrega:'30 días', cantidad:'2.5', unidad:'und', transporte:'10.00', precio_unitario:'100.00'};
test('lot editor shows BID fields, lists all lots and sends one versioned lot', async () => {
  const writes: RequestInit[] = [];
  const api = new ApiClient('https://example.test', async (_url, init={}) => { if (init.method === 'PUT') { writes.push(init); return Response.json({...record,version:2}); } return Response.json({numero:1,items:[item,item],total:'595.00',version:1}); });
  const container=document.createElement('div');document.body.append(container);const root=createRoot(container);
  try {
    await act(async()=>root.render(<NavigationGuardProvider><CoreContext.Provider value={{api,access,notify:()=>{}}}><TenderLots record={record} editable saved={()=>{}} /></CoreContext.Provider></NavigationGuardProvider>));
    assert.equal(container.querySelectorAll('select option').length,2);
    assert.ok(container.textContent?.includes('País de origen')); assert.ok(container.textContent?.includes('Transporte'));
    await act(async()=>[...container.querySelectorAll('button')].find(b=>b.textContent==='Quitar')!.click());
    assert.equal(container.querySelectorAll('tbody tr').length,1); assert.equal(container.querySelector('tbody tr td')?.textContent,'1');
    await act(async()=>container.querySelector('form')!.dispatchEvent(new Event('submit',{bubbles:true,cancelable:true})));
    assert.deepEqual(JSON.parse(String(writes[0]!.body)), {items:[item]}); assert.equal(new Headers(writes[0]!.headers).get('if-match'),'"1"');
  } finally { await act(async()=>root.unmount());container.remove(); }
});
test('PDF order can be changed with accessible controls and saved', async () => {
  const writes: RequestInit[]=[];
  const api=new ApiClient('https://example.test',async (_url,init={})=>{if(init.method==='PUT'){writes.push(init);return Response.json({...record,version:2});}return Response.json({version:1,items:[{key:'oferta',label:'Carta'}, {key:'lote:1',label:'Lote 1'}]});});
  const container=document.createElement('div');document.body.append(container);const root=createRoot(container);
  try {
    await act(async()=>root.render(<NavigationGuardProvider><CoreContext.Provider value={{api,access,notify:()=>{}}}><TenderPdfOrder record={record} editable saved={()=>{}} /></CoreContext.Provider></NavigationGuardProvider>));
    await act(async()=>container.querySelector<HTMLButtonElement>('button[aria-label="Subir Lote 1"]')!.click());
    await act(async()=>[...container.querySelectorAll('button')].find(b=>b.textContent==='Guardar orden')!.click());
    assert.deepEqual(JSON.parse(String(writes[0]!.body)),{items:['lote:1','oferta']});
  } finally { await act(async()=>root.unmount());container.remove(); }
});
test('reducing lots asks for confirmation and identifies the data to discard', async () => {
  const writes: RequestInit[] = [];
  const api = new ApiClient('https://example.test', async (url,init={}) => {
    const path = new URL(String(url)).pathname;
    if (init.method === 'PUT') { writes.push(init); return Response.json({...record,version:2}); }
    if (path.endsWith('/lotes')) return Response.json([{numero:2,items:3}]);
    if (path === '/api/v1/licitaciones') return Response.json({items:[record],page:1,page_size:25,has_more:false});
    return Response.json(record);
  });
  const granted:Access = {...access,modules:[{module_id:'licitaciones',display_name:'Licitaciones'}],permissions:[{module_id:'licitaciones',scope_key:'expedientes',capability:'ver'},{module_id:'licitaciones',scope_key:'expedientes',capability:'editar'}]};
  const container=document.createElement('div');document.body.append(container);const root=createRoot(container);
  const click=async(text:string,scope:ParentNode=container)=>{const button=[...scope.querySelectorAll('button')].find(b=>b.textContent===text);assert.ok(button,text);await act(async()=>button.click());};
  try {
    await act(async()=>root.render(<NavigationGuardProvider><CoreContext.Provider value={{api,access:granted,notify:()=>{}}}><Licitaciones /></CoreContext.Provider></NavigationGuardProvider>));
    await click('Abrir expediente'); await click('Variables');
    const row=[...container.querySelectorAll('fieldset.document-variable')].find(x=>x.querySelector<HTMLInputElement>('input')?.value==='lotes')!;
    const input=[...row.querySelectorAll<HTMLInputElement>('input')].find(x=>x.parentElement?.textContent?.startsWith('Valor'))!;
    const setter=Object.getOwnPropertyDescriptor(window.HTMLInputElement.prototype,'value')!.set!;
    await act(async()=>{setter.call(input,'1');input.dispatchEvent(new Event('input',{bubbles:true}));});
    await act(async()=>container.querySelector('form')!.dispatchEvent(new Event('submit',{bubbles:true,cancelable:true})));
    const dialog=document.body.querySelector('[role="dialog"]')!;assert.ok(dialog.textContent?.includes('Lote 2: 3 ítem'));assert.equal(writes.length,0);
    await click('Confirmar descarte y guardar',dialog);
    assert.equal(JSON.parse(String(writes[0]!.body)).confirm_discard,true);
  } finally { await act(async()=>root.unmount());container.remove(); }
});
