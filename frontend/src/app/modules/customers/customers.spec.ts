import { TestBed } from "@angular/core/testing";
import { vi } from "vitest";
import { Client } from "../../core/api/client";
import { Customers, Customer } from "./customers";

describe("Clientes",()=>{
 const record=():Customer=>({id:"9007199254740993",name:"Cliente sintético",rtn:"",contact_name:"",phone:"",email:"",address:"",institution_type:"public",is_active:true,team_id:"",version:"7",contacts:[],addresses:[]});
 let api:{get:ReturnType<typeof vi.fn>;write:ReturnType<typeof vi.fn>;has:ReturnType<typeof vi.fn>;message:ReturnType<typeof vi.fn>};
 beforeEach(()=>{api={get:vi.fn(async(path:string)=>path.includes("?")||path.endsWith("/history")||path.endsWith("/teams")?{data:[],total:0,page:1,per_page:25}:({...record(),created_by:"42",updated_at:"2026-10-09T00:00:00Z"})),write:vi.fn(async()=>({id:record().id,version:"8"})),has:vi.fn(()=>true),message:vi.fn(()=>"El registro cambió. Vuelva a abrirlo.")};TestBed.configureTestingModule({providers:[{provide:Client,useValue:api}]})});
 it("envía solo los campos editables y conserva IDs/versiones como cadenas",async()=>{
  const c=TestBed.createComponent(Customers).componentInstance;c.editor.set({...record(),created_by:"42",updated_at:"fecha"} as Customer);await c.save();
  const [method,path,body,key]=api.write.mock.calls[0] as unknown as [string,string,Record<string,unknown>,string];expect(method).toBe("PATCH");expect(path).toBe("/customers/9007199254740993");expect(body["version"]).toBe("7");expect(body).not.toHaveProperty("id");expect(body).not.toHaveProperty("created_by");expect(body).not.toHaveProperty("updated_at");expect(key.length).toBeGreaterThan(8);
 });
 it("mantiene la ficha y la clave después de un fallo; una intención distinta obtiene otra clave",async()=>{
  api.write.mockRejectedValue(new Error("conflict"));const c=TestBed.createComponent(Customers).componentInstance;const draft=record();draft.name="Edición pendiente";c.editor.set(draft);await c.save();await c.save();expect(api.write).toHaveBeenCalledTimes(2);expect(api.write.mock.calls[0][3]).toBe(api.write.mock.calls[1][3]);expect(c.editor()?.name).toBe("Edición pendiente");expect(c.error()).toContain("registro cambió");draft.name="Intención modificada";await c.save();expect(api.write.mock.calls[2][3]).not.toBe(api.write.mock.calls[1][3]);
 });
 it("oculta la creación y bloquea la ficha para un lector",async()=>{
  api.has.mockReturnValue(false);const f=TestBed.createComponent(Customers);f.componentInstance.editor.set(record());f.detectChanges();await f.whenStable();const html=f.nativeElement as HTMLElement;expect(html.textContent).not.toContain("Nuevo cliente");expect(html.querySelector("fieldset")?.disabled).toBe(true);expect(html.textContent).not.toContain("Registrar nota");
 });
 it("solicita paginación y filtros al backend y usa el total recibido",async()=>{
  api.get.mockResolvedValue({data:[record()],total:80,page:3,per_page:25});const c=TestBed.createComponent(Customers).componentInstance;c.page=3;c.search="Institución";c.active="false";await c.load();const path=api.get.mock.calls[0][0] as string;expect(path).toContain("page=3");expect(path).toContain("is_active=false");expect(c.total()).toBe(80);expect(c.rows()).toHaveLength(1);
 });
});
