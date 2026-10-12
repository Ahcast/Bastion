import { Component, inject, signal } from "@angular/core";
import { Client } from "../../api/client";
import { HomeMetrics } from "../../../modules/reports/home-metrics";
@Component({
  imports: [HomeMetrics],
  template: `<h1>Inicio</h1>
    <p>Bienvenido, {{ api.profile()?.user?.name }}.</p>
    @if (api.has("reportes.view") && api.moduleEnabled("reports")) {
      <bastion-home-metrics />
    }
    <section class="card">
      <h2>Estado de la plataforma</h2>
      <p>
        Identidad y administración habilitadas. El menú muestra los módulos
        disponibles según sus permisos.
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
