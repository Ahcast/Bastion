import { TestBed } from "@angular/core/testing";
import { HttpErrorResponse, provideHttpClient } from "@angular/common/http";
import {
  provideHttpClientTesting,
  HttpTestingController,
} from "@angular/common/http/testing";
import { describe, it, expect } from "vitest";
import { Client } from "./client";
it("carga perfil cuando login ya completa MFA con prueba recordada", async () => {
  TestBed.configureTestingModule({
    providers: [provideHttpClient(), provideHttpClientTesting()],
  });
  const api = TestBed.inject(Client),
    http = TestBed.inject(HttpTestingController);
  api.apiUrl = "https://api.example.test";
  api.csrf = "synthetic-csrf";
  const login = api.login("synthetic@example.test", "synthetic");
  http
    .expectOne("https://api.example.test/api/v1/auth/login")
    .flush({ csrf_token: "rotated", mfa_required: false });
  await new Promise((resolve) => setTimeout(resolve, 0));
  http
    .expectOne("https://api.example.test/api/v1/auth/status")
    .flush({ mfa_complete: true, mfa_enabled: true });
  await new Promise((resolve) => setTimeout(resolve, 0));
  http
    .expectOne("https://api.example.test/api/v1/auth/me")
    .flush({ user: { id: "1", name: "Sintético" }, grants: [] });
  await login;
  expect(api.mfa()).toBe("complete");
  expect(api.profile()?.user.id).toBe("1");
  expect(api.csrf).toBe("rotated");
  http.verify();
});

it("recupera CSRF al reingresar después de vencer la sesión", async () => {
  TestBed.configureTestingModule({
    providers: [provideHttpClient(), provideHttpClientTesting()],
  });
  const api = TestBed.inject(Client),
    http = TestBed.inject(HttpTestingController);
  api.apiUrl = "https://api.example.test";
  api.csrf = "expired-csrf";
  api.mfa.set("complete");
  api.message(new HttpErrorResponse({ status: 401 }));
  const login = api.login("synthetic@example.test", "synthetic");
  http
    .expectOne("https://api.example.test/api/v1/auth/csrf")
    .flush({ csrf_token: "fresh-csrf" });
  await new Promise((resolve) => setTimeout(resolve, 0));
  http
    .expectOne("https://api.example.test/api/v1/auth/login")
    .flush({ csrf_token: "rotated", mfa_required: false });
  await new Promise((resolve) => setTimeout(resolve, 0));
  http
    .expectOne("https://api.example.test/api/v1/auth/status")
    .flush({ mfa_complete: true, mfa_enabled: true });
  await new Promise((resolve) => setTimeout(resolve, 0));
  http
    .expectOne("https://api.example.test/api/v1/auth/me")
    .flush({ user: { id: "1" }, grants: [] });
  await login;
  expect(api.mfa()).toBe("complete");
  http.verify();
});
