
import { after, test } from 'node:test';
import assert from 'node:assert/strict';
import { Window } from 'happy-dom';
import { act } from 'react';
import { ApiClient } from '../src/api.js';
import { CoreContext } from '../src/ui.js';
import { NavigationGuardProvider } from '../src/navigationGuard.js';
import Cotizaciones from '../src/pages/Cotizaciones.js';
import { newOrder } from '../src/orders.js';
import { newQuote } from '../src/quotes.js';
import type { OrderConversionSeed, OrderInput, PurchaseOrder } from '../src/orders.js';
import type { QuoteStatus } from '../src/quotes.js';

const window = new Window();
for (const name of ['document','HTMLElement','HTMLInputElement','HTMLSelectElement','HTMLTextAreaElement','FormData','Event','MouseEvent','KeyboardEvent'] as const) Object.defineProperty(globalThis,name,{configurable:true,value:window[name]});
Object.assign(globalThis,{window,IS_REACT_ACT_ENVIRONMENT:true});
const { createRoot } = await import('react-dom/client');
after(()=>window.happyDOM.abort());

async function workspace(status: QuoteStatus = 'Aprobada', canOrder = true, failures: (number | 'network')[] = []) {
  const quote={...newQuote(),status,client_name:'Cliente aprobado',order_number:'OC-APROBADA',notes:'Nota privada',terms:'Entrega acordada <script>dato</script>',discount_type:'percent' as const,discount_value:'5',
    items:[{description:'Equipo',quantity:'1.25',unit_price:'100.00',discount_type:'amount' as const,discount_value:'2.00',tax_type:'isv18' as const}]};
  const commercial={subtotal:'125.00',importe_exonerado:'0.00',importe_exento:'0.00',importe_gravado_15:'0.00',importe_gravado_18:'116.38',descuentos_rebajas:'8.62',isv_15:'0.00',isv_18:'20.95',total:'137.33'};
  const data:OrderInput={...newOrder(),n_orders:'OC-APROBADA',client_name:quote.client_name,approved_quote:quote,products:[{description:'Equipo',cantidad:'1.25',sale_price:'100.00',purchase_price:'100.00',tracking:'',isv:false,status:'Pendiente',coments:''}]};
  const calculation={data,status:'Pendiente' as const,totals:{subtotal:'125.00',subtotal_exonerado:'0.00',tax:'20.95',total:'137.33',products_investment:'125.00',other_expenses:'0.00',investment_total:'125.00',profit:'12.33',commercial}};
  const seed:OrderConversionSeed={source:{id:7,version:4,quote_number:'COT-007'},data,calculation};
  const calls:{path:string;method:string;body:string;match:string|null;key:string|null}[]=[];
  const api=new ApiClient('https://api.example.test',async (url,options={})=>{
    const path=new URL(String(url)).pathname,method=options.method??'GET',headers=new Headers(options.headers);
    calls.push({path,method,body:String(options.body??''),match:headers.get('If-Match'),key:headers.get('Idempotency-Key')});
    if(path.endsWith('order-preparation'))return Response.json(seed);
    if(path==='/api/v1/ordenes-compra/destinations')return Response.json([{id:1,version:1,destination:'Oficina'}]);
    if(path.endsWith('/calculate'))return Response.json(calculation);
    if(path.endsWith('/order')){
      const failure=failures.shift();
      if(failure==='network')throw new TypeError('synthetic network failure');
      if(failure)return Response.json({detail:'La cotización cambió.'},{status:failure});
      const saved:PurchaseOrder={id:10,version:1,status:'Pendiente',currency:'HNL',source_quote:seed.source,data:JSON.parse(String(options.body)),calculation};
      return Response.json(saved);
    }
    if(path.endsWith('document-readiness'))return Response.json({pdf:false,company:false,email:false});
    if(path==='/api/v1/cotizaciones')return Response.json({items:[{id:7,version:4,quote_number:'COT-007',client_name:quote.client_name,quote_date:quote.quote_date,valid_until:quote.quote_date,total:'137.33',status,item_count:1}],page:1,page_size:25,has_more:false});
    throw new Error('Unexpected '+path);
  });
  const permissions=['ver','editar'].map(capability=>({module_id:'cotizaciones',scope_key:'documentos',capability}));
  if(canOrder)permissions.push({module_id:'ordenes_compra',scope_key:'documentos',capability:'editar'});
  const access={is_system_administrator:false,modules:[{module_id:'cotizaciones',display_name:'Cotizaciones'},{module_id:'ordenes_compra',display_name:'Órdenes'}],permissions};
  const container=document.createElement('div');document.body.append(container);const root=createRoot(container);
  await act(async()=>root.render(<NavigationGuardProvider><CoreContext.Provider value={{api,access,notify:()=>{}}}><Cotizaciones/></CoreContext.Provider></NavigationGuardProvider>));
  const click=async(text:string)=>{const button=[...document.querySelectorAll('button')].find(b=>b.textContent===text);assert.ok(button,text);await act(async()=>button.click());};
  const action=async()=>{const menu=container.querySelector<HTMLButtonElement>('button[aria-haspopup="menu"]');assert.ok(menu);await act(async()=>menu.click());return [...document.querySelectorAll<HTMLButtonElement>('[role="menuitem"]')].find(b=>b.textContent?.startsWith('Convertir a orden de compra'))!;};
  const prepare=async()=>{const button=await action();await act(async()=>button.click());};
  const save=async()=>{const select=container.querySelector<HTMLSelectElement>('select[required]');assert.ok(select);if(!select.disabled){await act(async()=>{select.value='1';select.dispatchEvent(new Event('change',{bubbles:true}));});}await act(async()=>container.querySelector('form')!.dispatchEvent(new Event('submit',{bubbles:true,cancelable:true})));};
  return {container,calls,click,action,prepare,save,cleanup:async()=>{await act(async()=>root.unmount());container.remove();}};
}
test('conversion requires approval and order edit permission before requesting a draft',async()=>{
  for(const [status,permission]of [['Borrador',true],['Aprobada',false]] as const){
    const ui=await workspace(status,permission);try{assert.ok((await ui.action()).disabled);assert.ok(!ui.calls.some(c=>c.path.endsWith('order-preparation')));}finally{await ui.cleanup();}
  }
});
test('preparation retains decimal quantities and approved terms and cancelling creates no order',async()=>{
  const ui=await workspace();try{
    await ui.prepare();
    assert.equal(ui.calls.find(c=>c.path.endsWith('order-preparation'))?.match,'"4"');
    assert.ok(ui.container.textContent?.includes('COT-007'));
    assert.ok(ui.container.textContent?.includes('ISV 18%'));
    assert.ok(ui.container.textContent?.includes('Entrega acordada'));
    assert.ok(ui.container.textContent?.includes('<script>dato</script>'));
    assert.equal(ui.container.querySelector('script'),null);
    const quantity=ui.container.querySelector<HTMLInputElement>('input[type="number"]')!;
    assert.equal(quantity.value,'1.25');assert.ok(quantity.readOnly);
    assert.ok([...ui.container.querySelectorAll('button')].find(b=>b.textContent==='Agregar producto')?.disabled);
    await ui.click('Volver');await ui.click('Descartar y continuar');
    assert.ok(!ui.calls.some(c=>c.path.endsWith('/order')));
  }finally{await ui.cleanup();}
});
test('ambiguous conversion retries with identical source version body and key',async()=>{
  const ui=await workspace('Aprobada',true,['network']);try{
    await ui.prepare();await ui.save();
    assert.ok(ui.container.textContent?.includes('No se pudo confirmar'));
    await act(async()=>ui.container.querySelector('form')!.dispatchEvent(new Event('submit',{bubbles:true,cancelable:true})));
    const calls=ui.calls.filter(c=>c.path.endsWith('/order'));
    assert.equal(calls.length,2);assert.ok(calls[0]);assert.ok(calls[1]);assert.deepEqual(calls[0],calls[1]);assert.ok(calls[0].key);assert.equal(calls[0].match,'"4"');
    assert.equal(JSON.parse(calls[0].body).approved_quote.items[0].tax_type,'isv18');
    assert.equal(JSON.parse(calls[0].body).id_destination,1);
    assert.ok(ui.container.textContent?.includes('Orden · OC-APROBADA'));
  }finally{await ui.cleanup();}
});
test('stale quote blocks conversion without trying to reload a missing saved order',async()=>{
  const ui=await workspace('Aprobada',true,[412]);try{
    await ui.prepare();await ui.save();
    assert.ok(ui.container.textContent?.includes('La cotización cambió'));
    assert.ok(![...ui.container.querySelectorAll('button')].some(b=>b.textContent==='Abrir versión actual'));
    const submit=ui.container.querySelector<HTMLButtonElement>('button[type="submit"]');assert.ok(submit?.disabled);
  }finally{await ui.cleanup();}
});
