import { Component, inject, signal, OnInit } from "@angular/core";
import { FormsModule } from "@angular/forms";
import { Client } from "../../core/api/client";

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
}

export interface PurchaseOrder extends Omit<PurchaseOrderInput, "items"> {
  id: string;
  status: "received";
  customer_version: string;
  source_quote_id: string | null;
  created_by: string;
  calculation: PurchaseOrderCalculation;
  items: (PurchaseOrderItemInput & {
    supplier_version: string;
    status: "pending";
    unit_discount: string;
    gross: string;
    item_discount: string;
    allocated_discount: string;
    base: string;
    line_investment: string;
  })[];
}

interface Summary {
  id: string;
  order_number: string;
  customer: { id: string; name: string };
  order_date: string;
  currency: "HNL" | "USD";
  total: string;
  status: "received";
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
    customer: { id: "", name: "", rtn: "", contact: "", phone: "", email: "", address: "" },
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
  imports: [FormsModule],
  templateUrl: "./purchase-orders.html",
  styles: [
    `fieldset { display:grid; gap:.8rem; border:1px solid #d6e1e8; margin:.7rem 0; padding:1rem; min-width:0 }
     legend { padding:0 .4rem; font-weight:600 }
     .actions { display:flex; gap:.6rem; flex-wrap:wrap; align-items:center }
     .summary { display:grid; gap:.3rem; grid-template-columns:repeat(auto-fit,minmax(13rem,1fr)) }
     .muted { opacity:.75 }
     textarea { min-height:4rem }`,
  ],
})
export class PurchaseOrders implements OnInit {
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

  search = "";
  customerId = "";
  totalMin = "";
  totalMax = "";
  page = 1;
  perPage = 25;
  private pending = new Map<string, { body: string; key: string }>();

  async ngOnInit() {
    await this.load();
  }

  async load(reset = false) {
    if (reset) this.page = 1;
    this.loading.set(true);
    this.error.set("");
    try {
      const q = new URLSearchParams({
        page: String(this.page),
        per_page: String(this.perPage),
        status: "received",
      });
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
    this.form = emptyPurchaseOrder();
    this.selected.set(null);
    this.preview.set(null);
    this.error.set("");
    this.message.set("");
    this.editorOpen.set(true);
  }

  close() {
    this.editorOpen.set(false);
    this.selected.set(null);
    this.preview.set(null);
    this.message.set("");
  }

  async open(id: string) {
    if (this.busy()) return;
    this.busy.set(true);
    this.error.set("");
    try {
      const order = await this.api.get<PurchaseOrder>("/purchase-orders/" + id);
      this.selected.set(order);
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

  writable() {
    return this.api.has(this.selected() ? "ordenes.update" : "ordenes.create");
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
    if (this.busy()) return;
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
      const result = await this.api.write<{ calculation: PurchaseOrderCalculation }>(
        "POST",
        "/purchase-orders/calculate",
        body,
      );
      this.preview.set(result.calculation);
    } catch (e) {
      this.preview.set(null);
      this.error.set(this.api.message(e));
    } finally {
      this.busy.set(false);
    }
  }

  async save() {
    if (this.busy() || !this.writable()) return;
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
      this.message.set(id ? "Orden de compra actualizada." : "Orden de compra registrada.");
    } catch (e) {
      this.error.set(this.api.message(e));
    } finally {
      this.busy.set(false);
    }
  }

  private async openAfterSave(id: string) {
    const order = await this.api.get<PurchaseOrder>("/purchase-orders/" + id);
    this.selected.set(order);
    this.form = this.editable(order);
    this.preview.set(order.calculation);
    this.editorOpen.set(true);
  }

  statusLabel(status: string) {
    return status === "received" ? "Recibida" : status;
  }

  itemStatusLabel(status: string) {
    return status === "pending" ? "Pendiente" : status;
  }
}
