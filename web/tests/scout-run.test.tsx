import { after, test } from 'node:test';
import assert from 'node:assert/strict';
import { Window } from 'happy-dom';
import { act } from 'react';
import { ApiClient } from '../src/api.js';
import { CoreContext } from '../src/ui.js';
import { ScoutRunControl } from '../src/components/ScoutRunControl.js';
import type { ScoutControlStatus } from '../src/components/ScoutRunControl.js';
const window = new Window();
for (const name of ['document','HTMLElement','Event','MouseEvent'] as const) Object.defineProperty(globalThis,name,{configurable:true,value:window[name]});
Object.assign(globalThis,{window,IS_REACT_ACT_ENVIRONMENT:true});
const { createRoot } = await import('react-dom/client');
after(()=>window.happyDOM.abort());

async function workspace(editable=true, initial: ScoutControlStatus={available:true,busy:false,job:null}) {
  let fail=false, refreshes=0;
  const calls: {method:string;key:string|null;body:unknown}[]=[];
  const api=new ApiClient('https://api.example.test',async (_url,options)=>{
    const method=options?.method??'GET';
    calls.push({method,key:new Headers(options?.headers).get('Idempotency-Key'),body:options?.body?JSON.parse(String(options.body)):null});
    if(method==='POST'&&fail){fail=false;return Response.json({detail:'Respuesta temporalmente no disponible'},{status:503});}
    return Response.json(method==='POST'?{available:true,busy:true,job:{id:'opaque',status:'running',started_at:'2026-10-24T20:00:00Z',finished_at:null}}:initial);
  });
  const container=document.createElement('div');document.body.append(container);const root=createRoot(container);
  await act(async()=>root.render(<CoreContext.Provider value={{api,access:{is_system_administrator:false,modules:[{module_id:'procesos',display_name:'Procesos'}],permissions:[{module_id:'procesos',scope_key:'consulta',capability:'ver'},...(editable?[{module_id:'procesos',scope_key:'importacion',capability:'editar'}]:[])]},notify:()=>{}}}><ScoutRunControl refreshReports={()=>{refreshes++;}}/></CoreContext.Provider>));
  const button=()=>container.querySelector('button')!;
  const click=async()=>{await act(async()=>{button().dispatchEvent(new MouseEvent('click',{bubbles:true}));});};
  return {container,calls,button,click,refreshes:()=>refreshes,fail:()=>{fail=true;},dispose:async()=>{await act(async()=>root.unmount());container.remove();}};
}

test('opening settings only reads status and viewers cannot run Scout',async()=>{
  const app=await workspace(false);try{assert.equal(app.calls.length,1);assert.equal(app.calls[0]?.method,'GET');assert.equal(app.button().disabled,true);assert.equal(app.container.querySelector('[role="dialog"]'),null);}finally{await app.dispose();}
});
test('one click starts the complete pipeline with an empty body and idempotency key without a dialog',async()=>{
  const app=await workspace();try{
    await act(async()=>{app.button().dispatchEvent(new MouseEvent('click',{bubbles:true}));app.button().dispatchEvent(new MouseEvent('click',{bubbles:true}));});
    const posts=app.calls.filter(c=>c.method==='POST');assert.equal(posts.length,1);assert.deepEqual(posts[0]?.body,{});assert.ok(posts[0]?.key);
    assert.equal(app.button().disabled,true);assert.ok(app.container.textContent?.includes('En ejecución'));assert.ok(app.refreshes()>0);assert.equal(app.container.querySelector('[role="dialog"]'),null);
  }finally{await app.dispose();}
});
test('ambiguous failures retain the same request key for an explicit retry',async()=>{
  const app=await workspace();try{app.fail();await app.click();assert.equal(app.button().disabled,false);await app.click();const posts=app.calls.filter(c=>c.method==='POST');assert.equal(posts.length,2);assert.equal(posts[0]?.key,posts[1]?.key);}finally{await app.dispose();}
});
test('busy automatic runs and unavailable controllers disable start; partial result remains explicit',async()=>{
  for(const status of [{available:true,busy:true,job:null},{available:false,busy:false,job:null}]){
    const app=await workspace(true,status);try{assert.equal(app.button().disabled,true);assert.equal(app.calls.filter(c=>c.method==='POST').length,0);}finally{await app.dispose();}
  }
  const app=await workspace(true,{available:true,busy:false,job:{id:'test',status:'partial',started_at:'2026-10-24T20:00:00Z',finished_at:'2026-10-24T20:01:00Z'}});try{assert.ok(app.container.textContent?.includes('Completado parcialmente'));assert.equal(app.button().disabled,false);}finally{await app.dispose();}
});
