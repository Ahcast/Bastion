import { Component, OnDestroy, inject, signal, Type } from "@angular/core";
import { NgComponentOutlet } from "@angular/common";
import { Client } from "../../api/client";
import { HomeMetrics } from "../../../modules/reports/home-metrics";
import { Processes } from "../../../modules/processes/processes";
import { Quotes } from "../../../modules/quotes/quotes";
import { PurchaseOrders } from "../../../modules/purchase-orders/purchase-orders";
import { Invoices } from "../../../modules/invoices/invoices";
import { Expenses } from "../../../modules/expenses/expenses";
type Workspace = "processes" | "quotes" | "orders" | "invoices" | "expenses";
interface Metrics {
  reports: Record<
    string,
    { currency: string; count: number; values: Record<string, string> }[]
  >;
}
const workspaces: {
  id: Workspace;
  title: string;
  permission: string;
  module: string;
  path: string;
  component: Type<unknown>;
}[] = [
  {
    id: "processes",
    title: "Procesos",
    permission: "procesos.view",
    module: "processes",
    path: "/processes?per_page=1&page=1",
    component: Processes,
  },
  {
    id: "quotes",
    title: "Cotizaciones",
    permission: "cotizaciones.view",
    module: "quotes",
    path: "/quotes?per_page=1&page=1&deleted=false",
    component: Quotes,
  },
  {
    id: "orders",
    title: "Órdenes de compra",
    permission: "ordenes.view",
    module: "purchase-orders",
    path: "/purchase-orders?per_page=1&page=1&deleted=false",
    component: PurchaseOrders,
  },
  {
    id: "invoices",
    title: "Por cobrar",
    permission: "facturas.view",
    module: "invoices",
    path: "/invoices?per_page=1&page=1",
    component: Invoices,
  },
  {
    id: "expenses",
    title: "Gastos",
    permission: "gastos.view",
    module: "expenses",
    path: "/expenses?per_page=1&page=1&deleted=false",
    component: Expenses,
  },
];
@Component({
  imports: [HomeMetrics, NgComponentOutlet],
  template: `<div class="bs-workspace bs-dashboard">
    <header class="bs-page-head">
      <div>
        <span class="bs-label">{{ today }}</span>
        <h1>Inicio</h1>
        <p>Seleccione un indicador para trabajar en ese módulo.</p>
      </div>
      <div class="bs-actions">
        <button class="bs-btn bs-btn-secondary" (click)="loadCounts()">
          Actualizar indicadores
        </button>
      </div>
    </header>
    @if (error()) {
      <p class="error" role="alert">{{ error() }}</p>
    }
    <div
      class="bs-kpis"
      role="tablist"
      aria-label="Módulo de la tabla de trabajo"
    >
      @for (w of workspaces; track w.id) {
        <button
          class="bs-kpi-tile"
          role="tab"
          [id]="'tab-' + w.id"
          [attr.aria-selected]="selected() === w.id"
          [tabIndex]="selected() === w.id ? 0 : -1"
          aria-controls="dashboard-work"
          [disabled]="!allowed(w.id)"
          (click)="select(w.id)"
          (keydown)="key($event, w.id)"
        >
          <span class="bs-label">{{ w.title }}</span
          ><span class="bs-kpi-value">{{ counts()[w.id] ?? "—" }}</span
          ><span class="bs-kpi-foot">{{
            allowed(w.id)
              ? w.id === "invoices"
                ? "Facturas consultables"
                : "Registros autorizados"
              : "Sin permiso de consulta"
          }}</span>
          @for (t of amounts(w.id); track t.currency) {
            <span class="bs-kpi-foot bs-mono"
              >Período · {{ t.currency }} {{ t.value }}</span
            >
          }
        </button>
      }
    </div>
    @if (api.has("reportes.view") && api.moduleEnabled("reports")) {
      <bastion-home-metrics
        [active]="selected()"
        (changed)="metrics.set($event)"
      />
    }
    @if (component(); as view) {
      <section
        id="dashboard-work"
        class="bs-dashboard-work"
        role="tabpanel"
        [attr.aria-labelledby]="'tab-' + selected()"
      >
        <ng-container [ngComponentOutlet]="view" />
      </section>
    } @else {
      <div class="bs-state">
        <h2>Sin módulos de trabajo disponibles</h2>
        <p>El menú muestra los accesos habilitados para su cuenta.</p>
      </div>
    }
  </div>`,
})
export class Dashboard implements OnDestroy {
  api = inject(Client);
  workspaces = workspaces;
  selected = signal<Workspace>("processes");
  counts = signal<Partial<Record<Workspace, number>>>({});
  metrics = signal<Metrics | null>(null);
  error = signal("");
  private sequence = 0;
  today = new Intl.DateTimeFormat("es-HN", {
    dateStyle: "full",
    timeZone: "America/Tegucigalpa",
  }).format(new Date());
  constructor() {
    const first = workspaces.find((w) => this.allowed(w.id));
    if (first) this.selected.set(first.id);
    void this.loadCounts();
  }
  allowed(id: Workspace) {
    const w = workspaces.find((x) => x.id === id)!;
    return this.api.has(w.permission) && this.api.moduleEnabled(w.module);
  }
  select(id: Workspace) {
    if (this.allowed(id)) this.selected.set(id);
  }
  component() {
    return this.allowed(this.selected())
      ? workspaces.find((w) => w.id === this.selected())!.component
      : null;
  }
  amounts(id: Workspace) {
    const field = id === "invoices" ? "balance" : "total";
    return (this.metrics()?.reports[id] || [])
      .filter((t) => t.values[field] !== undefined)
      .map((t) => ({ currency: t.currency, value: t.values[field] }));
  }
  async loadCounts() {
    const ticket = ++this.sequence;
    this.error.set("");
    const counts: Partial<Record<Workspace, number>> = {};
    await Promise.all(
      workspaces
        .filter((w) => this.allowed(w.id))
        .map(async (w) => {
          try {
            const r = await this.api.get<{ total: number }>(w.path);
            counts[w.id] = r.total;
          } catch (e) {
            if (ticket === this.sequence) this.error.set(this.api.message(e));
          }
        }),
    );
    if (ticket === this.sequence) this.counts.set(counts);
  }
  ngOnDestroy() {
    ++this.sequence;
  }
  key(event: KeyboardEvent, id: Workspace) {
    const enabled = workspaces.filter((w) => this.allowed(w.id));
    const index = enabled.findIndex((w) => w.id === id);
    let next = index;
    if (event.key === "ArrowRight") next = (index + 1) % enabled.length;
    else if (event.key === "ArrowLeft")
      next = (index + enabled.length - 1) % enabled.length;
    else if (event.key === "Home") next = 0;
    else if (event.key === "End") next = enabled.length - 1;
    else return;
    event.preventDefault();
    const target = enabled[next];
    if (target) {
      this.select(target.id);
      document.getElementById("tab-" + target.id)?.focus();
    }
  }
}
