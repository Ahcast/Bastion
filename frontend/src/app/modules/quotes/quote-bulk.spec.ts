import {TestBed} from '@angular/core/testing';
import {describe,it,expect,beforeEach,vi} from 'vitest';
import {Client} from '../../core/api/client';
import {QuoteBulk} from './quote-bulk';

describe('Cotizaciones en lote',()=>{
 let bulk:QuoteBulk;
 let api:{write:ReturnType<typeof vi.fn>;message:ReturnType<typeof vi.fn>;has:ReturnType<typeof vi.fn>};
 beforeEach(()=>{api={write:vi.fn(),message:vi.fn(()=> 'Falló'),has:vi.fn(()=>true)};TestBed.configureTestingModule({providers:[{provide:Client,useValue:api}]});bulk=TestBed.runInInjectionContext(()=>new QuoteBulk());bulk.rows=[{id:'1',number:'COT-1',customer_name:'A',reference:'R1',date:'2026-10-10',currency:'HNL',total:'100.00',status:'draft',version:'3'},{id:'2',number:'COT-2',customer_name:'B',reference:'R2',date:'2026-10-10',currency:'USD',total:'20.00',status:'draft',version:'7'}]});
 it('envía IDs y versiones como strings y conserva clave al reintentar',async()=>{bulk.selected.set(new Set(['1','2']));api.write.mockRejectedValue(new Error());await bulk.run('status');const key=api.write.mock.calls[0][3];await bulk.run('status');expect(api.write.mock.calls[1][3]).toBe(key);expect(api.write.mock.calls[0][0]).toBe('POST');expect(api.write.mock.calls[0][1]).toBe('/quotes/bulk/status');expect(api.write.mock.calls[0][2]).toEqual({items:[{id:'1',version:'3'},{id:'2',version:'7'}],status:'costing'})});
 it('expone batch y fallos parciales sin calcular importes en cliente',async()=>{bulk.selected.set(new Set(['1','2']));api.write.mockResolvedValue({batch_id:'44',action:'clone',status:'completed',results:[{position:0,id:'1',version:'3',status:'success'},{position:1,id:'2',version:'7',status:'failed',error_code:'conflict'}]});const changed=vi.fn();bulk.changed.subscribe(changed);await bulk.run('clone');expect(bulk.notice()).toBe('Lote 44: 1 correctas, 1 con error.');expect(bulk.failures()[0].error_code).toBe('conflict');expect(changed).toHaveBeenCalledTimes(1);expect(bulk.count()).toBe(0);expect(bulk.rows[0].total).toBe('100.00')});
 it('selección total usa únicamente filas visibles',()=>{bulk.toggleAll({target:{checked:true}} as unknown as Event);expect([...bulk.selected()]).toEqual(['1','2']);bulk.rows=[bulk.rows[0]];bulk.ngOnChanges();expect(bulk.count()).toBe(0)});
});
