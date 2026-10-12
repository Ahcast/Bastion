import {TestBed} from '@angular/core/testing';
import {beforeEach,describe,expect,it,vi} from 'vitest';
import {Client} from '../../core/api/client';
import {InvoiceCancellation,CancellationResult} from './invoice-cancellation';

describe('Anulación de factura',()=>{
 let c:InvoiceCancellation;let api:{has:ReturnType<typeof vi.fn>;write:ReturnType<typeof vi.fn>;message:ReturnType<typeof vi.fn>};
 const result:CancellationResult={id:'9007199254740993',version:'8',status:'cancelled',invoice_number:'TEST-00000007',cancelled_at:'2026-10-12T00:00:00Z'};
 beforeEach(()=>{api={has:vi.fn(()=>true),write:vi.fn().mockResolvedValue(result),message:vi.fn(()=> 'No se pudo anular')};TestBed.configureTestingModule({providers:[{provide:Client,useValue:api}]});c=TestBed.runInInjectionContext(()=>new InvoiceCancellation());c.invoice={id:result.id,version:'7',status:'issued'};c.ngOnChanges();c.reason='Error revisado';c.attested=true});
 it('conserva ID/version como cadenas y exige declaración y permisos',async()=>{c.attested=false;await c.cancel();c.attested=true;c.changesPending=true;await c.cancel();c.changesPending=false;api.has.mockReturnValue(false);await c.cancel();expect(api.write).not.toHaveBeenCalled();api.has.mockReturnValue(true);const changed=vi.fn();c.cancelled.subscribe(changed);await c.cancel();expect(api.write.mock.calls[0].slice(0,3)).toEqual(['POST','/invoices/'+result.id+'/cancel',{version:'7',reason:'Error revisado',original_and_copy_marked_and_archived:true}]);expect(changed).toHaveBeenCalledWith(result)});
 it('recupera ACK incierto con la misma clave sin duplicar intención',async()=>{api.write.mockRejectedValueOnce(new Error('ACK perdido'));await c.cancel();expect(c.error()).toBe('No se pudo anular');await c.cancel();expect(api.write.mock.calls[0][3]).toBe(api.write.mock.calls[1][3]);expect(c.confirmed()).toEqual(result)});
 it('bloquea factura OC y no vuelve a enviar después de anular',async()=>{c.invoice={...c.invoice,source_purchase_order_id:'19'};await c.cancel();expect(api.write).not.toHaveBeenCalled();c.invoice={...c.invoice,source_purchase_order_id:null};await c.cancel();c.invoice={...c.invoice,status:'cancelled',version:'8'};c.ngOnChanges();await c.cancel();expect(api.write).toHaveBeenCalledTimes(1)});
});
