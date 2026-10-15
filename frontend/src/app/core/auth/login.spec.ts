import { TestBed } from "@angular/core/testing";
import { signal } from "@angular/core";
import { Router } from "@angular/router";
import { describe, it, expect, vi } from "vitest";
import { Login } from "./login";
import { Client } from "../api/client";

describe("Vinculación OTP", () => {
  function setup(
    state: "anonymous" | "enroll" | "challenge" | "complete" = "enroll",
  ) {
    const api = {
      mfa: signal(state),
      write: vi
        .fn()
        .mockResolvedValue({
          otpauth_uri:
            "otpauth://totp/Bastion:synthetic?secret=JBSWY3DPEHPK3PXP",
          qr_data_url: "data:image/png;base64,synthetic",
        }),
      login: vi.fn(),
      factor: vi.fn().mockResolvedValue([]),
      logout: vi.fn(),
      message: (e: Error) => e.message,
    };
    const router = { navigateByUrl: vi.fn() };
    TestBed.configureTestingModule({
      imports: [Login],
      providers: [
        { provide: Client, useValue: api },
        { provide: Router, useValue: router },
      ],
    });
    return { api, router, fixture: TestBed.createComponent(Login) };
  }
  it("muestra QR automáticamente y conserva URI manual", async () => {
    const { api, fixture } = setup();
    fixture.detectChanges();
    await new Promise((resolve) => setTimeout(resolve, 0));
    fixture.detectChanges();
    expect(api.write).toHaveBeenCalledExactlyOnceWith(
      "POST",
      "/auth/mfa/enroll",
    );
    const img = fixture.nativeElement.querySelector(
      "img.mfa-qr",
    ) as HTMLImageElement;
    expect(img.alt).toContain("Código QR");
    expect(img.src).toBe("data:image/png;base64,synthetic");
    expect(fixture.nativeElement.querySelector("textarea").value).toContain(
      "otpauth://",
    );
  });
  it("permite reintentar si falla el QR", async () => {
    const { api, fixture } = setup();
    api.write.mockRejectedValueOnce(new Error("No se pudo generar el QR"));
    fixture.detectChanges();
    await new Promise((resolve) => setTimeout(resolve, 0));
    fixture.detectChanges();
    expect(
      fixture.nativeElement.querySelector("[role=alert]").textContent,
    ).toContain("No se pudo generar");
    await fixture.componentInstance.enroll();
    fixture.detectChanges();
    expect(fixture.nativeElement.querySelector("img")).not.toBeNull();
  });
  it("login recordado continúa y borra contraseña sin desafío", async () => {
    const { api, router, fixture } = setup("anonymous");
    api.login.mockImplementation(async () => api.mfa.set("complete"));
    const c = fixture.componentInstance;
    c.email = "synthetic@example.test";
    c.password = "synthetic";
    await c.login();
    expect(c.password).toBe("");
    expect(router.navigateByUrl).toHaveBeenCalledWith("/");
    expect(api.write).not.toHaveBeenCalled();
  });
  it("login de cuenta nueva genera QR y lo borra al confirmar o salir", async () => {
    const { api, fixture } = setup("anonymous");
    api.login.mockImplementation(async () => api.mfa.set("enroll"));
    const c = fixture.componentInstance;
    await c.login();
    expect(c.qr()).toContain("data:image/png");
    await c.factor();
    expect(c.qr()).toBe("");
    expect(c.uri()).toBe("");
    await c.enroll();
    await c.logout();
    expect(c.qr()).toBe("");
    expect(c.uri()).toBe("");
  });
});
