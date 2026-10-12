import { Injectable, inject, signal } from "@angular/core";
import { HttpClient, HttpErrorResponse } from "@angular/common/http";
import { firstValueFrom } from "rxjs";
export interface Grant {
  permission: string;
  scope: "all" | "self" | "team";
}
export interface User {
  id: string;
  name: string;
  email: string;
  active: boolean;
  mfa_enabled: boolean;
  version: string;
  roles: string[];
}
export interface Profile {
  user: User;
  grants: Grant[];
  enabled_modules?: string[];
}
@Injectable({ providedIn: "root" })
export class Client {
  private http = inject(HttpClient);
  apiUrl = "";
  csrf = "";
  humanActivity = true;
  startupError = signal("");
  profile = signal<Profile | null>(null);
  mfa = signal<"anonymous" | "enroll" | "challenge" | "complete">("anonymous");
  async initialize() {
    try {
      await this.initializeConnection();
    } catch (error) {
      this.startupError.set(this.message(error));
    }
  }
  private async initializeConnection() {
    const config = await firstValueFrom(
      this.http.get<{ apiUrl: string }>("/config.json"),
    );
    const url = new URL(config.apiUrl);
    if (
      url.pathname !== "/" ||
      url.search ||
      url.hash ||
      url.username ||
      url.password ||
      !(
        url.protocol === "https:" ||
        (url.protocol === "http:" &&
          ["localhost", "127.0.0.1"].includes(url.hostname))
      )
    )
      throw new Error("Configuración API inválida.");
    this.apiUrl = url.origin;
    await this.refreshCSRF();
    try {
      const status = await this.get<{
        mfa_complete: boolean;
        mfa_enabled: boolean;
      }>("/auth/status");
      this.mfa.set(
        status.mfa_complete
          ? "complete"
          : status.mfa_enabled
            ? "challenge"
            : "enroll",
      );
      if (status.mfa_complete) await this.me();
    } catch (error) {
      if (!(error instanceof HttpErrorResponse) || error.status !== 401)
        throw error;
    }
  }
  async refreshCSRF() {
    const value = await this.get<{ csrf_token: string }>("/auth/csrf");
    this.csrf = value.csrf_token;
  }
  get<T>(path: string): Promise<T> {
    return firstValueFrom(this.http.get<T>(this.apiUrl + "/api/v1" + path));
  }
  async write<T>(
    method: "POST" | "PUT" | "PATCH" | "DELETE",
    path: string,
    body: unknown = {},
    idempotencyKey?: string,
  ): Promise<T> {
    if (!this.csrf) await this.refreshCSRF();
    return firstValueFrom(
      this.http.request<T>(method, this.apiUrl + "/api/v1" + path, {
        body,
        headers: idempotencyKey ? { "Idempotency-Key": idempotencyKey } : {},
      }),
    );
  }
  async download(path: string, body: unknown): Promise<Blob> {
    if (!this.csrf) await this.refreshCSRF();
    try {
      return await firstValueFrom(
        this.http.post(this.apiUrl + "/api/v1" + path, body, {
          responseType: "blob",
        }),
      );
    } catch (e) {
      if (e instanceof HttpErrorResponse && e.error instanceof Blob) {
        try {
          const parsed = JSON.parse(await e.error.text());
          throw new HttpErrorResponse({
            error: parsed,
            status: e.status,
            statusText: e.statusText,
            url: e.url || undefined,
          });
        } catch (decoded) {
          if (decoded instanceof HttpErrorResponse) throw decoded;
        }
      }
      throw e;
    }
  }
  async login(email: string, password: string) {
    const result = await this.write<{ csrf_token: string }>(
      "POST",
      "/auth/login",
      { email, password },
    );
    this.csrf = result.csrf_token;
    const s = await this.get<{ mfa_enabled: boolean }>("/auth/status");
    this.mfa.set(s.mfa_enabled ? "challenge" : "enroll");
  }
  async factor(code: string, enroll: boolean): Promise<string[]> {
    const r = await this.write<{
      csrf_token: string;
      recovery_codes: string[] | null;
    }>("POST", "/auth/mfa/" + (enroll ? "confirm" : "challenge"), { code });
    this.csrf = r.csrf_token;
    this.mfa.set("complete");
    await this.me();
    return r.recovery_codes || [];
  }
  async me() {
    const p = await this.get<Profile>("/auth/me");
    this.profile.set(p);
  }
  async logout() {
    await this.write("POST", "/auth/logout");
    this.profile.set(null);
    this.csrf = "";
    this.mfa.set("anonymous");
    await this.refreshCSRF();
  }
  has(permission: string) {
    return (
      this.profile()?.grants.some((g) => g.permission === permission) || false
    );
  }
  moduleEnabled(id: string) {
    return this.profile()?.enabled_modules?.includes(id) || false;
  }
  message(error: unknown): string {
    if (error instanceof HttpErrorResponse) {
      if (error.status === 401) {
        this.profile.set(null);
        this.mfa.set("anonymous");
        return "La sesión venció. Inicie sesión de nuevo.";
      }
      return error.error?.message || "No se pudo conectar con el servidor.";
    }
    return error instanceof Error
      ? error.message
      : "No se pudo completar la operación.";
  }
}
