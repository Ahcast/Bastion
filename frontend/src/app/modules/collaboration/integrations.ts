import { Component, OnDestroy, OnInit, inject, signal } from "@angular/core";
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

      @if (pending()) {
        <p role="status">La respuesta no está confirmada. Reintente la misma operación para recuperar el resultado.</p>
        <button type="button" (click)="retry()" [disabled]="busy()">Reintentar operación</button>
      }
      <section aria-labelledby="approval-title">
        <h2 id="approval-title">Aprobaciones pendientes ({{ approvalsTotal() }})</h2>
        <button type="button" (click)="loadApprovals()" [disabled]="busy()">Actualizar aprobaciones</button>
        <ul>
          @for (a of approvals(); track a.id) {
            <li>
              <strong>{{ a.action }}</strong> · {{ entityLabel(a.entity_type) }} #{{ a.entity_id }}
              · vence {{ a.expires_at }}
              @if (a.reason) { <span> · {{ a.reason }}</span> }
              @if (entityPath(a)) { <a [routerLink]="entityPath(a)" [queryParams]="{id:a.entity_id}">Abrir en módulo propietario</a> }
            </li>
          } @empty { <li>No hay aprobaciones pendientes visibles.</li> }
        </ul>
        <p>Las decisiones se realizan únicamente en el módulo propietario.</p>
        <button type="button" (click)="approvalPage=approvalPage-1;loadApprovals()" [disabled]="approvalBusy() || approvalPage<=1">Anterior aprobación</button>
        <button type="button" (click)="approvalPage=approvalPage+1;loadApprovals()" [disabled]="approvalBusy() || approvalPage*100>=approvalsTotal()">Más aprobaciones</button>
      </section>

      <section aria-labelledby="links-title">
        <h2 id="links-title">Vínculos de un archivo</h2>
        <label>ID de biblioteca<input [(ngModel)]="entryID" [disabled]="blocked()" /></label>
        <button type="button" (click)="loadLinks()" [disabled]="busy() || !!pending()">Cargar vínculos</button>
        @if (entry()) { <p>Archivo: {{ entry()?.title }} · versión {{ entry()?.version }}</p> }
        <ul>
          @for (link of links(); track link.resource_type + ':' + link.resource_id) {
            <li>
              {{ entityLabel(link.resource_type) }} · {{ link.current_label || link.label }}
              · versión {{ link.current_version }}
              @if (link.stale) { <strong> · vínculo desactualizado</strong> }
              @if (api.has('colaboracion.link')) { <button type="button" (click)="remove(link)" [disabled]="blocked()">Quitar</button> }
            </li>
          } @empty { <li>No hay vínculos cargados.</li> }
        </ul>
        @if (api.has('colaboracion.link') && entry()) {
          <fieldset [disabled]="blocked()">
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
            <button type="button" (click)="add()" [disabled]="blocked()">Agregar y guardar</button>
          </fieldset>
          <bastion-reauthentication />
        }
      </section>
    </section>
  `,
})
export class CollaborationIntegrations implements OnInit, OnDestroy {
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

  busy = signal(false);
  approvalBusy = signal(false);
  pending = signal<{path:string;body:unknown;key:string}|null>(null);
  confirmed = signal(false);
  approvalPage = 1;
  private approvalSequence = 0;
  private linkSequence = 0;
  private disposed = false;
  blocked() { return this.busy() || !!this.pending() || this.confirmed(); }
  ngOnDestroy() { this.disposed = true; this.approvalSequence++; this.linkSequence++; }
  async ngOnInit() { await this.loadApprovals(); }
  async loadApprovals() {
    const sequence = ++this.approvalSequence;
    this.approvalBusy.set(true);
    try {
      const page = await this.api.get<ApprovalPage>(`/collaboration/approvals?page=${this.approvalPage}&per_page=100`);
      if (this.disposed || sequence !== this.approvalSequence) return;
      this.approvals.set(page.data); this.approvalsTotal.set(page.total);
    } catch (e) {
      if (!this.disposed && sequence === this.approvalSequence) {
        this.approvals.set([]); this.approvalsTotal.set(0); this.error.set(this.api.message(e));
      }
    } finally { if (!this.disposed && sequence === this.approvalSequence) this.approvalBusy.set(false); }
  }
  async loadLinks() {
    const id = this.entryID;
    const sequence = ++this.linkSequence;
    this.entry.set(null); this.links.set([]); this.error.set("");
    if (!/^[1-9]\d*$/.test(id)) { this.error.set("Indique un ID de biblioteca válido."); return; }
    this.busy.set(true);
    try {
      const [entry, links] = await Promise.all([
        this.api.get<Entry>(`/collaboration/${id}`),
        this.api.get<{data: Link[]}>(`/collaboration/${id}/links`),
      ]);
      if (this.disposed || sequence !== this.linkSequence || this.entryID !== id) return;
      this.entry.set(entry); this.links.set(links.data); this.confirmed.set(false);
    } catch (e) {
      if (!this.disposed && sequence === this.linkSequence) this.error.set(this.api.message(e));
    } finally { if (!this.disposed && sequence === this.linkSequence) this.busy.set(false); }
  }
  async add() {
    if (this.blocked()) return;
    if (!this.entry() || !/^[1-9]\d*$/.test(this.newID) || !/^[1-9]\d*$/.test(this.newVersion)) {
      this.error.set("Complete ID y versión del recurso."); return;
    }
    const next = this.links().filter((x) => !(x.resource_type === this.newType && x.resource_id === this.newID));
    await this.save([...next, { resource_type: this.newType, resource_id: this.newID, resource_version: this.newVersion, current_version: this.newVersion, label: "", current_label: "", stale: false }]);
  }
  async remove(link: Link) {
    if (!this.blocked()) await this.save(this.links().filter((x) => x !== link));
  }
  private async save(links: Link[]) {
    const current = this.entry(); if (!current) return;
    await this.perform({path:`/collaboration/${current.id}/links`,body:{
      version: current.version,
      links: links.map(x => ({resource_type:x.resource_type,resource_id:x.resource_id,resource_version:x.resource_version})),
    },key:crypto.randomUUID()});
  }
  async retry() {
    const pending = this.pending();
    if (pending && !this.busy()) await this.perform(pending);
  }
  private async perform(intent:{path:string;body:unknown;key:string}) {
    this.pending.set(intent); this.busy.set(true); this.error.set(""); this.message.set("");
    try {
      await this.api.write("PUT", intent.path, intent.body, intent.key);
      if (this.disposed) return;
      this.pending.set(null); this.confirmed.set(true);
      this.message.set("Vínculos actualizados."); this.newID = ""; this.newVersion = "1";
      await this.loadLinks();
    } catch (e) {
      if (!this.disposed) {
        this.error.set(this.api.message(e));
        const status = (e as {status?:number})?.status;
        if (status && status >= 400 && status < 500) this.pending.set(null);
      }
    } finally { if (!this.disposed) this.busy.set(false); }
  }
  entityLabel(v: string) {
    return ({ processes: "Proceso", quotes: "Cotización", purchase_orders: "Orden de compra", invoices: "Factura", collaboration: "Colaboración" } as Record<string,string>)[v] || v;
  }
  entityPath(a: Approval): string | null {
    return ({ quotes: "/quotes", purchase_orders: "/purchase-orders", invoices: "/invoices", processes: "/processes" } as Record<string,string>)[a.entity_type] || null;
  }
}
