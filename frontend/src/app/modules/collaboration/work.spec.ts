import { provideRouter } from "@angular/router";
import { TestBed } from "@angular/core/testing";
import { describe, expect, it, vi } from "vitest";
import { Client } from "../../core/api/client";
import { CollaborationWork } from "./work";

function api() {
  return {
    profile: () => ({ user: { id: "11" } }),
    has: vi.fn(() => true),
    get: vi.fn(async (path: string) => {
      if (path.startsWith("/collaboration/task-assignees?")) {
        return { data: [{ id: "11", name: "Operador" }, { id: "12", name: "Revisor" }] };
      }
      return {
        data: [
          {
            id: "31", creator_id: "11", assignee_id: "11", assignee_name: "Operador",
            title: "Revisar expediente", status: "open", version: "2",
          },
        ],
        total: 1, page: 1, per_page: 100,
      };
    }),
    write: vi.fn(async (_method: string, _path: string, _body?: unknown, _key?: string) => ({})),
    message: vi.fn(() => "Error sintético"),
  };
}

function fixture(client: ReturnType<typeof api>) {
  TestBed.configureTestingModule({ providers: [provideRouter([]), { provide: Client, useValue: client }] });
  return TestBed.runInInjectionContext(() => new CollaborationWork());
}

describe("Colaboración trabajo", () => {
  it("asocia el selector de asignado a una etiqueta estable", async () => {
    const client = api();
    TestBed.configureTestingModule({ providers: [provideRouter([]), { provide: Client, useValue: client }] });
    const fixture = TestBed.createComponent(CollaborationWork);
    await fixture.componentInstance.ngOnInit();
    fixture.detectChanges();
    await fixture.whenStable();
    fixture.detectChanges();
    const select = fixture.nativeElement.querySelector('select[aria-label="Asignar a"]') as HTMLSelectElement;
    expect(select).toBeTruthy();
    expect(Array.from(select.options).map(o => o.textContent?.trim())).toContain("Operador");
    select.value = "11";
    select.dispatchEvent(new Event("change", { bubbles: true }));
    await fixture.whenStable();
    expect(fixture.componentInstance.assigneeID).toBe("11");
  });
  it("consulta Mi trabajo con filtros canónicos", async () => {
    const client = api();
    const component = fixture(client);
    await component.load();
    const url = new URL("https://local.test" + client.get.mock.calls[0][0]);
    expect(url.pathname).toBe("/collaboration/tasks");
    expect(url.searchParams.get("view")).toBe("mine");
    expect(url.searchParams.get("page")).toBe("1");
    expect(component.total()).toBe(1);
  });

  it("crea tarea sin inventar recurso y usa idempotencia", async () => {
    const client = api();
    const component = fixture(client);
    component.title = "Preparar entrega";
    component.note = "Nota exacta";
    component.assigneeID = "12";
    component.dueDate = "2026-10-13";
    await component.create();
    const call = client.write.mock.calls[0];
    expect(call[0]).toBe("POST");
    expect(call[1]).toBe("/collaboration/tasks");
    expect(call[2]).toEqual({
      assignee_id: "12", resource_type: "", resource_id: "",
      title: "Preparar entrega", note: "Nota exacta", due_date: "2026-10-13",
    });
    expect(call[3]).toBeTruthy();
  });

  it("cambia estado y reabre con la versión observada", async () => {
    const client = api();
    const component = fixture(client);
    const task = {
      id: "31", creator_id: "11", assignee_id: "11", assignee_name: "Operador",
      title: "Revisar", status: "open" as const, version: "7",
    };
    await component.setStatus(task, "done");
    expect(client.write.mock.calls[0][1]).toBe("/collaboration/tasks/31/status");
    expect(client.write.mock.calls[0][2]).toEqual({ version: "7", status: "done" });
    await component.reopen({ ...task, status: "done" });
    expect(client.write.mock.calls.find((c) => c[1] === "/collaboration/tasks/31/reopen")?.[2]).toEqual({ version: "7" });
  });
  it("reintenta una respuesta incierta con el mismo cuerpo y clave", async () => {
    const client = api(); const component = fixture(client);
    client.write.mockRejectedValueOnce(new Error("Conexión perdida"));
    component.title = "Entrega"; component.assigneeID = "11";
    await component.create();
    const first = client.write.mock.calls[0];
    expect(component.pending()).toBeTruthy();
    component.title = "Otra entrega";
    await component.create();
    expect(client.write).toHaveBeenCalledTimes(1);
    await component.retry();
    expect(client.write.mock.calls[1]).toEqual(first);
    expect(component.pending()).toBeNull();
  });

  it("no repite una escritura confirmada cuando falla la recarga", async () => {
    const client = api(); const component = fixture(client);
    client.get.mockRejectedValue(new Error("Recarga fallida"));
    component.title = "Entrega"; component.assigneeID = "11";
    await component.create();
    expect(component.confirmed()).toBe(true);
    expect(component.message()).toBe("Operación guardada.");
    component.title = "Entrega siguiente";
    await component.create();
    expect(client.write).toHaveBeenCalledTimes(1);
  });

  it("edita solo datos mutables conservando versión y recurso", async () => {
    const client = api(); const component = fixture(client);
    await component.edit({id:"31",creator_id:"11",assignee_id:"11",assignee_name:"Operador",title:"Revisar",status:"open",version:"7",resource_type:"processes",resource_id:"4"});
    component.title = "Revisar entrega"; component.dueDate = "2026-10-14";
    await component.create();
    expect(client.write.mock.calls[0][0]).toBe("PATCH");
    expect(client.write.mock.calls[0][1]).toBe("/collaboration/tasks/31");
    expect(client.write.mock.calls[0][2]).toEqual({version:"7",assignee_id:"11",title:"Revisar entrega",note:"",due_date:"2026-10-14"});
  });

  it("descarta respuestas tardías y limpia tareas tras denegación", async () => {
    const client = api(); const component = fixture(client);
    let resolve!: (value:any) => void;
    client.get.mockImplementationOnce(() => new Promise(r => resolve=r));
    const older = component.load();
    await component.load();
    resolve({data:[],total:0}); await older;
    expect(component.total()).toBe(1);
    client.get.mockRejectedValueOnce(new Error("Acceso revocado"));
    await component.load();
    expect(component.tasks()).toEqual([]);
    expect(component.total()).toBe(0);
  });

});
