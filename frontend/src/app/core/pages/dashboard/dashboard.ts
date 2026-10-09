import { Component, inject, signal } from "@angular/core";
import { Client } from "../../api/client";
@Component({
  template: `<h1>Inicio</h1>
    <p>Bienvenido, {{ api.profile()?.user?.name }}.</p>
    <section class="card">
      <h2>Estado de la plataforma</h2>
      <p>
        Core de identidad y administración habilitado. Los módulos comerciales
        se incorporarán siguiendo el plan de desarrollo.
      </p>
      @if (error()) {
        <p role="alert">{{ error() }}</p>
      }
      @if (integrations()) {
        <dl>
          <dt>MCP técnico</dt>
          <dd>{{ integrations()!["mcp"] ? "Habilitado" : "Deshabilitado" }}</dd>
          <dt>Archivos S3</dt>
          <dd>
            {{ integrations()!["files"] ? "Habilitado" : "Deshabilitado" }}
          </dd>
          <dt>Correo</dt>
          <dd>
            {{ integrations()!["email"] ? "Habilitado" : "Deshabilitado" }}
          </dd>
        </dl>
      }
    </section>`,
})
export class Dashboard {
  api = inject(Client);
  integrations = signal<Record<string, boolean> | null>(null);
  error = signal("");
  constructor() {
    void this.load();
  }
  async load() {
    try {
      const r = await this.api.get<{ integrations: Record<string, boolean> }>(
        "/dashboard",
      );
      this.integrations.set(r.integrations);
    } catch (e) {
      this.error.set(this.api.message(e));
    }
  }
}
