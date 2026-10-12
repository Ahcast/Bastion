import {TestBed} from '@angular/core/testing';
import {beforeEach,describe,expect,it,vi} from 'vitest';
import {Client} from '../../core/api/client';
import {InvoiceCloning} from './invoice-cloning';

describe('Clonación de factura',()=>{
 let c:InvoiceCloning;let api:{has:ReturnType<typeof vi.fn>;write:ReturnType<typeof vi.fn>;message:ReturnType<typeof vi.fn>};
 const result={id:'9007199254740993',version:'1'};
 beforeEach(()=>{api={has:vi.fn(()=>true),write:vi.fn().mockResolvedValue(result),message:vi.fn(()=>'No se pudo clonar')};TestBed.configureTestingModule({providers:[{provide:Client,useValue:api}]});c=TestBed.runInInjectionContext(()=>new InvoiceCloning());c.invoice={id:'9007199254740992',version:'7',status:'issued',invoice_date:'2026-10-12',public_notes:'Anterior'};c.ngOnChanges()});
 it('usa fecha y notas explícitas, conserva identificadores grandes y requiere permisos',async()=>{c.date='2026-10-13';c.notes='Nueva';c.changesPending=true;await c.clone();c.changesPending=false;api.has.mockReturnValue(false);await c.clone();expect(api.write).not.toHaveBeenCalled();api.has.mockReturnValue(true);const changed=vi.fn();c.cloned.subscribe(changed);await c.clone();expect(api.write.mock.calls[0].slice(0,3)).toEqual(['POST','/invoices/9007199254740992/clone',{version:'7',invoice_date:'2026-10-13',public_notes:'Nueva'}]);expect(changed).toHaveBeenCalledWith(result)});
 it('reintenta la misma intención tras ACK incierto y bloquea una anulada',async()=>{api.write.mockRejectedValueOnce(new Error('ACK'));await c.clone();await c.clone();expect(api.write.mock.calls[0][3]).toBe(api.write.mock.calls[1][3]);expect(c.confirmed()).toEqual(result);c.invoice={...c.invoice,status:'cancelled',version:'8'};c.ngOnChanges();await c.clone();expect(api.write).toHaveBeenCalledTimes(2)});
});
