import {signal} from '@angular/core';
import {TestBed} from '@angular/core/testing';
import {Client} from '../../core/api/client';
import {QuoteEmail} from './quote-email';
import {beforeEach,describe,expect,it,vi} from 'vitest';

describe('Correo comercial de cotizaciones',()=>{
 let component:QuoteEmail;
 let api:{get:ReturnType<typeof vi.fn>;write:ReturnType<typeof vi.fn>;message:ReturnType<typeof vi.fn>;has:ReturnType<typeof vi.fn>;profile:ReturnType<typeof signal>};
 const quote={id:'9',number:'COT-2026-9',version:'4',status:'approved',created_by:'1',deleted_at:null,customer:{email:'cliente@example.com'}};
 beforeEach(()=>{
  api={get:vi.fn().mockResolvedValue({data:[],total:0}),write:vi.fn().mockResolvedValue({}),message:vi.fn(()=> 'Falló'),has:vi.fn(()=>true),profile:signal({user:{id:'2'}})};
  TestBed.configureTestingModule({providers:[{provide:Client,useValue:api}]});
  component=TestBed.runInInjectionContext(()=>new QuoteEmail());
  component.quote=quote;
 });
 it('conserva el destinatario introducido mientras finaliza la actualización del ledger',async()=>{
  api.has.mockImplementation((p:string)=>p!=='core.documents.read');
  const fixture=TestBed.createComponent(QuoteEmail);
  fixture.componentInstance.quote=quote;
  fixture.componentInstance.resetForm();
  fixture.detectChanges();
  const input=fixture.nativeElement.querySelector('input[placeholder="cliente@ejemplo.com"]') as HTMLInputElement;
  input.value='destino@example.test';input.dispatchEvent(new Event('input',{bubbles:true}));
  expect(fixture.componentInstance.to).toBe('destino@example.test');
  await fixture.whenStable();fixture.detectChanges();
  expect(input.value).toBe('destino@example.test');
 });
 it('solicita autorización exacta y no envía automáticamente',async()=>{
  component.pdfSnapshotId='15';component.to='cliente@example.com; compras@example.com';component.cc='copia@example.com';component.bcc='';component.subject='Cotización COT-2026-9';component.message='Adjuntamos la cotización aprobada.';component.scheduledAt='2026-10-11T09:30';component.expiresAt='2026-10-11T12:00';component.reason='Revisión previa al envío';component.attachmentIds=['21'];
  await component.requestAuthorization();
  expect(api.write).toHaveBeenCalledTimes(1);
  expect(api.write.mock.calls[0][1]).toBe('/quotes/9/email-authorizations');
  expect(api.write.mock.calls[0][2]).toEqual({quote_version:'4',pdf_snapshot_id:'15',to:['cliente@example.com','compras@example.com'],cc:['copia@example.com'],bcc:[],attachment_ids:['21'],subject:'Cotización COT-2026-9',message:'Adjuntamos la cotización aprobada.',scheduled_at:'2026-10-11T15:30:00.000Z',expires_at:'2026-10-11T18:00:00.000Z',reason:'Revisión previa al envío'});
  expect(api.write.mock.calls[0][1]).not.toContain('/send');
 });
 it('solo habilita enviar una intención aprobada de la versión actual',async()=>{
  const intent={id:'30',quote_version:'4',pdf_snapshot_id:'15',created_by:'1',to:['cliente@example.com'],cc:[],bcc:[],attachment_ids:[],subject:'Cotización',message:'Mensaje',scheduled_at:'2026-10-11T15:30:00Z',approval_id:'31',approval_version:'2',approval_status:'approved',requested_by:'1',status:'approved',created_at:'2026-10-10T15:00:00Z',updated_at:'2026-10-10T15:00:00Z'};
  expect(component.canSend(intent)).toBe(true);
  expect(component.canSend({...intent,quote_version:'3'})).toBe(false);
  expect(component.canSend({...intent,status:'queued'})).toBe(false);
  await component.send(intent);
  expect(api.write.mock.calls[0][1]).toBe('/quotes/9/email-authorizations/30/send');
  expect(api.write.mock.calls[0][2]).toEqual({});
 });
 it('impide autoaprobación y conserva explícito el estado SMTP incierto',()=>{
  const intent={id:'30',quote_version:'4',pdf_snapshot_id:'15',created_by:'1',to:['cliente@example.com'],cc:[],bcc:[],attachment_ids:[],subject:'Cotización',message:'Mensaje',scheduled_at:'2026-10-11T15:30:00Z',approval_id:'31',approval_version:'1',approval_status:'pending',requested_by:'2',status:'pending_approval',created_at:'2026-10-10T15:00:00Z',updated_at:'2026-10-10T15:00:00Z'};
  expect(component.canDecide(intent)).toBe(false);
  api.profile.set({user:{id:'3'}});
  expect(component.canDecide(intent)).toBe(true);
  expect(component.statusLabel('uncertain')).toBe('Resultado incierto');
  expect(component.errorLabel('external_result_uncertain')).toContain('resultado externo');
 });
});
