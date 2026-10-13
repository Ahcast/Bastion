import { TestBed } from "@angular/core/testing";
import { describe, expect, it, vi } from "vitest";
import { Client } from "../../core/api/client";
import { CollaborationIntegrations } from "./integrations";

function api() {
  return {
    has: vi.fn(() => true),
    get: vi.fn(async (path: string) => {
      if (path.startsWith("/collaboration/approvals?")) {
        return { data: [{ id: "5", action: "quote.issue", entity_type: "quotes", entity_id: "9", entity_version: "2", requested_by: "7", status: "pending", reason: "", expires_at: "2026-10-13T12:00:00Z", version: "1" }], total: 1, page: 1, per_page: 100 };
      }
      if (path === "/collaboration/44") return { id: "44", version: "3", title: "Expediente" };
      if (path === "/collaboration/44/links") return { data: [{ resource_type: "processes", resource_id: "1", resource_version: "1", current_version: "2", label: "HC-001", current_label: "HC-001", stale: true }] };
      throw new Error(path);
    }),
    write: vi.fn(async (_method: string, _path: string, _body?: unknown, _key?: string) => ({})),
    message: vi.fn(() => "Error sintético"),
  };
}

function fixture(client: ReturnType<typeof api>) {
  TestBed.configureTestingModule({ providers: [{ provide: Client, useValue: client }] });
  return TestBed.runInInjectionContext(() => new CollaborationIntegrations());
}

describe("Colaboración integraciones", () => {
  it("consulta aprobaciones como proyección de solo lectura", async () => {
    const client = api();
    const component = fixture(client);
    await component.loadApprovals();
    expect(component.approvalsTotal()).toBe(1);
    expect(client.write).not.toHaveBeenCalled();
    expect(component.entityPath(component.approvals()[0])).toBe("/quotes");
  });

  it("carga vínculos y conserva evidencia de versión/staleness", async () => {
    const client = api();
    const component = fixture(client);
    component.entryID = "44";
    await component.loadLinks();
    expect(component.entry()?.version).toBe("3");
    expect(component.links()[0]).toMatchObject({ resource_type: "processes", resource_version: "1", current_version: "2", stale: true });
  });

  it("reemplaza vínculos con versión optimista de la entrada", async () => {
    const client = api();
    const component = fixture(client);
    component.entryID = "44";
    await component.loadLinks();
    component.newType = "invoices";
    component.newID = "8";
    component.newVersion = "4";
    await component.add();
    const call = client.write.mock.calls[0];
    expect(call[0]).toBe("PUT");
    expect(call[1]).toBe("/collaboration/44/links");
    expect(call[2]).toEqual({
      version: "3",
      links: [
        { resource_type: "processes", resource_id: "1", resource_version: "1" },
        { resource_type: "invoices", resource_id: "8", resource_version: "4" },
      ],
    });
    expect(call[3]).toBeTruthy();
  });
  it("recupera un vínculo incierto usando la intención original", async () => {
    const client=api(); const component=fixture(client);
    component.entryID="44"; await component.loadLinks();
    component.newID="8"; component.newVersion="4";
    client.write.mockRejectedValueOnce(new Error("Respuesta perdida"));
    await component.add(); const first=client.write.mock.calls[0];
    await component.add(); expect(client.write).toHaveBeenCalledTimes(1);
    await component.retry(); expect(client.write.mock.calls[1]).toEqual(first);
  });
  it("preserva confirmación cuando falla recarga y limpia datos revocados", async () => {
    const client=api(); const component=fixture(client);
    component.entryID="44"; await component.loadLinks();
    component.newID="8"; component.newVersion="4";
    client.get.mockRejectedValue(new Error("Sin acceso"));
    await component.add();
    expect(component.confirmed()).toBe(true);
    expect(component.entry()).toBeNull(); expect(component.links()).toEqual([]);
    expect(component.message()).toBe("Vínculos actualizados.");
    await component.loadApprovals(); expect(component.approvals()).toEqual([]);
  });

});
