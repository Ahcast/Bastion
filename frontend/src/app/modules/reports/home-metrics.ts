import {
  Component,
  inject,
  signal,
  OnDestroy,
  Input,
  Output,
  EventEmitter,
} from "@angular/core";
import { RouterLink } from "@angular/router";
import { FormsModule } from "@angular/forms";
import { Client } from "../../core/api/client";
import { labels, periodQuery, Total } from "./reports";
interface Metrics {
  filters: Record<string, string | number>;
  reports: Record<string, Total[]>;
}
@Component({
  selector: "bastion-home-metrics",
  imports: [FormsModule, RouterLink],
  template: `<section class="bs-frame bs-home-summary">
    <div class="bs-frame-head">
      <h2 class="bs-frame-title">Resumen comercial</h2>
      <span class="bs-label">Período autorizado</span>
    </div>
    <form class="bs-filter-form" (ngSubmit)="load()">
      <label>Desde<input name="from" type="date" [(ngModel)]="from" /></label
      ><label>Hasta<input name="to" type="date" [(ngModel)]="to" /></label
      ><label
        >Moneda<select aria-label="Moneda" name="currency" [(ngModel)]="currency">
          <option value="">Todas, separadas</option>
          <option>HNL</option>
          <option>USD</option>
        </select></label
      ><button [disabled]="busy()">Actualizar resumen</button>
    </form>
    @if (error()) {
      <p class="error" role="alert">{{ error() }}</p>
    }
    @if (metrics(); as m) {
      <div class="bs-row">
        <section class="bs-frame">
          <h3>Requiere acción</h3>
          <div class="bs-list">
            @for (t of m.reports["invoices"] || []; track t.currency) {
              @if (t.values["balance"] && t.values["balance"] !== "0.00") {
                <a class="bs-item bs-item-alert" routerLink="/invoices"
                  ><span
                    ><span class="bs-item-t"
                      >Por cobrar · {{ t.currency }}
                      {{ t.values["balance"] }}</span
                    ><span class="bs-item-s"
                      >Facturas del período consultado</span
                    ></span
                  ><span class="bs-item-go">Revisar →</span></a
                >
              }
            }
          </div>
          <p class="bs-muted">
            El saldo y los estados se confirman en la ficha de cada factura.
          </p>
        </section>
        <section class="bs-frame">
          <h3>Documentos del período</h3>
          <div class="bs-hbars">
            @for (k of keys(m.reports); track k) {
              @for (t of m.reports[k]; track t.currency) {
                <div class="bs-count-group">
                  <span>{{ label(k) }} · {{ t.currency }}</span>
                  <div class="bs-count-track">
                    <span
                      [style.width.%]="(100 * t.count) / maxCount(m)"
                    ></span>
                  </div>
                  <span class="bs-mono">{{ t.count }}</span>
                </div>
              }
            }
          </div>
        </section>
        <section class="bs-frame">
          <h3>Desglose · {{ label(active) }}</h3>
          @for (t of m.reports[active] || []; track t.currency) {
            <dl class="bs-facts">
              @for (f of headlineFields(active); track f) {
                @if (t.values[f] !== undefined) {
                  <div>
                    <dt class="bs-label">{{ label(f) }} · {{ t.currency }}</dt>
                    <dd class="bs-fv bs-mono">{{ t.values[f] }}</dd>
                  </div>
                }
              }
            </dl>
          } @empty {
            <p>Sin resumen comercial para este módulo.</p>
          }
        </section>
      </div>
      <details>
        <summary>Consultar todos los totales del período</summary>
        <p>
          Período: {{ m.filters["date_from"] || "Sin límite inicial" }} —
          {{ m.filters["date_to"] || "Sin límite final" }}
        </p>
        @for (k of keys(m.reports); track k) {
          <h3>{{ label(k) }}</h3>
          @for (t of m.reports[k]; track t.currency) {
            <p>{{ t.currency }} · {{ t.count }} registros</p>
            <dl class="bs-facts">
              @for (f of keys(t.values); track f) {
                <div>
                  <dt class="bs-label">{{ label(f) }}</dt>
                  <dd class="bs-fv bs-mono">
                    {{ t.values[f] }} {{ t.currency }}
                  </dd>
                </div>
              }
            </dl>
          } @empty {
            <p>Sin registros en el período.</p>
          }
        }
      </details>
    }
  </section>`,
})
export class HomeMetrics implements OnDestroy {
  @Input() active = "invoices";
  @Output() changed = new EventEmitter<Metrics | null>();
  maxCount(m: Metrics) {
    return Math.max(
      1,
      ...Object.values(m.reports)
        .flat()
        .map((t) => t.count),
    );
  }
  headlineFields(k: string) {
    return k === "orders"
      ? ["total", "investment_total", "profit"]
      : k === "expenses"
        ? ["taxable_amount", "tax_amount", "total"]
        : ["total", "paid_total", "balance"];
  }
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
    return (
      (
        { processes: "Procesos", quotes: "Cotizaciones" } as Record<
          string,
          string
        >
      )[k] ||
      labels[k] ||
      k
    );
  }
  keys(o: object) {
    return Object.keys(o);
  }
  async load() {
    const ticket = ++this.sequence;
    this.metrics.set(null);
    this.changed.emit(null);
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
      if (ticket === this.sequence) {
        this.metrics.set(m);
        this.changed.emit(m);
      }
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
