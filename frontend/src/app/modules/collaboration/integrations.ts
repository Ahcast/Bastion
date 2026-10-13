import { Component, OnInit, inject, signal } from "@angular/core";
import { FormsModule } from "@angular/forms";
import { RouterLink } from "@angular/router";
import { Client } from "../../core/api/client";
import { Reauthentication } from "../../core/security/reauthentication";

interface Approval {
  id: string; action: string; entity_type: string; entity_id: string;
  entity_version: string; requested_by: string; status: string; reason: string;
  expires_at: string; version: string;
}
interface ApprovalPage { data: Approval[]; total: number; page: number; per_page: number }
interface Link {
  resource_type: string; resource_id: string; resource_version: string;
  current_version: string; label: string; current_label: string; stale: boolean;
}
interface Entry { id: string; version: string; title: string }

@Component({
  imports: [FormsModule, RouterLink, Reauthentication],
  template: `
    <section aria-labelledby="integration-title">
      <h1 id="integration-title">Integraciones y aprobaciones</h1>
      <p>Vista de solo lectura de Workflow y vínculos documentales de Colaboración.</p>
      <nav aria-label="Colaboración">
        <a routerLink="/collaboration">Biblioteca</a> ·
        <a routerLink="/collaboration/work">Mi trabajo</a>
      </nav>
      @if (error()) { <p role="alert">{{ error() }}</p> }
      @if (message()) { <p role="status">{{ message() }}</p> }

      <section aria-labelledby="approval-title">
        <h2 id="approval-title">Aprobaciones pendientes ({{ approvalsTotal() }})</h2>
        <button type="button" (click)="loadApprovals()">Actualizar aprobaciones</button>
        <ul>
          @for (a of approvals(); track a.id) {
            <li>
              <strong>{{ a.action }}</strong> · {{ entityLabel(a.entity_type) }} #{{ a.entity_id }}
              · vence {{ a.expires_at }}
              @if (a.reason) { <span> · {{ a.reason }}</span> }
              @if (entityPath(a)) { <a [routerLink]="entityPath(a)">Abrir en módulo propietario</a> }
            </li>
          } @empty { <li>No hay aprobaciones pendientes visibles.</li> }
        </ul>
        <p>Las decisiones se realizan únicamente en el módulo propietario.</p>
      </section>

      <section aria-labelledby="links-title">
        <h2 id="links-title">Vínculos de un archivo</h2>
        <label>ID de biblioteca<input [(ngModel)]="entryID" /></label>
        <button type="button" (click)="loadLinks()">Cargar vínculos</button>
        @if (entry()) { <p>Archivo: {{ entry()?.title }} · versión {{ entry()?.version }}</p> }
        <ul>
          @for (link of links(); track link.resource_type + ':' + link.resource_id) {
            <li>
              {{ entityLabel(link.resource_type) }} · {{ link.current_label || link.label }}
              · versión {{ link.current_version }}
              @if (link.stale) { <strong> · vínculo desactualizado</strong> }
              @if (api.has('colaboracion.link')) { <button type="button" (click)="remove(link)">Quitar</button> }
            </li>
          } @empty { <li>No hay vínculos cargados.</li> }
        </ul>
        @if (api.has('colaboracion.link') && entry()) {
          <fieldset>
            <legend>Agregar vínculo</legend>
            <label>Tipo
              <select [(ngModel)]="newType">
                <option value="processes">Proceso</option>
                <option value="quotes">Cotización</option>
                <option value="purchase_orders">Orden de compra</option>
                <option value="invoices">Factura</option>
              </select>
            </label>
            <label>ID<input [(ngModel)]="newID" /></label>
            <label>Versión actual<input [(ngModel)]="newVersion" inputmode="numeric" /></label>
            <button type="button" (click)="add()">Agregar y guardar</button>
          </fieldset>
          <app-reauthentication />
        }
      </section>
    </section>
  `,
})
export class CollaborationIntegrations implements OnInit {
  api = inject(Client);
  approvals = signal<Approval[]>([]);
  approvalsTotal = signal(0);
  links = signal<Link[]>([]);
  entry = signal<Entry | null>(null);
  error = signal("");
  message = signal("");
  entryID = "";
  newType = "processes";
  newID = "";
  newVersion = "1";

  async ngOnInit() { await this.loadApprovals(); }
  async loadApprovals() {
    this.error.set("");
    try {
      const page = await this.api.get<ApprovalPage>("/collaboration/approvals?page=1&per_page=100");
      this.approvals.set(page.data); this.approvalsTotal.set(page.total);
    } catch (e) { this.error.set(this.api.message(e)); }
  }
  async loadLinks() {
    this.error.set(""); this.message.set("");
    if (!/^\d+$/.test(this.entryID)) { this.error.set("Indique un ID de biblioteca válido."); return; }
    try {
      const [entry, links] = await Promise.all([
        this.api.get<Entry>(`/collaboration/${this.entryID}`),
        this.api.get<{data: Link[]}>(`/collaboration/${this.entryID}/links`),
      ]);
      this.entry.set(entry); this.links.set(links.data);
    } catch (e) { this.entry.set(null); this.links.set([]); this.error.set(this.api.message(e)); }
  }
  async add() {
    if (!this.entry() || !/^\d+$/.test(this.newID) || !/^\d+$/.test(this.newVersion)) {
      this.error.set("Complete ID y versión del recurso."); return;
    }
    const next = this.links().filter((x) => !(x.resource_type === this.newType && x.resource_id === this.newID));
    await this.save([...next, { resource_type: this.newType, resource_id: this.newID, resource_version: this.newVersion, current_version: this.newVersion, label: "", current_label: "", stale: false }]);
  }
  async remove(link: Link) {
    await this.save(this.links().filter((x) => x !== link));
  }
  private async save(links: Link[]) {
    const current = this.entry(); if (!current) return;
    this.error.set(""); this.message.set("");
    try {
      await this.api.write("PUT", `/collaboration/${current.id}/links`, {
        version: current.version,
        links: links.map((x) => ({ resource_type: x.resource_type, resource_id: x.resource_id, resource_version: x.resource_version })),
      }, crypto.randomUUID());
      this.message.set("Vínculos actualizados."); this.newID = ""; this.newVersion = "1";
      await this.loadLinks();
    } catch (e) { this.error.set(this.api.message(e)); }
  }
  entityLabel(v: string) {
    return ({ processes: "Proceso", quotes: "Cotización", purchase_orders: "Orden de compra", invoices: "Factura", collaboration: "Colaboración" } as Record<string,string>)[v] || v;
  }
  entityPath(a: Approval): string | null {
    return ({ quotes: "/quotes", purchase_orders: "/purchase-orders", invoices: "/invoices", processes: "/processes" } as Record<string,string>)[a.entity_type] || null;
  }
}
