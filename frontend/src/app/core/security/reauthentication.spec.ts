import {TestBed} from '@angular/core/testing';
import {Client} from '../api/client';
import {Reauthentication} from './reauthentication';
import {vi,describe,it,expect} from 'vitest';
describe('Confirmación de identidad',()=>{
 it('borra contraseña y segundo factor tras éxito o error y bloquea doble envío',async()=>{
 const api={write:vi.fn().mockRejectedValue(new Error()),message:vi.fn(()=> 'No confirmado')};
 TestBed.configureTestingModule({providers:[{provide:Client,useValue:api}]});
 const c=TestBed.runInInjectionContext(()=>new Reauthentication());
 c.password='synthetic';c.code='123456';await c.confirm();expect(c.password).toBe('');expect(c.code).toBe('');expect(c.message()).toBe('No confirmado');
 api.write.mockResolvedValue({});c.password='synthetic';c.code='123456';await c.confirm();expect(api.write.mock.calls[1][1]).toBe('/auth/reauth');expect(c.password).toBe('');expect(c.message()).toContain('Identidad confirmada');
 c.busy.set(true);await c.confirm();expect(api.write).toHaveBeenCalledTimes(2);
 });
});
