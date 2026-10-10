import {TestBed} from '@angular/core/testing';
import {beforeEach,describe,expect,it,vi} from 'vitest';
import {Client} from '../../core/api/client';
import {QuoteFX,FXObservation} from './quote-fx';

describe('Registro FX de Cotizaciones',()=>{
 let fx:QuoteFX;
 let api:{profile:ReturnType<typeof vi.fn>;get:ReturnType<typeof vi.fn>;write:ReturnType<typeof vi.fn>;message:ReturnType<typeof vi.fn>};
 beforeEach(()=>{
  api={profile:vi.fn(()=>({grants:[{permission:'cotizaciones.fx.view',scope:'all'},{permission:'cotizaciones.fx.manage',scope:'all'}]})),get:vi.fn(),write:vi.fn(),message:vi.fn((e:unknown)=>e instanceof Error?e.message:'Falló')};
  TestBed.configureTestingModule({providers:[{provide:Client,useValue:api}]});
  fx=TestBed.runInInjectionContext(()=>new QuoteFX());
 });
 it('solo reconoce permisos FX con scope global',()=>{
  expect(fx.canView()).toBe(true);expect(fx.canManage()).toBe(true);
  api.profile.mockReturnValue({grants:[{permission:'cotizaciones.fx.view',scope:'self'},{permission:'cotizaciones.fx.manage',scope:'team'}]});
  expect(fx.canView()).toBe(false);expect(fx.canManage()).toBe(false);
 });
 it('lista filtros sin realizar cálculos de moneda en cliente',async()=>{
  api.get.mockResolvedValue({data:[],total:0,page:1,per_page:25});fx.filterKind='tcr';fx.filterSource='bch';await fx.load();
  expect(api.get).toHaveBeenCalledWith('/quotes/fx-observations?page=1&per_page=25&rate_kind=tcr&source=bch');expect(fx.total()).toBe(0);
 });
 it('normaliza horas de Honduras, envía strings y conserva la clave al reintentar',async()=>{
  fx.rateKind='tcr';fx.rate='26.123456789012';fx.validFrom='2026-10-10T08:30';fx.validTo='2026-10-10T17:00';fx.source='bch';fx.sourceReference='synthetic:bch:tcr';fx.sourcePublishedAt='2026-10-10T07:45';fx.evidenceHash='';fx.reason='';
  api.write.mockRejectedValue(new Error('caída sintética'));await fx.record();const first=api.write.mock.calls[0];expect(first[0]).toBe('POST');expect(first[1]).toBe('/quotes/fx-observations');expect(first[2]).toEqual({rate_kind:'tcr',rate:'26.123456789012',valid_from:'2026-10-10T14:30:00.000Z',valid_to:'2026-10-10T23:00:00.000Z',source:'bch',source_reference:'synthetic:bch:tcr',source_published_at:'2026-10-10T13:45:00.000Z',evidence_hash:'',supersedes_id:'',reason:''});
  await fx.record();expect(api.write.mock.calls[1][3]).toBe(first[3]);expect(fx.rate).toBe('26.123456789012');
 });
 it('prepara una corrección sin sobrescribir la observación visible',()=>{
  const row:FXObservation={id:'9',base_currency:'USD',quote_currency:'HNL',rate_kind:'sell',rate:'26.500000000001',valid_from:'2026-10-10T06:00:00Z',valid_to:null,source:'bch',source_reference:'ref',source_published_at:null,evidence_hash:'',supersedes_id:'',superseded:false,reason:'',created_by:'1',created_by_name:'Admin',created_at:'2026-10-10T06:01:00Z'};
  fx.prepareCorrection(row);expect(fx.supersedesId).toBe('9');expect(fx.rate).toBe(row.rate);expect(row.rate).toBe('26.500000000001');expect(fx.validFrom).toBe('2026-10-10T00:00');
 });
 it('muestra explícitamente que el ledger no convierte cotizaciones',()=>{
  const fixture=TestBed.createComponent(QuoteFX);api.get.mockResolvedValue({data:[],total:0,page:1,per_page:25});fixture.detectChanges();expect(fixture.nativeElement.textContent).toContain('no convierte cotizaciones automáticamente');
 });
});
