import { after, test } from 'node:test';
import assert from 'node:assert/strict';
import { Window } from 'happy-dom';
import { act } from 'react';
import Cotizaciones from '../src/pages/Cotizaciones.js';
import { CoreContext } from '../src/ui.js';
import { ApiClient } from '../src/api.js';
import { newQuote } from '../src/quotes.js';
import type { QuoteDraft } from '../src/quotes.js';
const window = new Window();
for (const name of ['document','HTMLElement','FormData','Event','MouseEvent','KeyboardEvent'] as const) Object.defineProperty(globalThis,name,{ configurable:true,value:window[name] });
Object.assign(globalThis,{ window,IS_REACT_ACT_ENVIRONMENT:true });
const { createRoot } = await import('react-dom/client');
after(() => window.happyDOM.abort());
const labels = ['Ver','Editar','Marcar como enviado','Convertir a orden de compra','Convertir a factura','Aprobar','Clonar','PDF','Enviar por correo','Eliminar'];

async function workspace(editable = true, clients = false, linked = false) {
  const input = newQuote(); input.client_name = 'Cliente sintético';
  if(linked){input.id_client=41;input.client_rtn='08011900000000';input.client_email='cliente@example.test';}
  const totals = { subtotal:'100.00', importe_exonerado:'0.00',importe_exento:'0.00',importe_gravado_15:'100.00',importe_gravado_18:'0.00',descuentos_rebajas:'0.00',isv_15:'15.00',isv_18:'0.00',total:'115.00' };
  let draft: QuoteDraft = { id:1,version:1,status:'Borrador',currency:'HNL',quote_number:'COT-000001',data:input,calculation:{ data:input,valid_until:'2026-11-21',items:[],totals } };
  let failure = 0;
  const calls: { path:string; query:string; method:string; match:string|null; key:string|null; body:Record<string,unknown>|null }[] = [];
  const api = new ApiClient('https://api.example.test',async (url,options) => {
    const path = new URL(String(url)).pathname, method=options?.method ?? 'GET', headers = new Headers(options?.headers);
    calls.push({path,query:new URL(String(url)).search,method,match:headers.get('If-Match'),key:headers.get('Idempotency-Key'),body:options?.body ? JSON.parse(String(options.body)) as Record<string,unknown> : null});
    if (path === '/api/v1/clientes/catalogo') {
      const page=Number(new URL(String(url)).searchParams.get('page'));
      return Response.json({items:[{id:page===2?42:41,cliente_name:page===2?'Cliente siguiente':'Cliente del catálogo',email:'cliente@example.test',rtn:'08011900000000'}],page,page_size:25,has_more:page===1});
    }
    if (failure && method !== 'GET') { const status=failure;failure=0;return Response.json({title:'Conflict',detail:'Recarga la versión actual.'},{status}); }
    if (method === 'DELETE') return new Response(null,{status:204});
    if (path.endsWith('/document-readiness')) return Response.json({pdf:true,company:true,email:true});
    if (path.endsWith('/pdf')) return Response.json({filename:'COT-000001.pdf',mime:'application/pdf',content_base64:btoa('%PDF-synthetic')});
    if (path.endsWith('/emails')) return Response.json({id:8,delivery_status:'pending',scheduled_at:'2099-10-22T19:00:00',last_error_code:null},{status:202});
    if (path.endsWith('/status')) { draft={...draft,version:draft.version+1,status:JSON.parse(String(options?.body)).status}; return Response.json(draft); }
    if (path.endsWith('/clone')) return Response.json({...draft,id:2,quote_number:'COT-000002',data:{...input,client_name:''}},{status:201});
    if (method === 'PUT' || (method === 'POST' && path.endsWith('/cotizaciones'))) {
      const data=JSON.parse(String(options?.body));draft={...draft,data,version:draft.version+1};return Response.json(draft);
    }
    if (path.endsWith('/cotizaciones')) return Response.json({items:[{...draft,client_name:input.client_name,quote_date:input.quote_date,valid_until:'2026-11-21',total:'115.00',item_count:1}],page:1,page_size:25,has_more:false});
    return Response.json(draft);
  });
  const container=document.createElement('div');container.id='app';document.body.append(container);const root=createRoot(container);
  await act(async () => root.render(<CoreContext.Provider value={{api,access:{is_system_administrator:editable,modules:[{module_id:'cotizaciones',display_name:'Cotizaciones'},...(clients?[{module_id:'clientes',display_name:'Clientes'}]:[])],permissions:[{module_id:'cotizaciones',scope_key:'documentos',capability:'ver'}]},notify:()=>{}}}><Cotizaciones/></CoreContext.Provider>));
  const click=async (element:Element|null) => {assert.ok(element);await act(async()=>{ element.dispatchEvent(new MouseEvent('click',{bubbles:true})); });};
  const action=async(label:string)=>{await click(container.querySelector('button[aria-haspopup="menu"]'));const item=Array.from(document.querySelectorAll('button[role="menuitem"]')).find(button=>button.textContent?.startsWith(label));assert.ok(item);await click(item);};
  return {container,calls,click,action,fail:(status:number)=>{failure=status;},dispose:async()=>{await act(async()=>root.unmount());container.remove();}};
}
test('each quotation offers the requested menu order and keyboard navigation; conversions stay disabled',async()=>{
  const app=await workspace();try{
    const trigger=app.container.querySelector('button[aria-haspopup="menu"]')!;await app.click(trigger);
    const menu=document.querySelector('[role="menu"]')!;const items=Array.from(menu.querySelectorAll<HTMLButtonElement>('button'));
    assert.deepEqual(items.map(item=>item.childNodes[0]?.textContent),labels);
    assert.equal(items[3]?.disabled,true);assert.equal(items[4]?.disabled,true);
    assert.equal(document.activeElement,items[0]);
    await act(async()=>{menu.dispatchEvent(new KeyboardEvent('keydown',{key:'End',bubbles:true}));});
    assert.equal(document.activeElement,items.at(-1));
    await act(async()=>{menu.dispatchEvent(new KeyboardEvent('keydown',{key:'Escape',bubbles:true}));});
    assert.equal(document.querySelector('[role="menu"]'),null);assert.equal(document.activeElement,trigger);
  }finally{await app.dispose();}
});
test('Ver opens a read-only quotation; Editar explicitly enables fields',async()=>{
  const app=await workspace();try{
    await app.action('Ver');
    assert.equal(app.container.querySelector<HTMLFieldSetElement>('.quote-form')?.disabled,true);
    const edit=Array.from(app.container.querySelectorAll('button')).find(button=>button.textContent==='Editar')!;await app.click(edit);
    assert.equal(app.container.querySelector<HTMLFieldSetElement>('.quote-form')?.disabled,false);
    assert.equal(app.calls.filter(call=>call.method!=='GET').length,0);
  }finally{await app.dispose();}
});
test('marking and approving use versioned status commands and retain the table without list reloads',async()=>{
  const app=await workspace();try{
    const table=app.container.querySelector('table');await app.action('Marcar como enviado');
    assert.equal(app.container.querySelector('table'),table);assert.ok(app.container.textContent?.includes('Enviada'));
    await app.action('Aprobar');assert.ok(app.container.textContent?.includes('Aprobada'));
    assert.deepEqual(app.calls.filter(call=>call.method==='PATCH').map(call=>call.match),['"1"','"2"']);
    assert.equal(app.calls.filter(call=>call.path==='/api/v1/cotizaciones').length,1);
  }finally{await app.dispose();}
});
test('clone retries retain an idempotency key; delete requires confirmation and preserves version checks',async()=>{
  const app=await workspace();try{
    app.fail(503);await app.action('Clonar');await app.action('Clonar');
    const clones=app.calls.filter(call=>call.path.endsWith('/clone'));assert.equal(clones.length,2);assert.equal(clones[0]?.key,clones[1]?.key);assert.ok(clones[0]?.key);assert.equal(clones[1]?.match,'"1"');
    assert.ok(app.container.textContent?.includes('COT-000002'));
    await app.click(Array.from(app.container.querySelectorAll('button')).find(button=>button.textContent==='Volver')!);
    await app.action('Eliminar');assert.equal(app.calls.filter(call=>call.method==='DELETE').length,0);
    const dialog=document.querySelector('[role="dialog"]')!;await app.click(Array.from(dialog.querySelectorAll('button')).find(button=>button.textContent==='Eliminar cotización')!);
    assert.equal(app.calls.find(call=>call.method==='DELETE')?.match,'"1"');
  }finally{await app.dispose();}
});
test('a viewer sees the menu with all mutation actions disabled',async()=>{
  const app=await workspace(false);try{
    await app.click(app.container.querySelector('button[aria-haspopup="menu"]'));
    const buttons=Array.from(document.querySelectorAll<HTMLButtonElement>('button[role="menuitem"]'));
    for(const index of [1,2,3,4,5,6,8,9])assert.equal(buttons[index]?.disabled,true,labels[index]!);
    assert.equal(buttons[0]?.disabled,false);
  }finally{await app.dispose();}
});
test('email opens with the generated PDF, shared defaults and an explicit versioned enqueue action',async()=>{
  const app=await workspace();try{
    await app.action('Enviar por correo');
    const dialog=document.querySelector('[role="dialog"]')!;
    assert.ok(dialog.textContent?.includes('COT-000001.pdf'));
    assert.ok(dialog.textContent?.includes('Las notas privadas no se incluyen'));
    const email=dialog.querySelector<HTMLInputElement>('input[type="email"]')!;
    const setter=Object.getOwnPropertyDescriptor(window.HTMLInputElement.prototype,'value')!.set!;
    await act(async()=>{setter.call(email,'destino@example.test');email.dispatchEvent(new Event('input',{bubbles:true}));});
    assert.equal(app.calls.filter(call=>call.method==='POST').length,0);
    await act(async()=>{dialog.querySelector('form')!.dispatchEvent(new Event('submit',{bubbles:true,cancelable:true}));});
    const enqueue=app.calls.find(call=>call.path.endsWith('/emails'));
    assert.equal(enqueue?.method,'POST');assert.equal(enqueue?.match,'"1"');assert.ok(enqueue?.key);
    assert.equal(enqueue?.body?.to,'destino@example.test');
    assert.ok(dialog.textContent?.includes('Envío en cola'));
  }finally{await app.dispose();}
});

test('new quote selects a paginated catalog client, retains selection across search and saves the association',async()=>{
  const app=await workspace(true,true);try{
    await app.click(Array.from(app.container.querySelectorAll('button')).find(button=>button.textContent==='Nueva cotización')!);
    const catalog=()=>Array.from(app.container.querySelectorAll('label')).find(label=>label.textContent?.startsWith('Cliente del catálogo'))!.querySelector('select')!;
    await act(async()=>{catalog().value='41';catalog().dispatchEvent(new Event('change',{bubbles:true}));});
    assert.ok(app.container.textContent?.includes('RTN: 08011900000000'));
    const selectedName=Array.from(app.container.querySelectorAll('label')).find(label=>label.textContent?.startsWith('Cliente seleccionado'))!.querySelector('input')!;
    assert.equal(selectedName.value,'Cliente del catálogo');assert.equal(selectedName.readOnly,true);
    await app.click(Array.from(app.container.querySelectorAll('button')).find(button=>button.textContent==='Más clientes')!);
    assert.equal(catalog().value,'41');assert.ok(catalog().textContent?.includes('Cliente siguiente'));
    const search=Array.from(app.container.querySelectorAll('label')).find(label=>label.textContent?.startsWith('Buscar cliente por'))!.querySelector('input')!;
    await act(async()=>{Object.getOwnPropertyDescriptor(window.HTMLInputElement.prototype,'value')!.set!.call(search,'José & Cía');search.dispatchEvent(new Event('input',{bubbles:true}));});
    const calculations=app.calls.filter(call=>call.method==='POST').length;
    await act(async()=>{search.dispatchEvent(new KeyboardEvent('keydown',{key:'Enter',bubbles:true,cancelable:true}));});
    assert.equal(app.calls.filter(call=>call.method==='POST').length,calculations,'search Enter must not submit the quotation');
    assert.ok(app.calls.some(call=>call.query.includes('search=Jos%C3%A9%20%26%20C%C3%ADa')&&call.query.includes('page=1')));
    assert.equal(catalog().value,'41');
    await act(async()=>{app.container.querySelector('form')!.dispatchEvent(new Event('submit',{bubbles:true,cancelable:true}));});
    const saved=app.calls.find(call=>call.path==='/api/v1/cotizaciones'&&call.method==='POST')!;
    assert.equal(saved.body?.id_client,41);assert.equal(saved.body?.client_name,'Cliente del catálogo');
    assert.equal(saved.body?.client_rtn,'08011900000000');assert.equal(saved.body?.client_email,'cliente@example.test');
  }finally{await app.dispose();}
});

test('manual mode clears the catalog association and its contact data',async()=>{
  const app=await workspace(true,true);try{
    await app.action('Editar');
    const select=Array.from(app.container.querySelectorAll('label')).find(label=>label.textContent?.startsWith('Cliente del catálogo'))!.querySelector('select')!;
    await act(async()=>{select.value='41';select.dispatchEvent(new Event('change',{bubbles:true}));});
    await app.click(Array.from(app.container.querySelectorAll('button')).find(button=>button.textContent==='Usar cliente manual')!);
    const manual=Array.from(app.container.querySelectorAll('label')).find(label=>label.textContent?.startsWith('Cliente (manual)'))!.querySelector('input')!;
    assert.equal(manual.readOnly,false);
    await act(async()=>{app.container.querySelector('form')!.dispatchEvent(new Event('submit',{bubbles:true,cancelable:true}));});
    const saved=app.calls.find(call=>call.method==='PUT')!;
    assert.equal(saved.body?.id_client,null);assert.equal(saved.body?.client_rtn,'');assert.equal(saved.body?.client_email,'');
  }finally{await app.dispose();}
});

test('catalog access is not requested when Clientes is unavailable or when viewing a saved quote',async()=>{
  for(const [editable,clients] of [[true,false],[false,true]] as const){
    const app=await workspace(editable,clients);try{
      await app.action(editable?'Editar':'Ver');
      assert.equal(app.calls.some(call=>call.path==='/api/v1/clientes/catalogo'),false);
      assert.equal(app.container.textContent?.includes('Cliente del catálogo'),false);
    }finally{await app.dispose();}
  }
});

test('email suggests the saved catalog client address without sending automatically',async()=>{
  const app=await workspace(true,true,true);try{
    await app.action('Enviar por correo');
    const dialog=document.querySelector('[role="dialog"]')!;
    assert.equal(dialog.querySelector<HTMLInputElement>('input[type="email"]')?.value,'cliente@example.test');
    assert.equal(app.calls.some(call=>call.method==='POST'),false);
  }finally{await app.dispose();}
});
