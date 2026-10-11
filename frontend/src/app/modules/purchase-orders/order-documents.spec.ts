import {TestBed} from '@angular/core/testing';
import {vi,describe,it,beforeEach,expect} from 'vitest';
import {Client} from '../../core/api/client';
import {OrderDocuments} from './order-documents';
import {emptyPurchaseOrder,PurchaseOrder} from './purchase-orders';
describe('Documentos privados OC',()=>{
 let api:{get:ReturnType<typeof vi.fn>;write:ReturnType<typeof vi.fn>;has:ReturnType<typeof vi.fn>;message:ReturnType<typeof vi.fn>};
 beforeEach(()=>{api={get:vi.fn().mockResolvedValue({data:[],total:0}),write:vi.fn(),has:vi.fn(()=>true),message:vi.fn(()=> 'Falló')};TestBed.configureTestingModule({providers:[{provide:Client,useValue:api}]})});
 function setup(){const f=TestBed.createComponent(OrderDocuments);f.componentRef.setInput('order',{...emptyPurchaseOrder(),id:'9007199254740993',status:'received',version:'7',items:[]} as unknown as PurchaseOrder);return f}
 it('conserva intención tras timeout y publica versión antes de recarga fallida',async()=>{
 const f=setup(),c=f.componentInstance;c.documentId='9007199254740994';c.target='9007199254740995';api.write.mockRejectedValue(new Error());await c.link();await c.link();expect(api.write.mock.calls[1][3]).toBe(api.write.mock.calls[0][3]);expect(api.write.mock.calls[0][2]).toEqual({document_id:c.documentId,document_version:'1',version:'7',item_id:c.target});const linked=vi.fn();c.linked.subscribe(linked);api.write.mockResolvedValue({version:'8'});api.get.mockRejectedValue(new Error());await c.link();expect(linked).toHaveBeenCalledWith('8');expect(c.error()).toBe('Falló');expect(c.busy()).toBe(false)
 });
 it('bloquea adjuntos con edición pendiente y usa cabecera explícita nullable',async()=>{const f=setup(),c=f.componentInstance;c.documentId='5';f.componentRef.setInput('changesPending',true);await c.link();expect(api.write).not.toHaveBeenCalled();f.componentRef.setInput('changesPending',false);api.write.mockResolvedValue({version:'8'});await c.link();expect(api.write.mock.calls[0][2].item_id).toBeNull()});
 it('filtra papelera/ítem sin pérdida de IDs y no ofrece writes cerrados',async()=>{const f=setup(),c=f.componentInstance;c.filterItem='9007199254740995';c.trash=true;c.page=2;await c.load();expect(api.get).toHaveBeenCalledWith('/purchase-orders/9007199254740993/documents?page=2&per_page=25&deleted=true&item_id=9007199254740995');f.componentRef.setInput('order',{...c.order(),status:'closed'});expect(c.writable()).toBe(false);expect(c.status('quarantined')).toBe('En cuarentena')});
});
