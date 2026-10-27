import { after, test } from 'node:test';
import assert from 'node:assert/strict';
import { Window } from 'happy-dom';
import { ApiClient } from '../src/api.js';
import { CoreContext } from '../src/ui.js';
import { NavigationGuardProvider } from '../src/navigationGuard.js';
import { InvoiceCommandDialog } from '../src/components/InvoiceCommands.js';
import Facturas from '../src/pages/Facturas.js';
import type { Access } from '../src/types.js';
import type { InvoiceDraft, InvoiceSummary } from '../src/invoices.js';

const window=new Window();
for(const name of ['document','HTMLElement','HTMLInputElement','HTMLTextAreaElement','FormData','Event','MouseEvent','KeyboardEvent','navigator'] as const) Object.defineProperty(globalThis,name,{configurable:true,value:window[name]});
Object.assign(globalThis,{window,IS_REACT_ACT_ENVIRONMENT:true});
const {act}=await import('react');const {createRoot}=await import('react-dom/client');after(()=>window.happyDOM.abort());
const access:Access={is_system_administrator:false,modules:[{module_id:'facturas',display_name:'Facturas'}],permissions:['ver','editar','eliminar'].map(capability=>({module_id:'facturas',scope_key:'documentos',capability}))};
const row:InvoiceSummary={id:7,version:3,invoice_number:'000-001-01-00000001',client_name:'Cliente',order_number:'ORD-1',invoice_date:'2026-10-27',status:'Enviado',paid_total:'20.00',total:'115.00',balance:'95.00',item_count:1};

test('issued invoice actions respect immutable state, payments, document readiness and permissions',async()=>{
 for(const editor of [true,false]){
  const calls:string[]=[];const api=new ApiClient('https://example.test',async(url,init={})=>{
   calls.push(init.method??'GET');return Response.json(new URL(String(url)).pathname.endsWith('/document-readiness')?{pdf:true}:{items:[row],page:1,page_size:25,has_more:false});
  });
  const container=document.createElement('div');document.body.append(container);const root=createRoot(container);
  try{
   await act(async()=>root.render(<NavigationGuardProvider><CoreContext.Provider value={{api,access:editor?access:{...access,permissions:access.permissions.filter(p=>p.capability==='ver')},notify:()=>{}}}><Facturas/></CoreContext.Provider></NavigationGuardProvider>));
   await act(async()=>container.querySelector<HTMLButtonElement>('[aria-haspopup="menu"]')!.click());
   const items=[...document.querySelectorAll<HTMLButtonElement>('[role="menuitem"]')];
   assert.equal(items[1]?.disabled,true);assert.equal(items[2]?.disabled,true);assert.equal(items[8]?.disabled,true);
   assert.equal(items[3]?.disabled,!editor);assert.equal(items[4]?.disabled,!editor);
   assert.ok([5,6,7].every(index=>items[index]?.disabled===false),'document actions use read permission');
   assert.ok(calls.every(method=>method==='GET'));
  }finally{await act(async()=>root.unmount());container.remove();}
 }
});

test('ambiguous payment retries freeze the payload and reuse the same idempotency key and invoice version',async()=>{
 const bodies:string[]=[],keys:(string|null)[]=[];let completed=0;
 const api=new ApiClient('https://example.test',async(url,init={})=>{
  assert.equal(new URL(String(url)).pathname,'/api/v1/facturas/7/payments');assert.equal(new Headers(init.headers).get('If-Match'),'"3"');
  bodies.push(String(init.body));keys.push(new Headers(init.headers).get('Idempotency-Key'));
  if(bodies.length===1)throw new TypeError('synthetic network ambiguity');
  return Response.json({id:7,status:'Enviado',version:4} as InvoiceDraft);
 });
 const container=document.createElement('div');document.body.append(container);const root=createRoot(container);
 try{
  await act(async()=>root.render(<CoreContext.Provider value={{api,access,notify:()=>{}}}><InvoiceCommandDialog invoice={row} action="payment" close={()=>{}} changed={()=>{}} completed={()=>completed++}/></CoreContext.Provider>));
  const input=document.querySelector<HTMLInputElement>('[aria-label="Monto del pago (HNL)"]')!;
  await act(async()=>{Object.getOwnPropertyDescriptor(window.HTMLInputElement.prototype,'value')!.set!.call(input,'40');input.dispatchEvent(new Event('input',{bubbles:true}));});
  const submit=async()=>{await act(async()=>document.querySelector('form')!.dispatchEvent(new Event('submit',{bubbles:true,cancelable:true})));};
  await submit();assert.equal(completed,0);assert.ok(document.querySelector<HTMLFieldSetElement>('fieldset')?.disabled);assert.ok(document.body.textContent?.includes('No se confirmó la respuesta'));
  await submit();assert.equal(completed,1);assert.equal(bodies.length,2);assert.equal(bodies[0],bodies[1]);assert.ok(keys[0]);assert.equal(keys[0],keys[1]);
  assert.equal(JSON.parse(bodies[0]!).amount,'40');
 }finally{await act(async()=>root.unmount());container.remove();}
});

test('marking sent requires an explicit versioned confirmation and explains numbering and immutability',async()=>{
 let calls=0;
 const api=new ApiClient('https://example.test',async(url,init={})=>{
  calls++;assert.equal(new URL(String(url)).pathname,'/api/v1/facturas/7/mark-sent');assert.equal(new Headers(init.headers).get('If-Match'),'"3"');assert.ok(new Headers(init.headers).get('Idempotency-Key'));
  return Response.json({detail:'stale invoice'}, {status:412});
 });
 const container=document.createElement('div');document.body.append(container);const root=createRoot(container);
 try{
  await act(async()=>root.render(<CoreContext.Provider value={{api,access,notify:()=>{}}}><InvoiceCommandDialog invoice={{...row,status:'Borrador',invoice_number:null}} action="sent" close={()=>{}} changed={()=>{}} completed={()=>{}}/></CoreContext.Provider>));
  assert.equal(calls,0);assert.ok(document.body.textContent?.includes('Se asignará el siguiente número SAR'));assert.ok(document.body.textContent?.includes('Esta acción no envía un correo'));
  await act(async()=>document.querySelector('form')!.dispatchEvent(new Event('submit',{bubbles:true,cancelable:true})));
  assert.equal(calls,1);assert.ok(document.body.textContent?.includes('La factura cambió'));
  await act(async()=>document.querySelector('form')!.dispatchEvent(new Event('submit',{bubbles:true,cancelable:true})));
  assert.equal(calls,1);
 }finally{await act(async()=>root.unmount());container.remove();}
});
