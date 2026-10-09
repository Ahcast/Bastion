import { TestBed } from "@angular/core/testing";
import {
  provideHttpClient,
  withInterceptors,
  HttpClient,
} from "@angular/common/http";
import {
  provideHttpClientTesting,
  HttpTestingController,
} from "@angular/common/http/testing";
import { provideRouter } from "@angular/router";
import { Client } from "./client";
import { apiInterceptor } from "./interceptor";
describe("API security boundary", () => {
  let client: Client;
  let http: HttpClient;
  let testing: HttpTestingController;
  beforeEach(() => {
    TestBed.configureTestingModule({
      providers: [
        provideRouter([]),
        provideHttpClient(withInterceptors([apiInterceptor])),
        provideHttpClientTesting(),
      ],
    });
    client = TestBed.inject(Client);
    client.apiUrl = "https://api.example.test";
    client.csrf = "test-csrf";
    http = TestBed.inject(HttpClient);
    testing = TestBed.inject(HttpTestingController);
  });
  afterEach(() => testing.verify());
  it("sends cookie credentials and explicit CSRF only to API mutations", () => {
    http.post("https://api.example.test/api/v1/roles", {}).subscribe();
    const req = testing.expectOne("https://api.example.test/api/v1/roles");
    expect(req.request.withCredentials).toBe(true);
    expect(req.request.headers.get("X-CSRF-Token")).toBe("test-csrf");
    req.flush({});
  });
  it("does not leak session headers to external storage", () => {
    http.put("https://s3.example.test/file", {}).subscribe();
    const req = testing.expectOne("https://s3.example.test/file");
    expect(req.request.withCredentials).toBe(false);
    expect(req.request.headers.has("X-CSRF-Token")).toBe(false);
    req.flush({});
  });
  it("does not refresh idle time for repeated background reads", () => {
    http.get("https://api.example.test/api/v1/dashboard").subscribe();
    const first = testing.expectOne(
      "https://api.example.test/api/v1/dashboard",
    );
    expect(first.request.headers.get("X-Bastion-Activity")).toBe("true");
    first.flush({});
    http.get("https://api.example.test/api/v1/dashboard").subscribe();
    const next = testing.expectOne("https://api.example.test/api/v1/dashboard");
    expect(next.request.headers.has("X-Bastion-Activity")).toBe(false);
    next.flush({});
  });
  it("does not replay a failed write", () => {
    let failed = false;
    http
      .post("https://api.example.test/api/v1/roles", {})
      .subscribe({ error: () => (failed = true) });
    testing
      .expectOne("https://api.example.test/api/v1/roles")
      .flush({ message: "Conflicto" }, { status: 409, statusText: "Conflict" });
    expect(failed).toBe(true);
    testing.expectNone("https://api.example.test/api/v1/roles");
  });
});
