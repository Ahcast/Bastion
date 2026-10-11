import { TestBed } from "@angular/core/testing";
import { vi } from "vitest";
import { Client } from "../../core/api/client";
import {
  PurchaseOrder,
  PurchaseOrderCalculation,
  PurchaseOrders,
  emptyPurchaseOrder,
} from "./purchase-orders";

const calculation = (): PurchaseOrderCalculation => ({
  gross: "100.00",
  item_discount: "0.00",
  general_discount: "0.00",
  base_15: "100.00",
  base_18: "0.00",
  exempt: "0.00",
  exonerated: "0.00",
  isv_15: "15.00",
  isv_18: "0.00",
  total: "115.00",
  products_investment: "60.00",
  additional_expenses: "5.00",
  investment_total: "65.00",
  calculator_version: 1,
});

const record = (): PurchaseOrder => ({
  id: "9007199254740993",
  order_number: "OC-TEST-001",
  status: "received",
  customer: {
    id: "",
    name: "Cliente sintético",
    rtn: "",
    contact: "",
    phone: "",
    email: "",
    address: "",
  },
  customer_version: "0",
  source_quote_id: null,
  destination: "Tegucigalpa",
  order_date: "2026-10-11",
  required_delivery_date: null,
  currency: "HNL",
  comments: "",
  exoneration_reference: "",
  general_discount_type: "amount",
  general_discount: "0",
  additional_expenses: "5.00",
  team_id: "",
  version: "1",
  created_by: "42",
  calculation: calculation(),
  items: [
    {
      description: "Equipo sintético",
      unit: "unidad",
      quantity: "1",
      unit_sale_price: "100",
      unit_cost: "60",
      discount_type: "amount",
      discount: "0",
      tax_type: "isv15",
      supplier_id: "",
      supplier_name_snapshot: "Proveedor manual",
      tracking: "",
      comments: "",
      supplier_version: "0",
      status: "pending",
      unit_discount: "0",
      gross: "100.00",
      item_discount: "0.00",
      allocated_discount: "0.00",
      base: "100.00",
      line_investment: "60.00",
    },
  ],
});

function fillDraft(component: PurchaseOrders) {
  component.form = emptyPurchaseOrder();
  component.form.order_number = "OC-TEST-001";
  component.form.customer.name = "Cliente sintético";
  component.form.destination = "Tegucigalpa";
  component.form.order_date = "2026-10-11";
  component.form.additional_expenses = "5.00";
  component.addItem();
  const item = component.form.items[0];
  item.description = "Equipo sintético";
  item.unit_sale_price = "100";
  item.unit_cost = "60";
  item.supplier_name_snapshot = "Proveedor manual";
}

describe("Órdenes de Compra", () => {
  let api: {
    get: ReturnType<typeof vi.fn>;
    write: ReturnType<typeof vi.fn>;
    has: ReturnType<typeof vi.fn>;
    message: ReturnType<typeof vi.fn>;
  };

  beforeEach(() => {
    api = {
      get: vi.fn(async (path: string) =>
        path.startsWith("/purchase-orders?")
          ? { data: [], total: 0, page: 1, per_page: 25 }
          : record(),
      ),
      write: vi.fn(async (_method: string, path: string) =>
        path.endsWith("/calculate")
          ? { calculation: calculation(), items: [] }
          : { id: record().id, version: "1" },
      ),
      has: vi.fn(() => true),
      message: vi.fn(() => "El registro cambió. Vuelva a abrirlo."),
    };
    TestBed.configureTestingModule({ providers: [{ provide: Client, useValue: api }] });
  });

  it("crea un borrador monomoneda con versión string y entrega nullable", () => {
    const value = emptyPurchaseOrder();
    expect(value.currency).toBe("HNL");
    expect(value.version).toBe("0");
    expect(value.required_delivery_date).toBeNull();
    expect(value.items).toEqual([]);
  });

  it("crea una orden con el agregado estricto y una clave idempotente", async () => {
    const component = TestBed.createComponent(PurchaseOrders).componentInstance;
    fillDraft(component);
    await component.save();

    const [method, path, body, key] = api.write.mock.calls[0] as unknown as [
      string,
      string,
      Record<string, unknown>,
      string,
    ];
    expect(method).toBe("POST");
    expect(path).toBe("/purchase-orders");
    expect(body["version"]).toBe("0");
    expect(body["required_delivery_date"]).toBeNull();
    expect((body["items"] as Record<string, string>[])[0]["unit_sale_price"]).toBe("100");
    expect((body["items"] as Record<string, string>[])[0]["unit_cost"]).toBe("60");
    expect(key.length).toBeGreaterThan(8);
    expect(component.selected()?.id).toBe(record().id);
  });

  it("conserva la clave después de un fallo y cambia la clave si cambia la intención", async () => {
    api.write.mockRejectedValue(new Error("conflict"));
    const component = TestBed.createComponent(PurchaseOrders).componentInstance;
    fillDraft(component);

    await component.save();
    await component.save();
    expect(api.write.mock.calls[0][3]).toBe(api.write.mock.calls[1][3]);
    expect(component.form.order_number).toBe("OC-TEST-001");
    expect(component.error()).toContain("registro cambió");

    component.form.order_number = "OC-TEST-002";
    await component.save();
    expect(api.write.mock.calls[2][3]).not.toBe(api.write.mock.calls[1][3]);
  });

  it("calcula sólo con datos financieros y no filtra IDs de proveedor", async () => {
    const component = TestBed.createComponent(PurchaseOrders).componentInstance;
    fillDraft(component);
    component.form.items[0].supplier_id = "25";
    component.form.items[0].tracking = "Seguimiento privado";

    await component.calculate();
    const [method, path, body] = api.write.mock.calls[0] as unknown as [
      string,
      string,
      { items: Record<string, unknown>[]; additional_expenses: string },
    ];
    expect(method).toBe("POST");
    expect(path).toBe("/purchase-orders/calculate");
    expect(body.additional_expenses).toBe("5.00");
    expect(body.items[0]).not.toHaveProperty("supplier_id");
    expect(body.items[0]).not.toHaveProperty("tracking");
    expect(body.items[0]["unit_sale_price"]).toBe("100");
    expect(component.preview()?.total).toBe("115.00");
  });

  it("bloquea edición y creación para un lector", async () => {
    api.has.mockReturnValue(false);
    const fixture = TestBed.createComponent(PurchaseOrders);
    const component = fixture.componentInstance;
    fillDraft(component);
    component.editorOpen.set(true);
    fixture.detectChanges();
    await fixture.whenStable();
    fixture.detectChanges();

    const html = fixture.nativeElement as HTMLElement;
    const creationButton = Array.from(html.querySelectorAll("button")).find(
      (button) => button.textContent?.trim() === "Nueva orden de compra",
    );
    expect(creationButton).toBeUndefined();
    const editorFieldsets = Array.from(html.querySelectorAll("section form fieldset")) as HTMLFieldSetElement[];
    expect(editorFieldsets.length).toBeGreaterThan(0);
    expect(editorFieldsets.every((field) => field.disabled)).toBe(true);
  });

  it("envía filtros y paginación al backend y respeta su total", async () => {
    api.get.mockResolvedValue({ data: [record()], total: 61, page: 2, per_page: 25 });
    const component = TestBed.createComponent(PurchaseOrders).componentInstance;
    component.page = 2;
    component.search = "Equipo";
    component.statusFilter = "confirmed";
    component.customerId = "7";
    component.totalMin = "10.00";
    component.totalMax = "500.00";

    await component.load();
    const path = api.get.mock.calls[0][0] as string;
    expect(path).toContain("page=2");
    expect(path).toContain("status=confirmed");
    expect(path).toContain("customer_id=7");
    expect(path).toContain("total_min=10.00");
    expect(path).toContain("total_max=500.00");
    expect(component.total()).toBe(61);
  });
  it("bloquea transiciones con cambios sin guardar y edición con avance de ítems", async () => {
    const component = TestBed.createComponent(PurchaseOrders).componentInstance;
    const saved = record();
    component.selected.set(saved);
    component.form = component.editable(saved);
    component.form.comments = "Cambio pendiente";
    await component.transition("confirmed");
    expect(api.write).not.toHaveBeenCalled();
    expect(component.error()).toContain("Guarde o descarte");
    saved.items[0].status = "purchased";
    component.selected.set({ ...saved });
    expect(component.writable()).toBe(false);
    expect(component.nextItemStatus(0)).toBe("shipped");
  });

  it("conserva intención de transición ante timeout y versión confirmada ante recarga fallida", async () => {
    const component = TestBed.createComponent(PurchaseOrders).componentInstance;
    component.selected.set(record());
    component.form = component.editable(record());
    api.write.mockRejectedValueOnce(new Error("timeout"));
    await component.transition("confirmed");
    api.write.mockResolvedValueOnce({ id: record().id, version: "2", status: "confirmed", to_status: "confirmed" });
    api.get.mockRejectedValueOnce(new Error("reload"));
    await component.transition("confirmed");
    expect(api.write.mock.calls[0][3]).toBe(api.write.mock.calls[1][3]);
    expect(component.selected()?.status).toBe("confirmed");
    expect(component.selected()?.version).toBe("2");
    expect(component.form.version).toBe("2");
    expect(component.error()).toContain("registro cambió");
  });

  it("no ofrece estados reservados ni entrega incompleta o sin permisos", async () => {
    const component = TestBed.createComponent(PurchaseOrders).componentInstance;
    const saved = record();
    saved.status = "preparing_delivery";
    component.selected.set(saved);
    component.form = component.editable(saved);
    expect(component.nextOrderStatus()).toBe("delivered");
    await component.transition("delivered");
    expect(api.write).not.toHaveBeenCalled();
    api.has.mockReturnValue(false);
    expect(component.nextOrderStatus()).toBeNull();
    expect(component.nextItemStatus(0)).toBeNull();
    saved.status = "delivered";
    component.selected.set({ ...saved });
    expect(component.nextOrderStatus()).toBeNull();
    expect(component.statusLabel("goods_received")).toBe("Recibido");
    expect(component.itemStatusLabel("received")).toBe("Recibido");
  });

  it("consulta el historial paginado por ID string sin transformar la ficha", async () => {
    const component = TestBed.createComponent(PurchaseOrders).componentInstance;
    component.selected.set(record());
    api.get.mockResolvedValueOnce({ data: [{ id: "1", item_position: 1, from_status: "pending", to_status: "purchased", actor_id: "42", purchase_order_version: "2", created_at: "2026-10-11T16:00:00Z" }], total: 26, page: 2, per_page: 25 });
    await component.loadHistory(2);
    expect(api.get.mock.calls[0][0]).toBe("/purchase-orders/9007199254740993/status-history?page=2&per_page=25");
    expect(component.historyPage).toBe(2);
    expect(component.historyTotal()).toBe(26);
    expect(component.history()[0].item_position).toBe(1);
    expect(component.selected()?.version).toBe("1");
  });

});
