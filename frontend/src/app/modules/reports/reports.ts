import { Component, inject, signal, OnDestroy } from "@angular/core";
import { FormsModule } from "@angular/forms";
import { Client } from "../../core/api/client";
export type Kind = "invoices" | "orders" | "expenses";
export interface Total {
  currency: string;
  count: number;
  values: Record<string, string>;
}
export interface ReportPage {
  report: Kind;
  filters: Record<string, string | number>;
  data: Record<string, string>[];
  total: number;
  page: number;
  per_page: number;
  totals: Total[];
}
export const labels: Record<string, string> = {
  invoices: "Facturas",
  orders: "Órdenes",
  expenses: "Gastos",
  id: "ID",
  invoice_number: "Número de factura",
  customer: "Cliente",
  invoice_date: "Fecha factura",
  due_date: "Vencimiento",
  currency: "Moneda",
  status: "Estado",
  total: "Total",
  paid_total: "Pagos",
  balance: "Saldo",
  order_number: "Número de orden",
  destination: "Destino",
  order_date: "Fecha orden",
  subtotal: "Subtotal",
  tax_amount: "ISV",
  products_investment: "Inversión de productos",
  additional_expenses: "Gastos adicionales",
  investment_total: "Inversión total",
  profit: "Utilidad",
  supplier: "Proveedor",
  expense_date: "Fecha gasto",
  category: "Categoría",
  taxable_amount: "Gravado",
  exempt_amount: "Exento",
  draft: "Borrador",
  issued: "Emitida",
  received: "Recibida",
  confirmed: "Confirmada",
  purchasing: "En compra",
  in_transit: "En tránsito",
  goods_received: "Mercadería recibida",
  preparing_delivery: "Preparando entrega",
  delivered: "Entregada",
  invoiced: "Facturada",
  closed: "Cerrada",
  freight: "Flete",
  services: "Servicios",
  supplies: "Suministros",
  travel: "Viajes",
  utilities: "Servicios públicos",
  other: "Otros",
};
export const fields: Record<Kind, string[]> = {
  invoices: [
    "id",
    "invoice_number",
    "customer",
    "invoice_date",
    "due_date",
    "currency",
    "status",
    "total",
    "paid_total",
    "balance",
  ],
  orders: [
    "id",
    "order_number",
    "customer",
    "destination",
    "order_date",
    "currency",
    "status",
    "subtotal",
    "tax_amount",
    "total",
    "products_investment",
    "additional_expenses",
    "investment_total",
    "profit",
  ],
  expenses: [
    "id",
    "invoice_number",
    "supplier",
    "expense_date",
    "category",
    "currency",
    "taxable_amount",
    "exempt_amount",
    "tax_amount",
    "total",
  ],
};
export function periodQuery(
  from: string,
  to: string,
  currency: string,
): URLSearchParams {
  const q = new URLSearchParams();
  for (const [key, value] of [
    ["date_from", from],
    ["date_to", to],
    ["currency", currency],
  ])
    if (value) q.set(key, value);
  return q;
}
@Component({
  selector: "bastion-reports",
  imports: [FormsModule],
  template: ` <h1>Reportes</h1>
    <p>
      Totales del período autorizado, separados por moneda. Las referencias de
      gastos no modifican la utilidad de las órdenes.
    </p>
    <form (ngSubmit)="load(1)">
      <label
        >Reporte
        <select name="kind" [(ngModel)]="kind" (ngModelChange)="changeKind()">
          @for (k of available(); track k) {
            <option [value]="k">{{ label(k) }}</option>
          }
        </select></label
      >
      <label>Desde <input name="from" type="date" [(ngModel)]="from" /></label
      ><label>Hasta <input name="to" type="date" [(ngModel)]="to" /></label>
      <label
        >Moneda
        <select name="currency" [(ngModel)]="currency">
          <option value="">Todas</option>
          <option>HNL</option>
          <option>USD</option>
        </select></label
      >
      @if (kind !== "expenses") {
        <label
          >Estado
          <select name="status" [(ngModel)]="status">
            <option value="">Todos</option>
            @for (s of statuses(); track s) {
              <option [value]="s">{{ label(s) }}</option>
            }
          </select></label
        >
      }
      <label
        >Filas por página
        <select name="limit" [(ngModel)]="limit">
          <option [ngValue]="25">25</option>
          <option [ngValue]="50">50</option>
          <option [ngValue]="100">100</option>
        </select></label
      >
      <button [disabled]="busy() || !available().length">Consultar</button>
    </form>
    @if (busy()) {
      <p role="status">Consultando reporte…</p>
    }
    @if (error()) {
      <p role="alert">{{ error() }}</p>
    }
    @if (result(); as r) {
      <p>
        Período consultado:
        {{ r.filters["date_from"] || "Sin límite inicial" }} —
        {{ r.filters["date_to"] || "Sin límite final" }}.
        {{ r.total }} registros.
      </p>
      @for (t of r.totals; track t.currency) {
        <section class="card">
          <h2>Totales {{ t.currency }}</h2>
          <p>{{ t.count }} registros del conjunto completo</p>
          <dl>
            @for (f of moneyFields(r.report); track f) {
              <dt>{{ label(f) }}</dt>
              <dd>{{ t.values[f] }} {{ t.currency }}</dd>
            }
          </dl>
        </section>
      }
      <div style="overflow-x:auto">
        <table>
          <caption>
            {{
              label(r.report)
            }}
            — página
            {{
              r.page
            }}
          </caption>
          <thead>
            <tr>
              @for (f of columns(r.report); track f) {
                <th scope="col">{{ label(f) }}</th>
              }
            </tr>
          </thead>
          <tbody>
            @for (row of r.data; track row["id"]) {
              <tr>
                @for (f of columns(r.report); track f) {
                  <td>{{ display(f, row[f]) }}</td>
                }
              </tr>
            } @empty {
              <tr>
                <td [attr.colspan]="columns(r.report).length">
                  No hay registros para estos filtros.
                </td>
              </tr>
            }
          </tbody>
        </table>
      </div>
      <button
        [disabled]="busy() || r.page <= 1"
        (click)="load(r.page - 1, true)"
      >
        Anterior</button
      ><span> Página {{ r.page }} </span
      ><button
        [disabled]="busy() || r.page * r.per_page >= r.total"
        (click)="load(r.page + 1, true)"
      >
        Siguiente
      </button>
    }`,
})
export class Reports implements OnDestroy {
  api = inject(Client);
  labels = labels;
  kind: Kind = "invoices";
  from = "";
  to = "";
  currency = "";
  status = "";
  limit = 25;
  result = signal<ReportPage | null>(null);
  error = signal("");
  busy = signal(false);
  private sequence = 0;
  constructor() {
    this.kind = this.available()[0] || "invoices";
    void this.load(1);
  }
  available(): Kind[] {
    return (["invoices", "orders", "expenses"] as Kind[]).filter(
      (k) =>
        this.api.moduleEnabled(k === "orders" ? "purchase-orders" : k) &&
        this.api.has(
          {
            invoices: "facturas.view",
            orders: "ordenes.view",
            expenses: "gastos.view",
          }[k],
        ),
    );
  }
  label(key: string) {
    return labels[key] || key;
  }
  columns(k: Kind) {
    return fields[k];
  }
  moneyFields(k: Kind) {
    return fields[k].filter((f) =>
      [
        "subtotal",
        "tax_amount",
        "total",
        "paid_total",
        "balance",
        "products_investment",
        "additional_expenses",
        "investment_total",
        "profit",
        "taxable_amount",
        "exempt_amount",
      ].includes(f),
    );
  }
  display(field: string, value: string) {
    return field === "status" || field === "category"
      ? this.label(value)
      : value || "—";
  }
  statuses() {
    return this.kind === "invoices"
      ? ["draft", "issued"]
      : this.kind === "orders"
        ? [
            "received",
            "confirmed",
            "purchasing",
            "in_transit",
            "goods_received",
            "preparing_delivery",
            "delivered",
            "invoiced",
            "closed",
          ]
        : [];
  }
  changeKind() {
    this.status = "";
    this.result.set(null);
    void this.load(1);
  }
  async load(page = 1, paginate = false) {
    const ticket = ++this.sequence;
    const previous = this.result();
    this.result.set(null);
    this.error.set("");
    if (
      !this.api.has("reportes.view") ||
      !this.api.moduleEnabled("reports") ||
      !this.available().includes(this.kind)
    ) {
      this.busy.set(false);
      this.error.set("No tiene acceso a este reporte.");
      return;
    }
    const q =
      paginate && previous
        ? periodQuery(
            String(previous.filters["date_from"] || ""),
            String(previous.filters["date_to"] || ""),
            String(previous.filters["currency"] || ""),
          )
        : periodQuery(this.from, this.to, this.currency);
    const status =
      paginate && previous
        ? String(previous.filters["status"] || "")
        : this.status;
    if (status) q.set("status", status);
    q.set("page", String(page));
    q.set(
      "per_page",
      String(paginate && previous ? previous.per_page : this.limit),
    );
    this.busy.set(true);
    try {
      const result = await this.api.get<ReportPage>(
        "/reports/" + this.kind + "?" + q,
      );
      if (ticket === this.sequence) this.result.set(result);
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
