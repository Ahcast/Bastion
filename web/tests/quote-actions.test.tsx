import { after, test } from 'node:test';
import assert from 'node:assert/strict';
import { Window } from 'happy-dom';
import { act } from 'react';
import Cotizaciones from '../src/pages/Cotizaciones.js';
import { CoreContext } from '../src/ui.js';
import { ApiClient } from '../src/api.js';
import { NavigationGuardProvider } from '../src/navigationGuard.js';
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
  let attachments: {id:string;filename:string;title:string;mime:string;byte_size:number;created_at:string}[] = [];
  const calls: { path:string; query:string; method:string; match:string|null; key:string|null; body:Record<string,unknown>|null }[] = [];
  const api = new ApiClient('https://api.example.test',async (url,options) => {
    const path = new URL(String(url)).pathname, method=options?.method ?? 'GET', headers = new Headers(options?.headers);
    calls.push({path,query:new URL(String(url)).search,method,match:headers.get('If-Match'),key:headers.get('Idempotency-Key'),body:options?.body ? JSON.parse(String(options.body)) as Record<string,unknown> : null});
    if (path === '/api/v1/clientes/catalogo') {
      const page=Number(new URL(String(url)).searchParams.get('page'));
      return Response.json({items:[{id:page===2?42:41,cliente_name:page===2?'Cliente siguiente':'Cliente del catálogo',email:'cliente@example.test',rtn:'08011900000000'}],page,page_size:25,has_more:page===1});
    }
    if (failure && method !== 'GET') { const status=failure;failure=0;return Response.json({title:'Conflict',detail:'Recarga la versión actual.'},{status}); }
    if(path.endsWith('/attachments')) {
      if(method==='POST') {
        const input=JSON.parse(String(options?.body)); const file={id:(attachments.length+1).toString(16).padStart(48,'0'),filename:input.filename,title:input.title,mime:input.filename.endsWith('.pdf')?'application/pdf':'application/octet-stream',byte_size:atob(input.content_base64).length,created_at:'2026-10-23T00:00:00'};
        attachments=[file,...attachments]; draft={...draft,version:draft.version+1};
        return Response.json({attachment:file,quote_version:draft.version},{status:201});
      }
      return Response.json({items:attachments,page:1,has_more:false,quote_version:draft.version});
    }
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
  await act(async () => root.render(<CoreContext.Provider value={{api,access:{is_system_administrator:editable,modules:[{module_id:'cotizaciones',display_name:'Cotizaciones'},...(clients?[{module_id:'clientes',display_name:'Clientes'}]:[])],permissions:[{module_id:'cotizaciones',scope_key:'documentos',capability:'ver'}]},notify:()=>{}}}><NavigationGuardProvider><Cotizaciones/></NavigationGuardProvider></CoreContext.Provider>));
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

test('one client combobox searches, paginates, preserves selection and saves the catalog association',async()=>{
  const app=await workspace(true,true);try{
    await app.click(Array.from(app.container.querySelectorAll('button')).find(button=>button.textContent==='Nueva cotización')!);
    const input=app.container.querySelector<HTMLInputElement>('input[role="combobox"]')!;
    assert.equal(app.container.querySelectorAll('input[role="combobox"]').length,1);
    assert.equal(app.calls.some(call=>call.path==='/api/v1/clientes/catalogo'),false,'catalog waits until the field opens');
    await app.click(app.container.querySelector('button[aria-label="Mostrar clientes"]'));
    await app.click(Array.from(app.container.querySelectorAll('[role="option"]')).find(option=>option.textContent?.includes('Cliente del catálogo'))!);
    assert.equal(input.value,'Cliente del catálogo');assert.equal(input.readOnly,false);
    assert.ok(app.container.textContent?.includes('RTN: 08011900000000'));
    assert.equal(input.getAttribute('aria-expanded'),'false');
    await app.click(app.container.querySelector('button[aria-label="Mostrar clientes"]'));
    await app.click(Array.from(app.container.querySelectorAll('button')).find(button=>button.textContent==='Más clientes')!);
    assert.equal(input.value,'Cliente del catálogo');
    assert.ok(app.container.textContent?.includes('Cliente siguiente'));
    await act(async()=>{input.dispatchEvent(new KeyboardEvent('keydown',{key:'Escape',bubbles:true,cancelable:true}));});
    assert.equal(input.getAttribute('aria-expanded'),'false');assert.equal(input.value,'Cliente del catálogo');
    await act(async()=>{app.container.querySelector('form')!.dispatchEvent(new Event('submit',{bubbles:true,cancelable:true}));});
    const saved=app.calls.find(call=>call.path==='/api/v1/cotizaciones'&&call.method==='POST')!;
    assert.equal(saved.body?.id_client,41);assert.equal(saved.body?.client_name,'Cliente del catálogo');
    assert.equal(saved.body?.client_rtn,'08011900000000');assert.equal(saved.body?.client_email,'cliente@example.test');
  }finally{await app.dispose();}
});

test('typing in the same client field searches and becomes manual without submitting or retaining catalog contact data',async()=>{
  const app=await workspace(true,true,true);try{
    await app.action('Editar');
    const input=app.container.querySelector<HTMLInputElement>('input[role="combobox"]')!;
    await act(async()=>{Object.getOwnPropertyDescriptor(window.HTMLInputElement.prototype,'value')!.set!.call(input,'José & Cía');input.dispatchEvent(new Event('input',{bubbles:true}));});
    await act(async()=>{await new Promise(resolve=>setTimeout(resolve,240));});
    assert.ok(app.calls.some(call=>call.query.includes('search=Jos%C3%A9%20%26%20C%C3%ADa')&&call.query.includes('page=1')));
    await act(async()=>{input.dispatchEvent(new KeyboardEvent('keydown',{key:'ArrowDown',bubbles:true,cancelable:true}));});
    assert.ok(input.getAttribute('aria-activedescendant'));
    await act(async()=>{input.dispatchEvent(new KeyboardEvent('keydown',{key:'Enter',bubbles:true,cancelable:true}));});
    assert.equal(input.value,'José & Cía');assert.equal(input.getAttribute('aria-expanded'),'false');
    assert.equal(app.calls.some(call=>call.method==='PUT'),false);
    await act(async()=>{app.container.querySelector('form')!.dispatchEvent(new Event('submit',{bubbles:true,cancelable:true}));});
    const saved=app.calls.find(call=>call.method==='PUT')!;
    assert.equal(saved.body?.id_client,null);assert.equal(saved.body?.client_name,'José & Cía');
    assert.equal(saved.body?.client_rtn,'');assert.equal(saved.body?.client_email,'');
  }finally{await app.dispose();}
});

test('the client field supports keyboard selection without duplicate controls or an accidental form submit',async()=>{
  const app=await workspace(true,true);try{
    await app.action('Editar');
    const input=app.container.querySelector<HTMLInputElement>('input[role="combobox"]')!;
    await app.click(app.container.querySelector('button[aria-label="Mostrar clientes"]'));
    await act(async()=>{await new Promise(resolve=>setTimeout(resolve,240));});
    await act(async()=>{input.dispatchEvent(new KeyboardEvent('keydown',{key:'ArrowDown',bubbles:true,cancelable:true}));});
    await act(async()=>{input.dispatchEvent(new KeyboardEvent('keydown',{key:'ArrowDown',bubbles:true,cancelable:true}));});
    await act(async()=>{input.dispatchEvent(new KeyboardEvent('keydown',{key:'Enter',bubbles:true,cancelable:true}));});
    assert.equal(input.value,'Cliente del catálogo');assert.equal(input.getAttribute('aria-expanded'),'false');
    assert.equal(app.calls.some(call=>call.method==='PUT'),false);
    await act(async()=>{input.dispatchEvent(new KeyboardEvent('keydown',{key:'Enter',bubbles:true,cancelable:true}));});
    await act(async()=>{app.container.querySelector('form')!.dispatchEvent(new Event('submit',{bubbles:true,cancelable:true}));});
    assert.equal(app.calls.find(call=>call.method==='PUT')?.body?.id_client,41);
  }finally{await app.dispose();}
});

test('item rows remain editable in the table and removal does not overwrite the following item',async()=>{
  const app=await workspace();try{
    await app.action('Editar');
    const table=app.container.querySelector('table[aria-label="Ítems de cotización"]')!;
    assert.ok(table);assert.equal(table.querySelectorAll('tbody tr').length,1);
    await app.click(Array.from(app.container.querySelectorAll('button')).find(button=>button.textContent?.includes('Agregar ítem'))!);
    assert.equal(table.querySelectorAll('tbody tr').length,2);
    const description=table.querySelectorAll('textarea')[1]!;
    await act(async()=>{Object.getOwnPropertyDescriptor(window.HTMLTextAreaElement.prototype,'value')!.set!.call(description,'Segundo producto');description.dispatchEvent(new Event('input',{bubbles:true}));});
    await app.click(table.querySelector('button[aria-label="Quitar ítem 1"]'));
    assert.equal(table.querySelectorAll('tbody tr').length,1);
    assert.equal(table.querySelector('textarea')?.value,'Segundo producto');
    await act(async()=>{app.container.querySelector('form')!.dispatchEvent(new Event('submit',{bubbles:true,cancelable:true}));});
    const items=app.calls.find(call=>call.method==='PUT')?.body?.items as {description:string}[];
    assert.equal(items.length,1);assert.equal(items[0]?.description,'Segundo producto');
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
test('multiple selected files remain pending until the quote is saved and upload one by one with the returned versions',async()=>{
  const app=await workspace();try{
    await app.click(Array.from(app.container.querySelectorAll('button')).find(button=>button.textContent==='Nueva cotización')!);
    const input=app.container.querySelector<HTMLInputElement>('input[type="file"]')!;assert.equal(input.multiple,true);
    Object.defineProperty(input,'files',{configurable:true,value:[new window.File(['%PDF-synthetic'],'a.pdf'),new window.File(['data'],'b.txt')]});
    await act(async()=>{input.dispatchEvent(new Event('change',{bubbles:true}));});
    const upload=()=>Array.from(app.container.querySelectorAll<HTMLButtonElement>('button')).find(button=>button.textContent==='Subir 2 archivos')!;
    assert.equal(upload().disabled,true);assert.ok(app.container.textContent?.includes('Guarda la cotización'));
    assert.equal(app.calls.some(call=>call.path.endsWith('/attachments')&&call.method==='POST'),false);
    await act(async()=>{app.container.querySelector('form')!.dispatchEvent(new Event('submit',{bubbles:true,cancelable:true}));});
    assert.equal(upload().disabled,false);await app.click(upload());
    await act(async()=>{await new Promise(resolve=>setTimeout(resolve,30));});
    const sent=app.calls.filter(call=>call.path.endsWith('/attachments')&&call.method==='POST');
    assert.equal(sent.length,2);assert.equal(sent[0]?.match,'"2"');assert.equal(sent[1]?.match,'"3"');
    assert.ok(sent[0]?.key);assert.notEqual(sent[0]?.key,sent[1]?.key);
    assert.deepEqual(sent.map(call=>call.body?.filename),['a.pdf','b.txt']);
    assert.equal(app.container.textContent?.includes('pendiente de subir'),false);
    assert.ok(app.container.textContent?.includes('a.pdf'));assert.ok(app.container.textContent?.includes('b.txt'));
    await act(async()=>{app.container.querySelector('form')!.dispatchEvent(new Event('submit',{bubbles:true,cancelable:true}));});
    assert.equal(app.calls.find(call=>call.method==='PUT')?.match,'"4"');
  }finally{await app.dispose();}
});
test('a failed attachment retains its idempotency key and oversized selections do not upload',async()=>{
  const app=await workspace();try{
    await app.action('Editar');
    const picker=app.container.querySelector<HTMLInputElement>('input[type="file"]')!;
    const tooBig=new window.File(['x'],'huge.txt');Object.defineProperty(tooBig,'size',{value:21*1024*1024});
    Object.defineProperty(picker,'files',{configurable:true,value:[tooBig]});
    await act(async()=>{picker.dispatchEvent(new Event('change',{bubbles:true}));});
    assert.ok(app.container.textContent?.includes('hasta 20 MiB'));
    assert.equal(app.calls.some(call=>call.path.endsWith('/attachments')&&call.method==='POST'),false);
    Object.defineProperty(picker,'files',{configurable:true,value:[new window.File(['data'],'retry.txt')]});
    await act(async()=>{picker.dispatchEvent(new Event('change',{bubbles:true}));});
    app.fail(503);
    await app.click(Array.from(app.container.querySelectorAll('button')).find(button=>button.textContent==='Subir 1 archivo')!);
    await act(async()=>{await new Promise(resolve=>setTimeout(resolve,30));});
    assert.ok(app.container.textContent?.includes('pendiente de subir'));
    await app.click(Array.from(app.container.querySelectorAll('button')).find(button=>button.textContent==='Subir 1 archivo')!);
    await act(async()=>{await new Promise(resolve=>setTimeout(resolve,30));});
    const attempts=app.calls.filter(call=>call.path.endsWith('/attachments')&&call.method==='POST');
    assert.equal(attempts.length,2);assert.equal(attempts[0]?.key,attempts[1]?.key);assert.equal(attempts[0]?.match,attempts[1]?.match);
  }finally{await app.dispose();}
});
