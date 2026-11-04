import { after, test } from 'node:test';
import assert from 'node:assert/strict';
import { Window } from 'happy-dom';
import { ApiClient } from '../src/api.js';
import { CoreContext } from '../src/ui.js';
import { NavigationGuardProvider, useUnsavedChanges } from '../src/navigationGuard.js';
import TenderEconomicPreview from '../src/components/TenderEconomicPreview.js';
import TenderLots from '../src/components/TenderLots.js';
import TenderPdfOrder from '../src/components/TenderPdfOrder.js';
import Licitaciones from '../src/pages/Licitaciones.js';
import type { Access } from '../src/types.js';
import type { Expedient } from '../src/tenders.js';

const window = new Window();
for (const name of ['document', 'HTMLElement', 'HTMLInputElement', 'HTMLTextAreaElement', 'Event', 'MouseEvent', 'navigator'] as const) Object.defineProperty(globalThis, name, { configurable:true, value:window[name] });
Object.assign(globalThis, { window, IS_REACT_ACT_ENVIRONMENT:true });
const { act } = await import('react'); const { createRoot } = await import('react-dom/client');

test('economic preview saves drafts first and requests only economic PDF with the new version', async () => {
  let version = 1;
  const writes: RequestInit[] = [];
  const api = new ApiClient('https://example.test', async (url, init = {}) => {
    assert.ok(String(url).endsWith('/oferta-economica/preview'));
    writes.push(init);
    return Response.json({ filename:'oferta.pdf', mime:'application/pdf', content_base64:btoa('%PDF-1.4') });
  });
  function Draft() { useUnsavedChanges(true, false, async () => { version = 2; return true; }); return null; }
  const open = window.open;
  window.open = (() => ({ closed:true, opener:null, document:{title:'',body:{textContent:''}} })) as unknown as typeof window.open;
  const container = document.createElement('div'); document.body.append(container); const root = createRoot(container);
  try {
    await act(async () => root.render(<NavigationGuardProvider><CoreContext.Provider value={{api, access, notify:()=>{}}}><Draft /><TenderEconomicPreview latest={() => ({...record,version})} /></CoreContext.Provider></NavigationGuardProvider>));
    await act(async () => container.querySelector('button')!.click());
    assert.equal(writes.length, 1);
    assert.equal(new Headers(writes[0]!.headers).get('if-match'), '"2"');
    assert.equal(writes[0]!.method, 'POST');
  } finally { window.open = open; await act(async () => root.unmount()); container.remove(); }
});
after(() => window.happyDOM.abort());
const access: Access = { is_system_administrator:true, modules:[], permissions:[] };
const record: Expedient = { id: 7, name:'', version:1, state:'borrador', forms:[], variables:[{key:'lotes', label:'Lotes', kind:'integer', value:'2'}, {key:'stylo', label:'Estilo', kind:'text', value:'bid'}] };
const item = {descripcion:'Equipo', pais_origen:'China', fecha_entrega:'30 días', cantidad:'2.5', unidad:'und', transporte:'10.00', precio_unitario:'100.00'};

test('JICA edits DDP prices and origin without a second transport or tax charge', async () => {
  const jica = {...record, variables:record.variables!.map(v=>v.key==='stylo'?{...v,value:'jica'}:v)};
  const row={...item,transporte:'0.00'}; const writes:RequestInit[]=[];
  const api=new ApiClient('https://example.test',async (_url,init={})=>{if(init.method==='PUT'){writes.push(init);return Response.json({...jica,version:2});}return Response.json({numero:1,items:[row,row],total:'500.00',version:1});});
  const container=document.createElement('div');document.body.append(container);const root=createRoot(container);
  try {
    await act(async()=>root.render(<NavigationGuardProvider><CoreContext.Provider value={{api,access,notify:()=>{}}}><TenderLots record={jica} editable saved={()=>{}} /></CoreContext.Provider></NavigationGuardProvider>));
    const headings=[...container.querySelectorAll('th')].map(h=>h.textContent);
    assert.ok(headings.includes('País de origen'));assert.ok(headings.includes('Precio unitario DDP'));assert.ok(!headings.includes('Transporte'));
    assert.ok(container.textContent?.includes('DDP: incluye ISV y transporte'));
    await act(async()=>[...container.querySelectorAll('button')].find(b=>b.textContent==='Quitar')!.click());
    await act(async()=>container.querySelector('form')!.dispatchEvent(new Event('submit',{bubbles:true,cancelable:true})));
    assert.deepEqual(JSON.parse(String(writes[0]!.body)),{items:[row],tamano_tabla:'7'});
  } finally {await act(async()=>root.unmount());container.remove();}
});
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
    assert.deepEqual(JSON.parse(String(writes[0]!.body)), {items:[item],tamano_tabla:'7'}); assert.equal(new Headers(writes[0]!.headers).get('if-match'),'"1"');
  } finally { await act(async()=>root.unmount());container.remove(); }
});
test('PDF order can be changed with accessible controls and saved', async () => {
  const writes: RequestInit[]=[];
  const api=new ApiClient('https://example.test',async (_url,init={})=>{if(init.method==='PUT'){writes.push(init);return Response.json({...record,version:2});}return Response.json({version:1,items:[{key:'oferta',label:'Carta'}, {key:'lote:1',label:'Lote 1'}]});});
  const container=document.createElement('div');document.body.append(container);const root=createRoot(container);
  try {
    await act(async()=>root.render(<NavigationGuardProvider><CoreContext.Provider value={{api,access,notify:()=>{}}}><TenderPdfOrder record={record} editable saved={()=>{}} /></CoreContext.Provider></NavigationGuardProvider>));
    await act(async()=>container.querySelector<HTMLButtonElement>('button[aria-label="Subir Lote 1"]')!.click());
    const title=container.querySelector<HTMLInputElement>('input[aria-label="Título del documento 1"]')!;
    const setter=Object.getOwnPropertyDescriptor(window.HTMLInputElement.prototype,'value')!.set!;
    await act(async()=>{setter.call(title,'Lote de equipos');title.dispatchEvent(new Event('input',{bubbles:true}));});
    await act(async()=>[...container.querySelectorAll('button')].find(b=>b.textContent==='Guardar orden y títulos')!.click());
    assert.deepEqual(JSON.parse(String(writes[0]!.body)),{items:['lote:1','oferta'],titles:{'lote:1':'Lote de equipos',oferta:'Carta'}});
  } finally { await act(async()=>root.unmount());container.remove(); }
});
test('lot drafts survive changing the selected lot', async () => {
  const api = new ApiClient('https://example.test', async url => { const number = Number(new URL(String(url)).pathname.split('/').at(-1)); return Response.json({numero:number, items:[], total:'0.00', version:1}); });
  const container=document.createElement('div'); document.body.append(container); const root=createRoot(container);
  try {
    await act(async()=>root.render(<NavigationGuardProvider><CoreContext.Provider value={{api,access,notify:()=>{}}}><TenderLots record={record} editable saved={()=>{}} /></CoreContext.Provider></NavigationGuardProvider>));
    await act(async()=>[...container.querySelectorAll('button')].find(b=>b.textContent==='Añadir ítem')!.click());
    const description=container.querySelector('textarea')!; const setter=Object.getOwnPropertyDescriptor(window.HTMLTextAreaElement.prototype,'value')!.set!;
    await act(async()=>{setter.call(description,'Equipo');description.dispatchEvent(new Event('input',{bubbles:true}));});
    const select=container.querySelector('select')!;
    await act(async()=>{select.value='2';select.dispatchEvent(new Event('change',{bubbles:true}));});
    await act(async()=>{select.value='1';select.dispatchEvent(new Event('change',{bubbles:true}));});
    assert.equal(container.querySelector('textarea')?.value,'Equipo');
  } finally { await act(async()=>root.unmount()); container.remove(); }
});
test('table font size is kept separately for each lot and saved in points', async () => {
  const writes: {lot:number; body:{tamano_tabla:string}}[] = [];
  const api = new ApiClient('https://example.test', async (url,init={}) => {
    const lot=Number(new URL(String(url)).pathname.split('/').at(-1));
    if(init.method==='PUT'){writes.push({lot,body:JSON.parse(String(init.body))});return Response.json({...record,version:2});}
    return Response.json({numero:lot,items:[],tamano_tabla:'7',total:'0.00',version:1});
  });
  const container=document.createElement('div'); document.body.append(container); const root=createRoot(container);
  try {
    await act(async()=>root.render(<NavigationGuardProvider><CoreContext.Provider value={{api,access,notify:()=>{}}}><TenderLots record={record} editable saved={()=>{}} /></CoreContext.Provider></NavigationGuardProvider>));
    const setter=Object.getOwnPropertyDescriptor(window.HTMLInputElement.prototype,'value')!.set!;
    const size=()=>container.querySelector<HTMLInputElement>('input[type="number"]')!;
    await act(async()=>{setter.call(size(),'8.5');size().dispatchEvent(new Event('input',{bubbles:true}));});
    const select=container.querySelector('select')!; await act(async()=>{select.value='2';select.dispatchEvent(new Event('change',{bubbles:true}));});
    assert.equal(size().value,'7');
    await act(async()=>{setter.call(size(),'9');size().dispatchEvent(new Event('input',{bubbles:true}));});
    await act(async()=>{select.value='1';select.dispatchEvent(new Event('change',{bubbles:true}));});
    assert.equal(size().value,'8.5');
    await act(async()=>container.querySelector('form')!.dispatchEvent(new Event('submit',{bubbles:true,cancelable:true})));
    assert.deepEqual(writes.map(x=>[x.lot,x.body.tamano_tabla]),[[1,'8.5'],[2,'9']]);
  } finally { await act(async()=>root.unmount()); container.remove(); }
});
test('a newly selected lot cannot be edited with the previous lot response', async () => {
  let finish: ((value:Response)=>void) | undefined;
  const api = new ApiClient('https://example.test', async url => { const number=Number(new URL(String(url)).pathname.split('/').at(-1)); if(number===2) return new Promise<Response>(resolve=>{finish=resolve;}); return Response.json({numero:1,items:[],total:'0.00',version:1}); });
  const container=document.createElement('div'); document.body.append(container); const root=createRoot(container);
  try {
    await act(async()=>root.render(<NavigationGuardProvider><CoreContext.Provider value={{api,access,notify:()=>{}}}><TenderLots record={record} editable saved={()=>{}} /></CoreContext.Provider></NavigationGuardProvider>));
    const select=container.querySelector('select')!;
    await act(async()=>{select.value='2';select.dispatchEvent(new Event('change',{bubbles:true}));});
    assert.ok(![...container.querySelectorAll('button')].some(button=>button.textContent==='Añadir ítem'));
    await act(async()=>{finish!(Response.json({numero:2,items:[],total:'0.00',version:1}));});
    assert.ok([...container.querySelectorAll('button')].some(button=>button.textContent==='Añadir ítem'));
  } finally { await act(async()=>root.unmount());container.remove(); }
});
test('lot-only save is blocked while style or lot count is a draft', async () => {
  const api = new ApiClient('https://example.test', async () => Response.json({numero:1,items:[],total:'0.00',version:1}));
  const draft = { ...record, variables: record.variables!.map(v => v.key === 'stylo' ? {...v,value:'std'} : v) };
  const container=document.createElement('div'); document.body.append(container); const root=createRoot(container);
  try {
    await act(async()=>root.render(<NavigationGuardProvider><CoreContext.Provider value={{api,access,notify:()=>{}}}><TenderLots record={draft} latest={()=>record} editable saved={()=>{}} /></CoreContext.Provider></NavigationGuardProvider>));
    await act(async()=>[...container.querySelectorAll('button')].find(b=>b.textContent==='Añadir ítem')!.click());
    assert.equal([...container.querySelectorAll('button')].find(b=>b.textContent==='Guardar lote')?.disabled,true);
    assert.ok(container.textContent?.includes('Guardar expediente'));
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
    await act(async()=>container.querySelector('form.document-values')!.dispatchEvent(new Event('submit',{bubbles:true,cancelable:true})));
    const dialog=document.body.querySelector('[role="dialog"]')!;assert.ok(dialog.textContent?.includes('Lote 2: 3 ítem'));assert.equal(writes.length,0);
    await click('Confirmar descarte y guardar',dialog);
    assert.equal(JSON.parse(String(writes[0]!.body)).confirm_discard,true);
  } finally { await act(async()=>root.unmount());container.remove(); }
});
