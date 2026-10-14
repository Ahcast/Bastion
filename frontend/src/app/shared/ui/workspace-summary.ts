import { Component, Input } from "@angular/core";
const states: Record<string, string> = {
  draft: "Borrador",
  costing: "Costeo",
  review: "Revisión",
  approved: "Aprobada",
  sent: "Enviada",
  accepted: "Aceptada",
  rejected: "Rechazada",
  expired: "Vencida",
  converted: "Convertida",
  received: "Recibida",
  confirmed: "Confirmada",
  purchasing: "En compra",
  in_transit: "En tránsito",
  goods_received: "Mercadería recibida",
  preparing_delivery: "Preparando entrega",
  delivered: "Entregada",
  invoiced: "Facturada",
  closed: "Cerrada",
  issued: "Emitida",
  cancelled: "Anulada",
  pending: "Pendiente",
  partially_paid: "Pago parcial",
  paid: "Pagada",
  overdue: "Vencida",
  active: "Activo",
  inactive: "Inactivo",
  current: "Vigente",
  unknown: "Por verificar",
  trash: "Papelera",
};
@Component({
  selector: "bs-workspace-summary",
  template: ` <div
      class="bs-kpis bs-kpis-4"
      aria-label="Resumen de la consulta"
    >
      <div class="bs-kpi-tile bs-static bs-kpi-leading">
        <span class="bs-label">Registros encontrados</span
        ><span class="bs-kpi-value">{{ total }}</span
        ><span class="bs-kpi-foot">Consulta autorizada</span>
      </div>
      <div class="bs-kpi-tile bs-static">
        <span class="bs-label">En esta página</span
        ><span class="bs-kpi-value">{{ rows.length }}</span
        ><span class="bs-kpi-foot">Registros visibles</span>
      </div>
      <div class="bs-kpi-tile bs-static">
        <span class="bs-label">{{
          financial
            ? "Documentos HNL"
            : processes
              ? "Marcados de interés"
              : "Activos en esta página"
        }}</span
        ><span class="bs-kpi-value">{{
          financial
            ? countCurrency("HNL")
            : countFlag(processes ? "is_interesting" : "is_active")
        }}</span
        ><span class="bs-kpi-foot">En esta página</span>
      </div>
      <div class="bs-kpi-tile bs-static">
        <span class="bs-label">{{
          financial
            ? "Documentos USD"
            : processes
              ? "Vigentes en esta página"
              : "Con correo registrado"
        }}</span
        ><span class="bs-kpi-value">{{
          financial
            ? countCurrency("USD")
            : processes
              ? countFlag("is_current")
              : emailCount()
        }}</span
        ><span class="bs-kpi-foot">En esta página</span>
      </div>
    </div>
    <div class="bs-two bs-analysis">
      <section class="bs-frame">
        <div class="bs-frame-head">
          <h2 class="bs-frame-title">
            {{ dates().length ? "Registros por fecha" : "Consulta actual" }}
          </h2>
          <span class="bs-label">Esta página</span>
        </div>
        @if (dates().length) {
          <div class="bs-bars" aria-label="Cantidad de registros por fecha">
            @for (d of dates(); track d.label) {
              <div class="bs-bar-col">
                <span class="bs-mono">{{ d.count }}</span>
                <div
                  class="bs-count-bar"
                  [style.height.px]="16 + (96 * d.count) / maxDate()"
                ></div>
                <span class="bs-label">{{ d.label.slice(5) }}</span>
              </div>
            }
          </div>
        } @else {
          <p>Registros que puede consultar con los filtros actuales.</p>
        }
        <p class="bs-muted">
          Las cifras de esta página no representan todo el historial.
        </p>
      </section>
      <section class="bs-frame">
        <div class="bs-frame-head">
          <h2 class="bs-frame-title">Distribución por estado</h2>
          <span class="bs-label">Esta página</span>
        </div>
        <div class="bs-hbars">
          @for (g of groups(); track g.label) {
            <div class="bs-count-group">
              <span>{{ g.label }}</span>
              <div class="bs-count-track">
                <span
                  [style.width.%]="
                    rows.length ? (100 * g.count) / rows.length : 0
                  "
                ></span>
              </div>
              <span class="bs-mono">{{ g.count }}</span>
            </div>
          } @empty {
            <p>Sin registros con los filtros actuales.</p>
          }
        </div>
      </section>
    </div>`,
})
export class WorkspaceSummary {
  @Input() rows: object[] = [];
  @Input() total = 0;
  @Input() financial = false;
  @Input() processes = false;
  private value(row: object) {
    return row as Record<string, unknown>;
  }
  countFlag(key: string) {
    return this.rows.filter((r) => this.value(r)[key] === true).length;
  }
  countCurrency(currency: string) {
    return this.rows.filter((r) => this.value(r)["currency"] === currency)
      .length;
  }
  emailCount() {
    return this.rows.filter((r) => !!this.value(r)["email"]).length;
  }
  groups() {
    const counts = new Map<string, number>();
    for (const row of this.rows) {
      const r = this.value(row);
      let status = String(r["commercial_status"] || r["status"] || "unknown");
      if (r["deleted_at"]) status = "trash";
      else if (typeof r["is_active"] === "boolean")
        status = r["is_active"] ? "active" : "inactive";
      else if ("is_current" in r)
        status =
          r["is_current"] === true
            ? "current"
            : r["is_current"] === false
              ? "expired"
              : "unknown";
      const label = states[status] || "Registrado";
      counts.set(label, (counts.get(label) || 0) + 1);
    }
    return [...counts].map(([label, count]) => ({ label, count }));
  }
  dates() {
    const counts = new Map<string, number>();
    for (const row of this.rows) {
      const r = this.value(row);
      const value = String(
        r["date"] ||
          r["order_date"] ||
          r["invoice_date"] ||
          r["expense_date"] ||
          r["deadline"] ||
          "",
      ).slice(0, 10);
      if (/^\d{4}-\d{2}-\d{2}$/.test(value))
        counts.set(value, (counts.get(value) || 0) + 1);
    }
    return [...counts]
      .sort(([a], [b]) => a.localeCompare(b))
      .slice(-10)
      .map(([label, count]) => ({ label, count }));
  }
  maxDate() {
    return Math.max(1, ...this.dates().map((d) => d.count));
  }
}
