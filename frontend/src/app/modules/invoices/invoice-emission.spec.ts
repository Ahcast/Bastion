import {TestBed} from '@angular/core/testing';
import {beforeEach,describe,expect,it,vi} from 'vitest';
import {Client} from '../../core/api/client';
import {InvoiceEmission,EmissionReview,InvoiceEmissionResult} from './invoice-emission';
const review=():EmissionReview=>({invoice_version:'9007199254740993',fiscal_version:'2',company_version:'3',fiscal:{cai:'CAI-SINTETICO',prefix:'TEST-',number_width:8,range_start:'1',range_end:'100',valid_from:'2026-10-01',valid_until:'2026-10-31',currency:'HNL',enabled:true},company:{name:'Empresa sintética',tax_id:'08011900123456',address:'Dirección'},estimated_number:'TEST-00000001',due_date:'2026-11-10'});
const result=():InvoiceEmissionResult=>({id:'9007199254740995',version:'9007199254740994',status:'issued',invoice_number:'TEST-00000002',issued_at:'2026-10-11T22:00:00Z',due_date:'2026-11-10'});
describe('Emisión de factura',()=>{
 let c:InvoiceEmission;let api:{has:ReturnType<typeof vi.fn>;get:ReturnType<typeof vi.fn>;write:ReturnType<typeof vi.fn>;message:ReturnType<typeof vi.fn>};
 beforeEach(()=>{api={has:vi.fn(()=>true),get:vi.fn().mockResolvedValue(review()),write:vi.fn().mockResolvedValue(result()),message:vi.fn(()=> 'Error de emisión')};TestBed.configureTestingModule({providers:[{provide:Client,useValue:api}]});c=TestBed.runInInjectionContext(()=>new InvoiceEmission());c.invoice={id:'9007199254740995',version:'9007199254740993',status:'draft'};c.ngOnChanges()});
 it('respeta cambios pendientes, permisos y versión revisada',async()=>{
 c.changesPending=true;await c.load();expect(api.get).not.toHaveBeenCalled();c.changesPending=false;api.has.mockReturnValue(false);await c.load();expect(api.get).not.toHaveBeenCalled();api.has.mockReturnValue(true);await c.load();c.reason='Revisada';expect(c.canEmit()).toBe(true);c.invoice={...c.invoice,version:'9007199254740994'};expect(c.canEmit()).toBe(false);await c.emit();expect(api.write).not.toHaveBeenCalled();
 });
 it('envía versiones exactas y usa número confirmado, sin enviar el estimado',async()=>{
 await c.load();c.reason='Revisión final';const emitted=vi.fn();c.issued.subscribe(emitted);await c.emit();expect(api.write.mock.calls[0].slice(0,3)).toEqual(['POST','/invoices/9007199254740995/emit',{version:'9007199254740993',fiscal_version:'2',company_version:'3',reason:'Revisión final'}]);expect(c.confirmed()?.invoice_number).toBe('TEST-00000002');expect(emitted).toHaveBeenCalledWith(result());await c.emit();expect(api.write).toHaveBeenCalledTimes(1);
 });
 it('recupera respuesta incierta mediante la misma intención',async()=>{
 await c.load();c.reason='Revisión';api.write.mockRejectedValueOnce(new Error('ACK perdido'));await c.emit();expect(c.confirmed()).toBeNull();expect(c.reason).toBe('Revisión');await c.emit();expect(api.write.mock.calls[0][3]).toBe(api.write.mock.calls[1][3]);expect(c.confirmed()).toEqual(result());
 });
 it('descarta contexto tardío si cambia factura o hay ediciones',async()=>{
 let resolve!:(v:EmissionReview)=>void;api.get.mockImplementation(()=>new Promise(r=>resolve=r));const loading=c.load();c.invoice={id:'10',version:'1',status:'draft'};c.ngOnChanges();resolve(review());await loading;expect(c.review()).toBeNull();
 });
 it('bloquea otras acciones durante la petición y siempre libera la ficha',async()=>{
 const busy=vi.fn();c.busyChange.subscribe(busy);await c.load();expect(busy.mock.calls.map(x=>x[0])).toEqual([true,false]);busy.mockClear();c.reason='Revisión';api.write.mockRejectedValue(new Error('Respuesta incierta'));await c.emit();expect(busy.mock.calls.map(x=>x[0])).toEqual([true,false]);c.disabled=true;await c.load();await c.emit();expect(api.write).toHaveBeenCalledTimes(1);
 });

});
