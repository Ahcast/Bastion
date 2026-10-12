import { Component, inject, signal, OnDestroy } from "@angular/core";
import { FormsModule } from "@angular/forms";
import { Client } from "../../core/api/client";
import { labels, periodQuery, Total } from "./reports";
interface Metrics {
  filters: Record<string, string | number>;
  reports: Record<string, Total[]>;
}
@Component({
  selector: "bastion-home-metrics",
  imports: [FormsModule],
  template: `<section class="card">
    <h2>Resumen comercial</h2>
    <p>El mismo período y permisos producen los mismos totales en Reportes.</p>
    <form (ngSubmit)="load()">
      <label>Desde <input name="from" type="date" [(ngModel)]="from" /></label
      ><label>Hasta <input name="to" type="date" [(ngModel)]="to" /></label
      ><label
        >Moneda
        <select aria-label="Moneda" name="currency" [(ngModel)]="currency">
          <option value="">Todas</option>
          <option>HNL</option>
          <option>USD</option>
        </select></label
      ><button [disabled]="busy()">Actualizar resumen</button>
    </form>
    @if (error()) {
      <p role="alert">{{ error() }}</p>
    }
    @if (metrics(); as m) {
      <p>
        Período: {{ m.filters["date_from"] || "Sin límite inicial" }} —
        {{ m.filters["date_to"] || "Sin límite final" }}
      </p>
      @for (k of keys(m.reports); track k) {
        <h3>{{ label(k) }}</h3>
        @for (t of m.reports[k]; track t.currency) {
          <p>{{ t.currency }} · {{ t.count }} registros</p>
          <dl>
            @for (f of keys(t.values); track f) {
              <dt>{{ label(f) }}</dt>
              <dd>{{ t.values[f] }} {{ t.currency }}</dd>
            }
          </dl>
        } @empty {
          <p>Sin registros en el período.</p>
        }
      }
    }
  </section>`,
})
export class HomeMetrics implements OnDestroy {
  api = inject(Client);
  from = "";
  to = "";
  currency = "";
  metrics = signal<Metrics | null>(null);
  busy = signal(false);
  error = signal("");
  private sequence = 0;
  constructor() {
    void this.load();
  }
  label(k: string) {
    return labels[k] || k;
  }
  keys(o: object) {
    return Object.keys(o);
  }
  async load() {
    const ticket = ++this.sequence;
    this.metrics.set(null);
    this.error.set("");
    if (!this.api.has("reportes.view") || !this.api.moduleEnabled("reports")) {
      this.busy.set(false);
      return;
    }
    this.busy.set(true);
    try {
      const m = await this.api.get<Metrics>(
        "/reports/home?" + periodQuery(this.from, this.to, this.currency),
      );
      if (ticket === this.sequence) this.metrics.set(m);
    } catch (e) {
      if (ticket === this.sequence) this.error.set(this.api.message(e));
    } finally {
      if (ticket === this.sequence) this.busy.set(false);
    }
  }
  ngOnDestroy() {
    ++this.sequence;
  }
}
