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
 it('preserva IDs seleccionados visibles durante una recarga y poda los que desaparecen',()=>{bulk.toggleAll({target:{checked:true}} as unknown as Event);expect([...bulk.selected()]).toEqual(['1','2']);bulk.rows=[{...bulk.rows[0],version:'4',status:'costing'}];bulk.ngOnChanges();expect([...bulk.selected()]).toEqual(['1']);expect(bulk.rows[0].version).toBe('4')});
 it('usa la versión recargada para la siguiente acción después de conservar selección',async()=>{bulk.selected.set(new Set(['1']));bulk.rows=[{...bulk.rows[0],version:'9',status:'costing'},bulk.rows[1]];bulk.ngOnChanges();api.write.mockResolvedValue({batch_id:'45',action:'status',status:'completed',results:[{position:0,id:'1',version:'9',status:'failed',error_code:'conflict'}]});await bulk.run('status');expect(api.write.mock.calls[0][2]).toEqual({items:[{id:'1',version:'9'}],status:'costing'});expect(bulk.notice()).toBe('Lote 45: 0 correctas, 1 con error.')});
});

describe('Recuperación de lotes',()=>{
 it('conserva cuerpo original después de recargar filas y cambiar destino',async()=>{
  const api={write:vi.fn().mockRejectedValue(new Error()),message:()=> 'Falló',has:()=>true};TestBed.configureTestingModule({providers:[{provide:Client,useValue:api}]});
  const bulk=TestBed.runInInjectionContext(()=>new QuoteBulk());bulk.rows=[{id:'9007199254740993',number:'COT-1',customer_name:'A',reference:'',date:'2026-10-10',currency:'HNL',total:'1.01',status:'draft',version:'12'}];bulk.selected.set(new Set([bulk.rows[0].id]));await bulk.run('clone');
  const call=api.write.mock.calls[0];bulk.rows=[];bulk.ngOnChanges();bulk.target='review';await bulk.run('status');expect(api.write).toHaveBeenCalledTimes(1);await bulk.retry();expect(api.write.mock.calls[1]).toEqual(call);
 });
 it('no pierde la intención ni anuncia éxito mientras el lote siga en proceso',async()=>{
  const api={write:vi.fn().mockResolvedValue({batch_id:'3',action:'status',status:'processing',results:[]}),message:()=>'',has:()=>true};TestBed.configureTestingModule({providers:[{provide:Client,useValue:api}]});
  const bulk=TestBed.runInInjectionContext(()=>new QuoteBulk());bulk.pending.set({action:'status',body:{items:[{id:'1',version:'1'}],status:'costing'},key:'same-key'});await bulk.retry();expect(bulk.pending()?.key).toBe('same-key');expect(bulk.notice()).toContain('en proceso');
 });
 it('renderiza controles y bloquea la operación mientras esté abierta una ficha',async()=>{
  const api={write:vi.fn(),message:()=>'',has:()=>true};TestBed.configureTestingModule({providers:[{provide:Client,useValue:api}]});const fixture=TestBed.createComponent(QuoteBulk);fixture.componentRef.setInput('rows',[{id:'1',number:'COT-1',version:'2',customer_name:'A'}]);fixture.componentRef.setInput('disabled',true);fixture.detectChanges();
  expect(fixture.nativeElement.textContent).toContain('Acciones en lote');expect(fixture.nativeElement.querySelector('input').disabled).toBe(true);fixture.componentInstance.selected.set(new Set(['1']));await fixture.componentInstance.run('clone');expect(api.write).not.toHaveBeenCalled();
 });
});
