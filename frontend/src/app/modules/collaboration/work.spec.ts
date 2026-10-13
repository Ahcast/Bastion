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
  TestBed.configureTestingModule({ providers: [{ provide: Client, useValue: client }] });
  return TestBed.runInInjectionContext(() => new CollaborationWork());
}

describe("Colaboración trabajo", () => {
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
});
