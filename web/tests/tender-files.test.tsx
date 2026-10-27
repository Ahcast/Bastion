import { after,test } from 'node:test';
import assert from 'node:assert/strict';
import { Window } from 'happy-dom';
import { ApiClient } from '../src/api.js';
import { CoreContext } from '../src/ui.js';
import { NavigationGuardProvider } from '../src/navigationGuard.js';
import TenderFiles from '../src/components/TenderFiles.js';
import { maxTenderFileBytes,validateTenderFile,tenderContentBlob } from '../src/tenderAttachments.js';
import type { Access } from '../src/types.js';
import type { Expedient } from '../src/tenders.js';

const window=new Window();
for(const key of ['document','HTMLElement','HTMLInputElement','Event','MouseEvent','navigator','File','FileReader'] as const) Object.defineProperty(globalThis,key,{configurable:true,value:window[key]});
Object.assign(globalThis,{window,IS_REACT_ACT_ENVIRONMENT:true});
const {act}=await import('react');const {createRoot}=await import('react-dom/client');
after(()=>window.happyDOM.abort());
const access:Access={is_system_administrator:false,modules:[{module_id:'licitaciones',display_name:'Licitaciones'},{module_id:'colaboracion',display_name:'Colaboración'}],permissions:[{module_id:'licitaciones',scope_key:'expedientes',capability:'ver'},{module_id:'licitaciones',scope_key:'expedientes',capability:'editar'},{module_id:'colaboracion',scope_key:'persona:7',capability:'ver'}]};
const record:Expedient={id:7,name:'Expediente sintético',version:1,state:'borrador',forms:[]};
const list={items:[],expedient_version:1,retained_bytes:0};
const click=async(text:string,scope:ParentNode)=>{const b=[...scope.querySelectorAll('button')].find(b=>b.textContent===text);assert.ok(b,text);await act(async()=>b.click());};
test('tender file limits use decimal MB and reject active formats before encoding',()=>{
 assert.doesNotThrow(()=>validateTenderFile({name:'documento.pdf',size:maxTenderFileBytes}));
 assert.throws(()=>validateTenderFile({name:'documento.pdf',size:maxTenderFileBytes+1}));
 assert.throws(()=>validateTenderFile({name:'a.svg',size:10}));
 assert.throws(()=>tenderContentBlob({filename:'a.svg',mime:'image/svg+xml',content_base64:btoa('<svg/>')},true));
});
test('PC batch uploads retain idempotency keys for pending files and advance the version',async()=>{
 const posts:RequestInit[]=[];let fail=true;
 const api=new ApiClient('https://example.test',async(_url,init={})=>{
  if(init.method==='POST'){posts.push(init);if(posts.length===2&&fail)return Response.json({detail:'Fallo temporal'},{status:503});return Response.json({...record,version:posts.length===1?2:3});}
  return Response.json(list);
 });
 const container=document.createElement('div');document.body.append(container);const root=createRoot(container);
 try{
  await act(async()=>root.render(<NavigationGuardProvider><CoreContext.Provider value={{api,access,notify:()=>{}}}><TenderFiles record={record} section="legal" changed={()=>{}} start={action=>action()}/></CoreContext.Provider></NavigationGuardProvider>));
  await click('Subir desde mi PC',container);const dialog=document.body.querySelector('[role="dialog"]')!;const input=dialog.querySelector<HTMLInputElement>('input[type="file"]')!;
  assert.equal(input.multiple,true);
  Object.defineProperty(input,'files',{configurable:true,value:[new window.File(['%PDF-1.4'],'legal.pdf',{type:'application/pdf'}),new window.File(['image'],'foto.png',{type:'image/png'})]});
  await act(async()=>input.dispatchEvent(new Event('change',{bubbles:true})));
  await act(async()=>dialog.querySelector('form')!.dispatchEvent(new Event('submit',{bubbles:true,cancelable:true})));
  assert.equal(posts.length,2);assert.equal(new Headers(posts[0]!.headers).get('if-match'),'"1"');assert.equal(new Headers(posts[1]!.headers).get('if-match'),'"2"');
  const oldKey=new Headers(posts[1]!.headers).get('idempotency-key');fail=false;
  await act(async()=>dialog.querySelector('form')!.dispatchEvent(new Event('submit',{bubbles:true,cancelable:true})));
  assert.equal(posts.length,3);assert.equal(new Headers(posts[2]!.headers).get('idempotency-key'),oldKey);
  assert.equal(JSON.parse(String(posts[2]!.body)).section,'legal');
  assert.equal(JSON.parse(String(posts[2]!.body)).filename,'foto.png');assert.equal(JSON.parse(String(posts[0]!.body)).filename,'legal.pdf');
 }finally{await act(async()=>root.unmount());container.remove();}
});
test('Collaboration selection respects library read access and copies into the technical block',async()=>{
 const posts:RequestInit[]=[];
 const api=new ApiClient('https://example.test',async(url,init={})=>{
  if(init.method==='POST'){posts.push(init);return Response.json({...record,version:2});}
  if(new URL(String(url)).pathname==='/api/v1/colaboracion')return Response.json({items:[{id:4,filename:'permitida.pdf',mime:'application/pdf',size_bytes:10},{id:5,filename:'grande.pdf',mime:'application/pdf',size_bytes:20_000_001},{id:6,filename:'activo.svg',mime:'image/svg+xml',size_bytes:10}],page:1,page_size:25,has_more:false});
  return Response.json(list);
 });
 const container=document.createElement('div');document.body.append(container);const root=createRoot(container);
 try{
  await act(async()=>root.render(<NavigationGuardProvider><CoreContext.Provider value={{api,access,notify:()=>{}}}><TenderFiles record={record} section="tecnica" changed={()=>{}} start={action=>action()}/></CoreContext.Provider></NavigationGuardProvider>));
  await click('Seleccionar de Colaboración',container);const dialog=document.body.querySelector('[role="dialog"]')!;
  assert.ok(!dialog.textContent?.includes('activo.svg'));
  const buttons=[...dialog.querySelectorAll('button')].filter(b=>b.textContent==='Incorporar archivo');assert.equal(buttons.length,2);assert.equal(buttons[1]!.disabled,true);
  await act(async()=>buttons[0]!.click());
  assert.deepEqual(JSON.parse(String(posts[0]!.body)),{section:'tecnica',source_id:4});assert.equal(new Headers(posts[0]!.headers).get('if-match'),'"1"');
 }finally{await act(async()=>root.unmount());container.remove();}
});
