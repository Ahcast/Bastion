import {TestBed} from '@angular/core/testing';
import {describe,it,expect,beforeEach,vi} from 'vitest';
import {Client} from '../../core/api/client';
import {InvoiceFiscalSettings,FiscalValue} from './fiscal-settings';
const value=():FiscalValue=>({cai:'CAI-SINTETICO-NO-VALIDO',prefix:'TEST-',number_width:8,range_start:'1',range_end:'100',valid_from:'2026-10-01',valid_until:'2026-10-31',currency:'HNL',enabled:false});
describe('Configuración fiscal de facturas',()=>{
 let c:InvoiceFiscalSettings;let api:{has:ReturnType<typeof vi.fn>;get:ReturnType<typeof vi.fn>;write:ReturnType<typeof vi.fn>;message:ReturnType<typeof vi.fn>};
 beforeEach(()=>{api={has:vi.fn(()=>true),get:vi.fn().mockResolvedValue({value:value(),version:'1',next_value:'9007199254740993'}),write:vi.fn().mockResolvedValue({version:'2',next_value:'9007199254740993'}),message:vi.fn(()=> 'Error fiscal')};TestBed.configureTestingModule({providers:[{provide:Client,useValue:api}]});c=TestBed.runInInjectionContext(()=>new InvoiceFiscalSettings())});
 it('no inventa autorización ni contador y respeta revocación',async()=>{
 expect(c.form.cai).toBe('');expect(c.form.number_width).toBeNull();expect(c.form.range_start).toBe('');expect(c.next()).toBe('');
 api.has.mockReturnValue(false);await c.load();c.loaded.set(true);c.reason='Cambio';await c.save();expect(api.get).not.toHaveBeenCalled();expect(api.write).not.toHaveBeenCalled();
 });
 it('muestra el estado consultado sin redondear el rango ni tratar ediciones como confirmadas',async()=>{
 const saved={...value(),enabled:true,range_end:'9007199254740992'};
 api.get.mockResolvedValue({value:saved,version:'1',next_value:'9007199254740993'});await c.load();expect(c.state()).toBe('Rango agotado');c.form.range_end='9007199254740994';expect(c.state()).toBe('Rango agotado');
 api.get.mockResolvedValue({value:{...saved,enabled:false},version:'2',next_value:'9007199254740993'});await c.load();expect(c.state()).toBe('Autorización deshabilitada');
 });
 it('conserva strings exactos y no permite enviar el contador',async()=>{
 await c.load();c.reason='Rango sintético';await c.save();const [method,path,body,key]=api.write.mock.calls[0];expect(method).toBe('PUT');expect(path).toBe('/invoices/fiscal-settings');expect(body).toEqual({version:'1',value:value(),reason:'Rango sintético'});expect(key).toBeTruthy();expect(c.next()).toBe('9007199254740993');expect(c.version).toBe('2');expect(c.needsReload()).toBe(true);
 await c.save();expect(api.write).toHaveBeenCalledTimes(1);await c.load();expect(c.needsReload()).toBe(false);
 });
 it('reintenta respuesta incierta con la misma intención',async()=>{
 await c.load();c.reason='Revisión';api.write.mockRejectedValueOnce(new Error('ACK incierto'));await c.save();expect(c.reason).toBe('Revisión');expect(c.version).toBe('1');expect(c.needsReload()).toBe(false);await c.save();expect(api.write.mock.calls[0][3]).toBe(api.write.mock.calls[1][3]);expect(c.needsReload()).toBe(true);
 });
 it('un 404 después de confirmar no convierte una edición en alta',async()=>{
 api.get.mockRejectedValueOnce({status:404});await c.load();expect(c.version).toBe('0');expect(c.loaded()).toBe(true);c.form=value();c.reason='Alta sintética';await c.save();api.get.mockRejectedValueOnce({status:404});await c.load();expect(c.version).toBe('2');expect(c.form).toEqual(value());expect(c.needsReload()).toBe(true);expect(c.error()).toBe('Error fiscal');c.reason='Otro cambio';await c.save();expect(api.write).toHaveBeenCalledTimes(1);
 });
});
