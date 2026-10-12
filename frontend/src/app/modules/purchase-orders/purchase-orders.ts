import {OrderBilling,BillingResult} from "./order-billing";
import { OrderDocuments } from "./order-documents";
import { Component, inject, signal, OnInit } from "@angular/core";
import { ActivatedRoute, RouterLink } from "@angular/router";
import { DatePipe } from "@angular/common";
import { Reauthentication } from "../../core/security/reauthentication";
import { FormsModule } from "@angular/forms";
import { Client } from "../../core/api/client";

export type OrderStatus =
  | "received"
  | "confirmed"
  | "purchasing"
  | "in_transit"
  | "goods_received"
  | "preparing_delivery"
  | "delivered"
  | "invoiced"
  | "closed";
export type ItemStatus =
  "pending" | "purchased" | "shipped" | "received" | "delivered";
interface HistoryEntry {
  id: string;
  item_position: number | null;
  from_status: string;
  to_status: string;
  actor_id: string;
  purchase_order_version: string;
  created_at: string;
}

export interface PurchaseOrderCustomer {
  id: string;
  name: string;
  rtn: string;
  contact: string;
  phone: string;
  email: string;
  address: string;
}

export interface PurchaseOrderItemInput {
  id?: string;
  description: string;
  unit: string;
  quantity: string;
  unit_sale_price: string;
  unit_cost: string;
  discount_type: "amount" | "percent";
  discount: string;
  tax_type: "isv15" | "isv18" | "exempt" | "exonerated";
  supplier_id: string;
  supplier_name_snapshot: string;
  tracking: string;
  comments: string;
}

export interface PurchaseOrderInput {
  order_number: string;
  customer: PurchaseOrderCustomer;
  destination: string;
  order_date: string;
  required_delivery_date: string | null;
  currency: "HNL" | "USD";
  comments: string;
  exoneration_reference: string;
  general_discount_type: "amount" | "percent";
  general_discount: string;
  additional_expenses: string;
  team_id: string;
  version: string;
  items: PurchaseOrderItemInput[];
}

export interface PurchaseOrderCalculation {
  gross: string;
  item_discount: string;
  general_discount: string;
  base_15: string;
  base_18: string;
  exempt: string;
  exonerated: string;
  isv_15: string;
  isv_18: string;
  total: string;
  products_investment: string;
  additional_expenses: string;
  investment_total: string;
  calculator_version: number;
  profit?: string;
  profit_version?: number;
}

export interface PurchaseOrder extends Omit<PurchaseOrderInput, "items"> {
  id: string;
  deleted_at?: string | null;
  status: OrderStatus;
  customer_version: string;
  source_quote_id: string | null;
 invoice_id?:string|null;
 invoice_number_snapshot?:string|null;
  source_quote_version?: string | null;
  cloned_from_id?: string | null;
  cloned_from_version?: string | null;
  created_by: string;
  calculation: PurchaseOrderCalculation;
  items: (PurchaseOrderItemInput & {
    supplier_version: string;
    status: ItemStatus;
    unit_discount: string;
    gross: string;
    item_discount: string;
    allocated_discount: string;
    base: string;
    line_investment: string;
  })[];
}

interface Summary {
  deleted_at?: string | null;
  id: string;
  order_number: string;
  customer: { id: string; name: string };
  order_date: string;
  currency: "HNL" | "USD";
  total: string;
  status: OrderStatus;
  version: string;
}

interface Page<T> {
  data: T[];
  total: number;
  page: number;
  per_page: number;
}

function hondurasDate(): string {
  return new Intl.DateTimeFormat("en-CA", {
    timeZone: "America/Tegucigalpa",
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
  }).format(new Date());
}

export function emptyPurchaseOrder(): PurchaseOrderInput {
  return {
    order_number: "",
    customer: {
      id: "",
      name: "",
      rtn: "",
      contact: "",
      phone: "",
      email: "",
      address: "",
    },
    destination: "",
    order_date: hondurasDate(),
    required_delivery_date: null,
    currency: "HNL",
    comments: "",
    exoneration_reference: "",
    general_discount_type: "amount",
    general_discount: "0",
    additional_expenses: "0",
    team_id: "",
    version: "0",
    items: [],
  };
}

@Component({
  imports: [OrderBilling,
    OrderDocuments,
    RouterLink,
    FormsModule,
    DatePipe,
    Reauthentication,
  ],
  templateUrl: "./purchase-orders.html",
  styles: [
    `
      fieldset {
        display: grid;
        gap: 0.8rem;
        border: 1px solid #d6e1e8;
        margin: 0.7rem 0;
        padding: 1rem;
        min-width: 0;
      }
      legend {
        padding: 0 0.4rem;
        font-weight: 600;
      }
      .actions {
        display: flex;
        gap: 0.6rem;
        flex-wrap: wrap;
        align-items: center;
      }
      .summary {
        display: grid;
        gap: 0.3rem;
        grid-template-columns: repeat(auto-fit, minmax(13rem, 1fr));
      }
      .muted {
        opacity: 0.75;
      }
      textarea {
        min-height: 4rem;
      }
    `,
  ],
})
export class PurchaseOrders implements OnInit {
 billingBusy=signal(false);
 billingNeedsReload=signal(false);
  api = inject(Client);
  rows = signal<Summary[]>([]);
  total = signal(0);
  loading = signal(false);
  busy = signal(false);
  error = signal("");
  message = signal("");
  selected = signal<PurchaseOrder | null>(null);
  editorOpen = signal(false);
  preview = signal<PurchaseOrderCalculation | null>(null);
  form = emptyPurchaseOrder();

  history = signal<HistoryEntry[]>([]);
  historyTotal = signal(0);
  historyPage = 1;
  statusFilter = "";
  orderStatuses: OrderStatus[] = [
    "received",
    "confirmed",
    "purchasing",
    "in_transit",
    "goods_received",
    "preparing_delivery",
    "delivered",
    "invoiced",
    "closed",
  ];
  deleted = false;
  trashReason = "";
  search = "";
  customerId = "";
  totalMin = "";
  totalMax = "";
  page = 1;
  perPage = 25;
  cloneForm: {
    order_number: string;
    order_date: string;
    required_delivery_date: string | null;
    destination: string;
    comments: string;
    additional_expenses: string;
  } | null = null;
  cloned = signal<{ id: string; version: string } | null>(null);
  beginClone() {
    const source = this.selected();
    if (
      !source ||
      source.deleted_at ||
      (this.busy()||this.billingBusy()) ||
      this.dirty() ||
      !this.api.has("ordenes.create")
    )
      return;
    this.cloned.set(null);
    this.cloneForm = {
      order_number: "",
      order_date: hondurasDate(),
      required_delivery_date: null,
      destination: source.destination,
      comments: "",
      additional_expenses: "0",
    };
  }
  async clone() {
    const source = this.selected();
    if (
      !source ||
      source.deleted_at ||
      !this.cloneForm ||
      (this.busy()||this.billingBusy()) ||
      this.dirty() ||
      !this.api.has("ordenes.create") ||
      this.cloned()
    )
      return;
    const path = "/purchase-orders/" + source.id + "/clone";
    const input = { ...this.cloneForm, version: source.version };
    const body = JSON.stringify(input);
    let intent = this.pending.get(path);
    if (!intent || intent.body !== body) {
      intent = { body, key: crypto.randomUUID() };
      this.pending.set(path, intent);
    }
    this.busy.set(true);
    this.error.set("");
    try {
      const result = await this.api.write<{ id: string; version: string }>(
        "POST",
        path,
        input,
        intent.key,
      );
      this.pending.delete(path);
      this.cloned.set(result);
      this.message.set("Orden clonada. Puede abrir la nueva orden.");
      await this.load();
    } catch (e) {
      this.error.set(this.api.message(e));
    } finally {
      this.busy.set(false);
    }
  }
  private pending = new Map<string, { body: string; key: string }>();

  private route = inject(ActivatedRoute, { optional: true });
  async ngOnInit() {
    await this.load();
    const id = this.route?.snapshot.queryParamMap.get("id");
    if (id) await this.open(id);
  }

  async load(reset = false) {
    if (reset) this.page = 1;
    this.loading.set(true);
    this.error.set("");
    try {
      const q = new URLSearchParams({
        page: String(this.page),
        per_page: String(this.perPage),
      });
      if (this.deleted) q.set("deleted", "true");
      if (this.statusFilter) q.set("status", this.statusFilter);
      if (this.search) q.set("search", this.search);
      if (this.customerId) q.set("customer_id", this.customerId);
      if (this.totalMin) q.set("total_min", this.totalMin);
      if (this.totalMax) q.set("total_max", this.totalMax);
      const result = await this.api.get<Page<Summary>>("/purchase-orders?" + q);
      this.rows.set(result.data);
      this.total.set(result.total);
    } catch (e) {
      this.rows.set([]);
      this.total.set(0);
      this.error.set(this.api.message(e));
    } finally {
      this.loading.set(false);
    }
  }

  create() {
 if(this.billingBusy())return;
    this.cloneForm = null;
    this.cloned.set(null);
    this.form = emptyPurchaseOrder();
    this.selected.set(null);
    this.preview.set(null);
    this.error.set("");
    this.message.set("");
    this.editorOpen.set(true);
  }

  close() {
 if(this.billingBusy())return;
    this.cloneForm = null;
    this.cloned.set(null);
    this.editorOpen.set(false);
    this.selected.set(null);
    this.preview.set(null);
    this.message.set("");
  }

  async open(id: string) {
 if(this.billingBusy())return;
    if ((this.busy()||this.billingBusy())) return;
    this.busy.set(true);
    this.error.set("");
    try {
      const order = await this.api.get<PurchaseOrder>("/purchase-orders/" + id);
      this.history.set([]);
      this.historyTotal.set(0);
      this.historyPage = 1;
      this.cloneForm = null;
      this.cloned.set(null);
      this.selected.set(order);
 this.billingNeedsReload.set(false);
      this.form = this.editable(order);
      this.preview.set(order.calculation);
      this.editorOpen.set(true);
    } catch (e) {
      this.error.set(this.api.message(e));
    } finally {
      this.busy.set(false);
    }
  }

  editable(order: PurchaseOrder): PurchaseOrderInput {
    return {
      order_number: order.order_number,
      customer: { ...order.customer },
      destination: order.destination,
      order_date: order.order_date,
      required_delivery_date: order.required_delivery_date,
      currency: order.currency,
      comments: order.comments,
      exoneration_reference: order.exoneration_reference,
      general_discount_type: order.general_discount_type,
      general_discount: order.general_discount,
      additional_expenses: order.calculation.additional_expenses,
      team_id: order.team_id,
      version: order.version,
      items: order.items.map((item) => ({
        ...(item.id ? { id: item.id } : {}),
        description: item.description,
        unit: item.unit,
        quantity: item.quantity,
        unit_sale_price: item.unit_sale_price,
        unit_cost: item.unit_cost,
        discount_type: item.discount_type,
        discount: item.discount,
        tax_type: item.tax_type,
        supplier_id: item.supplier_id,
        supplier_name_snapshot: item.supplier_name_snapshot,
        tracking: item.tracking,
        comments: item.comments,
      })),
    };
  }

  async trash() {
    const saved = this.selected();
    if (
      !saved ||
      (this.busy()||this.billingBusy()) ||
      this.dirty() ||
      !this.api.has("ordenes.delete") ||
      !this.trashReason.trim()
    )
      return;
    const restore = !!saved.deleted_at;
    const path = "/purchase-orders/" + saved.id + (restore ? "/restore" : "");
    const body = { version: saved.version, reason: this.trashReason };
    this.busy.set(true);
    this.error.set("");
    this.message.set("");
    try {
      const result = await this.api.write<{
        id: string;
        version: string;
        deleted_at: string | null;
      }>(restore ? "POST" : "DELETE", path, body, this.key(path, body));
      this.pending.delete(path);
      this.selected.set({
        ...saved,
        version: result.version,
        deleted_at: result.deleted_at,
      });
      this.form.version = result.version;
      this.trashReason = "";
      this.message.set(
        restore ? "Orden restaurada." : "Orden enviada a la papelera.",
      );
      await this.openAfterSave(result.id);
      await this.load();
    } catch (e) {
      this.error.set(this.api.message(e));
    } finally {
      this.busy.set(false);
    }
  }
  async billingConfirmed(r:BillingResult){const o=this.selected();if(!o||o.id!==r.id)return;this.selected.set({...o,version:r.version,status:r.status as OrderStatus,invoice_id:r.invoice_id});this.form.version=r.version;this.billingNeedsReload.set(true);this.message.set('Facturación confirmada · factura #'+r.invoice_id+' · OC versión '+r.version);this.busy.set(true);try{await this.openAfterSave(r.id);this.billingNeedsReload.set(false);await this.load()}catch(e){this.error.set(this.api.message(e))}finally{this.busy.set(false)}}
  documentLinked(result: { id: string; version: string }) {
    const order = this.selected();
    if (!order || order.id !== result.id) return;
    this.selected.set({ ...order, version: result.version });
    this.form.version = result.version;
  }

  writable() {
    const saved = this.selected();
    if (saved?.deleted_at) return false;
    return (
      this.api.has(saved ? "ordenes.update" : "ordenes.create") &&
      (!saved ||
        (saved.status === "received" &&
          saved.items.every((item) => item.status === "pending")))
    );
  }

  dirty() {
    const saved = this.selected();
    return (
      !!saved &&
      JSON.stringify(this.form) !== JSON.stringify(this.editable(saved))
    );
  }

  nextOrderStatus(): OrderStatus | null {
    const saved = this.selected();
    if (!saved || saved.deleted_at) return null;
    const index = this.orderStatuses.indexOf(saved.status);
    if (index < 0 || index >= 6) return null;
    const next = this.orderStatuses[index + 1];
    const permission =
      next === "delivered" ? "ordenes.deliver" : "ordenes.transition";
    return this.api.has(permission) ? next : null;
  }

  nextItemStatus(index: number): ItemStatus | null {
    const saved = this.selected();
    if (
      !saved ||
      saved.deleted_at ||
      !this.api.has("ordenes.items.transition") ||
      ["delivered", "invoiced", "closed"].includes(saved.status)
    )
      return null;
    const statuses: ItemStatus[] = [
      "pending",
      "purchased",
      "shipped",
      "received",
      "delivered",
    ];
    const current = statuses.indexOf(saved.items[index]?.status);
    return current >= 0 && current < 4 ? statuses[current + 1] : null;
  }

  deliveryReady() {
    return (
      this.selected()?.items.every((item) => item.status === "delivered") ??
      false
    );
  }

  async transition(to: string, position = 0) {
    const saved = this.selected();
    if (!saved || (this.busy()||this.billingBusy())) return;
    if (this.dirty()) {
      this.error.set(
        "Guarde o descarte los cambios antes de cambiar el estado.",
      );
      return;
    }
    const expected = position
      ? this.nextItemStatus(position - 1)
      : this.nextOrderStatus();
    if (
      expected !== to ||
      (position === 0 && to === "delivered" && !this.deliveryReady())
    )
      return;
    const path =
      "/purchase-orders/" +
      saved.id +
      (position ? "/items/" + position : "") +
      "/transition";
    const body = { to_status: to, version: saved.version };
    this.busy.set(true);
    this.error.set("");
    try {
      const result = await this.api.write<{
        id: string;
        version: string;
        status: OrderStatus;
        to_status: string;
      }>("POST", path, body, this.key(path, body));
      this.pending.delete(path);
      const updated = structuredClone(saved);
      updated.version = result.version;
      updated.status = result.status;
      if (position) updated.items[position - 1].status = to as ItemStatus;
      // Retain the confirmed version/state even when the following GET fails.
      this.selected.set(updated);
      this.form = this.editable(updated);
      this.message.set("Estado actualizado.");
      await this.openAfterSave(result.id);
      await this.load();
      await this.loadHistory(1);
    } catch (e) {
      this.error.set(this.api.message(e));
    } finally {
      this.busy.set(false);
    }
  }

  async loadHistory(page = this.historyPage) {
    const id = this.selected()?.id;
    if (!id) return;
    try {
      const result = await this.api.get<Page<HistoryEntry>>(
        "/purchase-orders/" +
          id +
          "/status-history?page=" +
          page +
          "&per_page=25",
      );
      if (this.selected()?.id !== id) return;
      this.history.set(result.data);
      this.historyTotal.set(result.total);
      this.historyPage = result.page;
    } catch (e) {
      this.error.set(this.api.message(e));
    }
  }

  addItem() {
    if (this.form.items.length >= 200) return;
    this.form.items.push({
      description: "",
      unit: "unidad",
      quantity: "1",
      unit_sale_price: "0",
      unit_cost: "0",
      discount_type: "amount",
      discount: "0",
      tax_type: "isv15",
      supplier_id: "",
      supplier_name_snapshot: "",
      tracking: "",
      comments: "",
    });
    this.preview.set(null);
  }

  removeItem(index: number) {
    this.form.items.splice(index, 1);
    this.preview.set(null);
  }

  private key(operation: string, body: unknown) {
    const encoded = JSON.stringify(body);
    const old = this.pending.get(operation);
    if (old?.body === encoded) return old.key;
    const key = crypto.randomUUID();
    this.pending.set(operation, { body: encoded, key });
    return key;
  }

  async calculate() {
    if ((this.busy()||this.billingBusy())) return;
    this.busy.set(true);
    this.error.set("");
    try {
      const body = {
        items: this.form.items.map((item) => ({
          description: item.description,
          unit: item.unit,
          quantity: item.quantity,
          unit_sale_price: item.unit_sale_price,
          unit_cost: item.unit_cost,
          discount_type: item.discount_type,
          discount: item.discount,
          tax_type: item.tax_type,
        })),
        general_discount_type: this.form.general_discount_type,
        general_discount: this.form.general_discount,
        exoneration_reference: this.form.exoneration_reference,
        additional_expenses: this.form.additional_expenses,
      };
      const result = await this.api.write<{
        calculation: PurchaseOrderCalculation;
      }>("POST", "/purchase-orders/calculate", body);
      this.preview.set(result.calculation);
    } catch (e) {
      this.preview.set(null);
      this.error.set(this.api.message(e));
    } finally {
      this.busy.set(false);
    }
  }

  async save() {
    if ((this.busy()||this.billingBusy()) || !this.writable()) return;
    const id = this.selected()?.id || "";
    const path = "/purchase-orders" + (id ? "/" + id : "");
    const body = structuredClone(this.form);
    this.busy.set(true);
    this.error.set("");
    this.message.set("");
    try {
      const result = await this.api.write<{ id: string; version: string }>(
        id ? "PATCH" : "POST",
        path,
        body,
        this.key(path, body),
      );
      this.pending.delete(path);
      this.form.version = result.version;
      await this.openAfterSave(result.id);
      await this.load();
      this.message.set(
        id ? "Orden de compra actualizada." : "Orden de compra registrada.",
      );
    } catch (e) {
      this.error.set(this.api.message(e));
    } finally {
      this.busy.set(false);
    }
  }

  private async openAfterSave(id: string) {
    const order = await this.api.get<PurchaseOrder>("/purchase-orders/" + id);
    this.selected.set(order);
 this.billingNeedsReload.set(false);
    this.form = this.editable(order);
    this.preview.set(order.calculation);
    this.editorOpen.set(true);
  }

  statusLabel(status: string) {
    const labels: Record<string, string> = {
      received: "Recibida",
      confirmed: "Confirmada",
      purchasing: "Comprando",
      in_transit: "En tránsito",
      goods_received: "Recibido",
      preparing_delivery: "Preparando entrega",
      delivered: "Entregado",
      invoiced: "Facturado",
      closed: "Cerrado",
    };
    return labels[status] || status;
  }

  itemStatusLabel(status: string) {
    const labels: Record<string, string> = {
      pending: "Pendiente",
      purchased: "Comprado",
      shipped: "Enviado",
      received: "Recibido",
      delivered: "Entregado",
    };
    return labels[status] || status;
  }
}
