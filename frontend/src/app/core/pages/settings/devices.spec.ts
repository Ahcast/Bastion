import { TestBed } from "@angular/core/testing";
import { signal } from "@angular/core";
import { Router } from "@angular/router";
import { it, expect, vi } from "vitest";
import { Client } from "../../api/client";
import { Devices, TrustedDevice } from "./devices";
function setup() {
  const device: TrustedDevice = {
    id: "a".repeat(32),
    name: "Chrome en Windows",
    current: false,
    created_at: "2026-10-15T12:00:00Z",
    last_seen_at: "2026-10-15T12:00:00Z",
    expires_at: "2026-11-14T12:00:00Z",
  };
  const api = {
      get: vi.fn().mockResolvedValue({ data: [device], total: 1 }),
      write: vi.fn().mockResolvedValue({ current: false }),
      profile: signal({}),
      mfa: signal("complete"),
      csrf: "synthetic",
      message: (e: Error) => e.message,
    },
    router = { navigateByUrl: vi.fn().mockResolvedValue(true) };
  TestBed.configureTestingModule({
    providers: [
      { provide: Client, useValue: api },
      { provide: Router, useValue: router },
    ],
  });
  return {
    api,
    router,
    device,
    c: TestBed.runInInjectionContext(() => new Devices()),
  };
}
it("consulta equipos paginados y conserva lista si revocación es rechazada", async () => {
  const { api, c, device } = setup();
  await c.load();
  expect(api.get).toHaveBeenCalledWith("/auth/devices?page=1&per_page=25");
  api.write.mockRejectedValue(new Error("Confirme su contraseña."));
  await c.revoke(device);
  expect(c.devices()).toEqual([device]);
  expect(c.message()).toBe("Confirme su contraseña.");
  expect(c.busy()).toBe(false);
});
it("revoca equipo ajeno al actual y recarga el registro", async () => {
  const { api, c, device } = setup();
  await c.load();
  api.get.mockResolvedValue({ data: [], total: 0 });
  await c.revoke(device);
  expect(api.write).toHaveBeenCalledWith(
    "DELETE",
    "/auth/devices/" + device.id,
  );
  expect(c.devices()).toEqual([]);
  expect(api.mfa()).toBe("complete");
  expect(c.message()).toContain("revocado");
});
it("revocar navegador actual limpia sesión local y navega a login", async () => {
  const { api, router, c, device } = setup();
  api.write.mockResolvedValue({ current: true });
  await c.revoke({ ...device, current: true });
  expect(api.profile()).toBeNull();
  expect(api.mfa()).toBe("anonymous");
  expect(api.csrf).toBe("");
  expect(router.navigateByUrl).toHaveBeenCalledWith("/login");
});
