import {TestBed} from '@angular/core/testing';
import {vi,describe,it,expect,beforeEach} from 'vitest';
import {Client} from '../../core/api/client';
import {QuotePurchaseOrder} from './quote-purchase-order';
import {empty,Quote} from './quotes';

describe('Conversión cotización a OC',()=>{
 let component:QuotePurchaseOrder;let api:{write:ReturnType<typeof vi.fn>;message:ReturnType<typeof vi.fn>;has:ReturnType<typeof vi.fn>;moduleEnabled:ReturnType<typeof vi.fn>};let record:Quote;
 beforeEach(()=>{
  api={write:vi.fn(),message:vi.fn(()=> 'Falló'),has:vi.fn(()=>true),moduleEnabled:vi.fn(()=>true)};
  TestBed.configureTestingModule({providers:[{provide:Client,useValue:api}]});
  const fixture=TestBed.createComponent(QuotePurchaseOrder);component=fixture.componentInstance;
  record={...empty(),id:'9007199254740993',number:'COT-1',status:'accepted',version:'6',source_process_id:'',deleted_at:null,items:[],calculation:{total:'201.25'}};
  fixture.componentRef.setInput('quote',record);component.orderNumber=' OC-1 ';component.orderDate='2026-10-11';
 });
 it('envía sólo datos nuevos de OC y conserva clave/payload ante timeout',async()=>{
  api.write.mockRejectedValue(new Error());await component.convert();await component.convert();
  const first=api.write.mock.calls[0];expect(first[1]).toBe('/quotes/9007199254740993/convert-to-purchase-order');expect(first[2]).toEqual({version:'6',order_number:'OC-1',destination:'',order_date:'2026-10-11',required_delivery_date:null,comments:'',additional_expenses:'0'});expect(api.write.mock.calls[1][3]).toBe(first[3]);
  component.orderNumber='OC-2';await component.convert();expect(api.write.mock.calls[2][3]).not.toBe(first[3]);expect(component.error()).toBe('Falló');
 });
 it('rechaza contenido pendiente y requiere aceptación, módulo y permisos',async()=>{
  const fixture=TestBed.createComponent(QuotePurchaseOrder);fixture.componentRef.setInput('quote',record);fixture.componentRef.setInput('changesPending',true);await fixture.componentInstance.convert();expect(api.write).not.toHaveBeenCalled();expect(fixture.componentInstance.error()).toContain('Guarde');
  fixture.componentRef.setInput('quote',{...record,status:'approved'});expect(fixture.componentInstance.available()).toBe(false);fixture.componentRef.setInput('quote',record);api.moduleEnabled.mockReturnValue(false);expect(component.available()).toBe(false);api.moduleEnabled.mockReturnValue(true);api.has.mockImplementation(p=>p!=='ordenes.create');expect(component.available()).toBe(false);
 });
 it('publica resultado confirmado antes de cualquier recarga y bloquea doble envío simultáneo',async()=>{
  const result={id:record.id,version:'7',status:'converted',purchase_order_id:'9007199254740994',purchase_order_version:'1'};const emitted=vi.fn();component.converted.subscribe(emitted);
  let resolve!:(value:typeof result)=>void;api.write.mockImplementation(()=>new Promise(r=>resolve=r));const pending=component.convert();await component.convert();expect(api.write).toHaveBeenCalledTimes(1);resolve(result);await pending;expect(emitted).toHaveBeenCalledWith(result);expect(component.busy()).toBe(false);
 });
});
