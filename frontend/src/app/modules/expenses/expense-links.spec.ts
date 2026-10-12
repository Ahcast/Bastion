import {TestBed} from '@angular/core/testing';
import {beforeEach,describe,expect,it,vi} from 'vitest';
import {Client} from '../../core/api/client';
import {ExpenseLinks} from './expense-links';

describe('Referencias y comprobantes de gastos',()=>{
 let c:ExpenseLinks;
 let api:{has:ReturnType<typeof vi.fn>;moduleEnabled:ReturnType<typeof vi.fn>;write:ReturnType<typeof vi.fn>;get:ReturnType<typeof vi.fn>;message:ReturnType<typeof vi.fn>};
 const expense={id:'9007199254740993',version:'4',deleted_at:null};
 beforeEach(()=>{
  api={has:vi.fn(()=>true),moduleEnabled:vi.fn(()=>true),write:vi.fn(),get:vi.fn(),message:vi.fn((e:unknown)=>e instanceof Error?e.message:'Error sintético')};
  TestBed.configureTestingModule({providers:[{provide:Client,useValue:api}]});
  c=TestBed.runInInjectionContext(()=>new ExpenseLinks());
  c.expense={...expense};
 });
 it('captura la versión viva de cada referencia y conserva la clave tras ACK incierto',async()=>{
  api.get.mockImplementation(async(path:string)=>path==='/processes/42'?{version:'7'}:{});
  api.write.mockRejectedValueOnce(new Error('ACK perdido')).mockResolvedValueOnce({id:expense.id,version:'5'});
  c.referenceIds.processes='42';c.referenceReason='Evidencia revisada';
  await c.saveReferences();await c.saveReferences();
  expect(api.get).toHaveBeenCalledWith('/processes/42');
  const first=api.write.mock.calls[0],second=api.write.mock.calls[1];
  expect(first[0]).toBe('POST');expect(first[1]).toBe(`/expenses/${expense.id}/references`);
  expect(first[2]).toEqual({version:'4',reason:'Evidencia revisada',references:[{entity_type:'processes',entity_id:'42',entity_version:'7'}]});
  expect(first[3]).toBe(second[3]);
  expect(c.error()).toBe('');
 });
 it('bloquea una nueva referencia si el módulo propietario está deshabilitado',async()=>{
  api.moduleEnabled.mockImplementation((id:string)=>id!=='processes');
  c.referenceIds.processes='42';c.referenceReason='Intento inválido';
  await c.saveReferences();
  expect(api.write).not.toHaveBeenCalled();
  expect(c.error()).toContain('deshabilitado');
 });
 it('sube por URL firmada, finaliza en Core y enlaza sin marcar el archivo limpio',async()=>{
  const changed=vi.fn();c.changed.subscribe(changed);
  c.file.set(new File(['pdf'],'factura.pdf',{type:'application/pdf'}));
  (c as unknown as {digest:(file:File)=>Promise<{hex:string;base64:string}>}).digest=vi.fn().mockResolvedValue({hex:'a'.repeat(64),base64:'YWJj'});
  const put=vi.fn().mockResolvedValue({ok:true});vi.stubGlobal('fetch',put);
  api.write.mockImplementation(async(_method:string,path:string)=>{
   if(path==='/documents')return{id:'33',number:1,status:'pending',upload_url:'https://storage.example/upload',checksum_base64:'YWJj'};
   if(path==='/documents/33/finalize')return{};
   if(path===`/expenses/${expense.id}/documents`)return{id:expense.id,version:'5'};
   throw new Error('ruta inesperada '+path);
  });
  await c.uploadAndLink();
  expect(put).toHaveBeenCalledWith('https://storage.example/upload',expect.objectContaining({method:'PUT',headers:{'Content-Type':'application/pdf','x-amz-checksum-sha256':'YWJj'}}));
  expect(api.write.mock.calls.map(x=>x[1])).toEqual(['/documents','/documents/33/finalize',`/expenses/${expense.id}/documents`]);
  expect(api.write.mock.calls.some(x=>String(x[1]).includes('/scan'))).toBe(false);
  expect(api.write.mock.calls[2][2]).toEqual({version:'4',document_id:'33',document_version:'1'});
  expect(c.pendingDocument()).toBeNull();
  expect(changed).toHaveBeenCalledWith({id:expense.id,version:'5',message:'Comprobante del gasto vinculado.'});
  vi.unstubAllGlobals();
 });
 it('retiene el comprobante finalizado y reutiliza la misma clave si el vínculo pierde ACK',async()=>{
  c.file.set(new File(['pdf'],'factura.pdf',{type:'application/pdf'}));
  (c as unknown as {digest:(file:File)=>Promise<{hex:string;base64:string}>}).digest=vi.fn().mockResolvedValue({hex:'b'.repeat(64),base64:'YmJi'});
  vi.stubGlobal('fetch',vi.fn().mockResolvedValue({ok:true}));
  let linkAttempts=0;
  api.write.mockImplementation(async(_method:string,path:string,_body:unknown,key?:string)=>{
   if(path==='/documents')return{id:'34',number:1,status:'pending',upload_url:'https://storage.example/upload2',checksum_base64:'YmJi'};
   if(path==='/documents/34/finalize')return{};
   if(path===`/expenses/${expense.id}/documents`){linkAttempts++;if(linkAttempts===1)throw new Error('ACK perdido');return{id:expense.id,version:'5',key};}
   throw new Error('ruta inesperada '+path);
  });
  await c.uploadAndLink();
  const firstKey=api.write.mock.calls[2][3];
  expect(c.pendingDocument()?.id).toBe('34');
  await c.retryPendingLink();
  expect(api.write.mock.calls[3][3]).toBe(firstKey);
  expect(api.write.mock.calls.filter(x=>x[1]==='/documents')).toHaveLength(1);
  expect(c.pendingDocument()).toBeNull();
  vi.unstubAllGlobals();
 });
 it('impide mutar referencias o comprobantes con cambios locales o gasto en papelera',()=>{
  c.changesPending=true;expect(c.canWriteReferences()).toBe(false);expect(c.canUpload()).toBe(false);
  c.changesPending=false;c.expense={...expense,deleted_at:'2026-10-12T12:00:00Z'};expect(c.canWriteReferences()).toBe(false);expect(c.canUpload()).toBe(false);
 });
});
