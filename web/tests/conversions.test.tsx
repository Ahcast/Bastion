import { test, after } from 'node:test';
import assert from 'node:assert/strict';
import { Window } from 'happy-dom';
import { act } from 'react';
import Cotizaciones from '../src/pages/Cotizaciones.js';
import { ApiClient } from '../src/api.js';
import { CoreContext } from '../src/ui.js';
import { NavigationGuardProvider } from '../src/navigationGuard.js';
const window = new Window();
for(const name of ['document','HTMLElement','Event','MouseEvent','FormData','KeyboardEvent'] as const) Object.defineProperty(globalThis,name,{configurable:true,value:window[name]});
Object.assign(globalThis,{window,IS_REACT_ACT_ENVIRONMENT:true});
const {createRoot}=await import('react-dom/client');after(()=>window.happyDOM.abort());
async function setup({reason=null,destinations=true,access=true}: {reason?:string|null;destinations?:boolean;access?:boolean}={}) {
 let failure=0,version=1;const opened:[string,number][]=[];
 const calls:{path:string;method:string;key:string|null;match:string|null;body:unknown}[]=[];
 const api=new ApiClient('https://api.example.test',async(url,options)=>{
  const path=new URL(String(url)).pathname,method=options?.method??'GET',headers=new Headers(options?.headers);
  calls.push({path,method,key:headers.get('Idempotency-Key'),match:headers.get('If-Match'),body:options?.body?JSON.parse(String(options.body)):null});
  if(path.endsWith('/document-readiness'))return Response.json({pdf:true,email:true,company:true,attachments:true});
  if(path.endsWith('/order-conversion'))return Response.json({quote_version:version,reason});
  if(path.endsWith('/destinations'))return Response.json(destinations?[{id:1,version:1,destination:'Destino sintético'}]:[]);
  if(method==='POST') {
   if(failure){const code=failure;failure=0;return Response.json({detail:'Error sintético'},{status:code});}
   version++;
   return Response.json({quote_id:1,quote_version:version,target_module:path.endsWith('/convert-invoice')?'facturas':'ordenes_compra',target_id:17,reused:false});
  }
  return Response.json({items:[{id:1,version,status:version>1?'Convertido':'Borrador',quote_number:'COT-000001',client_name:'Cliente sintético',order_number:'REF-1',quote_date:'2026-10-23',valid_until:'2026-11-23',total:'115.00',item_count:1}],page:1,page_size:25,has_more:false});
 });
 const node=document.createElement('div');document.body.append(node);const root=createRoot(node);
 const permissions=[{module_id:'cotizaciones',scope_key:'documentos',capability:'ver'},{module_id:'cotizaciones',scope_key:'documentos',capability:'editar'},...['facturas','ordenes_compra'].flatMap(module_id=>(access?['ver','editar']:['ver']).map(capability=>({module_id,scope_key:'documentos',capability})))];
 await act(async()=>root.render(<CoreContext.Provider value={{api,access:{is_system_administrator:false,modules:['cotizaciones','facturas','ordenes_compra'].map(module_id=>({module_id,display_name:module_id})),permissions},notify:()=>{}}}><NavigationGuardProvider><Cotizaciones openConverted={(module,id)=>opened.push([module,id])}/></NavigationGuardProvider></CoreContext.Provider>));
 const click=async(e:Element|undefined|null)=>{assert.ok(e);await act(async()=>{e.dispatchEvent(new MouseEvent('click',{bubbles:true}));});};
 const action=async(label:string)=>{await click(node.querySelector('button[aria-haspopup=menu]'));await click([...document.querySelectorAll('[role=menuitem]')].find(e=>e.textContent?.startsWith(label)));};
 return{node,calls,opened,click,action,fail:(code:number)=>{failure=code;},dispose:async()=>{await act(async()=>root.unmount());node.remove();}};
}
async function fillOrder(){
 const dialog=document.querySelector('[role=dialog]')!,input=dialog.querySelector('input')!,select=dialog.querySelector('select')!;
 await act(async()=>{Object.getOwnPropertyDescriptor(window.HTMLInputElement.prototype,'value')!.set!.call(input,'ORDER-25');input.dispatchEvent(new Event('input',{bubbles:true}));Object.getOwnPropertyDescriptor(window.HTMLSelectElement.prototype,'value')!.set!.call(select,'1');select.dispatchEvent(new Event('change',{bubbles:true}));});
 return dialog;
}
const submit=async(dialog:Element)=>act(async()=>{dialog.querySelector('form')!.dispatchEvent(new Event('submit',{bubbles:true,cancelable:true}));});
test('conversions require destination edit permission even when its read permission exists',async()=>{
 const app=await setup({access:false});try{
  await app.click(app.node.querySelector('button[aria-haspopup=menu]'));
  for(const label of ['Convertir a factura','Convertir a orden de compra'])assert.equal([...document.querySelectorAll<HTMLButtonElement>('[role=menuitem]')].find(e=>e.textContent?.startsWith(label))?.disabled,true);
  assert.equal(app.calls.some(c=>c.method==='POST'),false);
 }finally{await app.dispose();}
});
test('invoice conversion explains draft terms and only creates on explicit submit before opening its own module',async()=>{
 const app=await setup();try{
  await app.action('Convertir a factura');const dialog=document.querySelector('[role=dialog]')!;
  assert.ok(dialog.textContent?.includes('crédito de 30 días'));assert.equal(app.calls.some(c=>c.method==='POST'),false);
  await submit(dialog);const call=app.calls.find(c=>c.method==='POST')!;assert.ok(call.key);assert.equal(call.match,'"1"');assert.ok(call.path.endsWith('/convert-invoice'));
  assert.deepEqual(app.opened,[['facturas',17]]);assert.ok(app.node.textContent?.includes('Convertido'));
 }finally{await app.dispose();}
});
test('order header and idempotency key remain frozen across an ambiguous retry',async()=>{
 const app=await setup();try{
  await app.action('Convertir a orden de compra');const dialog=await fillOrder();app.fail(503);await submit(dialog);
  assert.equal(dialog.querySelector('fieldset')?.disabled,true);assert.ok(dialog.textContent?.includes('Reintenta los mismos datos'));
  await submit(dialog);const calls=app.calls.filter(c=>c.method==='POST');assert.equal(calls.length,2);
  assert.equal(calls[0]?.key,calls[1]?.key);assert.equal(calls[1]?.match,'"1"');assert.deepEqual(calls[0]?.body,{n_orders:'ORDER-25',id_destination:1});assert.deepEqual(calls[1]?.body,calls[0]?.body);
  assert.deepEqual(app.opened,[['ordenes_compra',17]]);
 }finally{await app.dispose();}
});
test('unsupported quote data or an empty destination catalog blocks conversion without a mutation',async()=>{
 for(const options of [{reason:'Órdenes no admite ISV del 18%'},{destinations:false}]){
  const app=await setup(options);try{
   await app.action('Convertir a orden de compra');const dialog=document.querySelector('[role=dialog]')!;
   assert.equal(dialog.querySelector<HTMLButtonElement>('button[type=submit]')?.disabled,true);await submit(dialog);
   assert.equal(app.calls.some(c=>c.method==='POST'),false);assert.ok(dialog.textContent?.includes('ISV del 18%')||dialog.textContent?.includes('Agrega un destino'));
  }finally{await app.dispose();}
 }
});
test('a stale quote keeps the order header visible and never opens a target',async()=>{
 const app=await setup();try{
  await app.action('Convertir a orden de compra');const dialog=await fillOrder();app.fail(412);await submit(dialog);
  assert.equal(dialog.querySelector('input')?.value,'ORDER-25');assert.equal(app.opened.length,0);assert.ok(dialog.textContent?.includes('Tus datos se conservan'));
 }finally{await app.dispose();}
});

