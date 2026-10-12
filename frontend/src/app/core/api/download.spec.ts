import { TestBed } from "@angular/core/testing";
import {
  provideHttpClient,
  withInterceptors,
  HttpErrorResponse,
} from "@angular/common/http";
import {
  provideHttpClientTesting,
  HttpTestingController,
} from "@angular/common/http/testing";
import { provideRouter } from "@angular/router";
import { it, expect } from "vitest";
import { Client } from "./client";
import { apiInterceptor } from "./interceptor";
function fixture() {
  TestBed.configureTestingModule({
    providers: [
      provideRouter([]),
      provideHttpClient(withInterceptors([apiInterceptor])),
      provideHttpClientTesting(),
    ],
  });
  const api = TestBed.inject(Client);
  api.apiUrl = "http://localhost:8080";
  api.csrf = "synthetic-csrf";
  return { api, http: TestBed.inject(HttpTestingController) };
}
it("descarga POST incluye sesión, CSRF y respuesta binaria", async () => {
  const { api, http } = fixture();
  const promise = api.download("/reports/orders/export", { currency: "HNL" });
  const req = http.expectOne(
    "http://localhost:8080/api/v1/reports/orders/export",
  );
  expect(req.request.method).toBe("POST");
  expect(req.request.withCredentials).toBe(true);
  expect(req.request.headers.get("X-CSRF-Token")).toBe("synthetic-csrf");
  expect(req.request.responseType).toBe("blob");
  const blob = new Blob(["PK"]);
  req.flush(blob);
  expect(await promise).toBe(blob);
  http.verify();
});
it("errores JSON en Blob conservan código y mensaje Core", async () => {
  const { api, http } = fixture();
  const blob = new Blob(
    [
      JSON.stringify({
        code: "reauth_required",
        message: "Confirme su identidad.",
      }),
    ],
    { type: "application/json" },
  );
  Object.defineProperty(blob, "text", {
    value: async () =>
      JSON.stringify({
        code: "reauth_required",
        message: "Confirme su identidad.",
      }),
  });
  const promise = api.download("/reports/orders/export", {}).catch((e) => e);
  http
    .expectOne("http://localhost:8080/api/v1/reports/orders/export")
    .flush(blob, { status: 403, statusText: "Forbidden" });
  const e = await promise;
  expect(e).toBeInstanceOf(HttpErrorResponse);
  expect(e.error.code).toBe("reauth_required");
  expect(api.message(e)).toBe("Confirme su identidad.");
  http.verify();
});
