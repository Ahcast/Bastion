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
    TestBed.configureTestingModule({
      providers: [{ provide: Client, useValue: api }],
    });
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
    expect(
      (body["items"] as Record<string, string>[])[0]["unit_sale_price"],
    ).toBe("100");
    expect((body["items"] as Record<string, string>[])[0]["unit_cost"]).toBe(
      "60",
    );
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
    const editorFieldsets = Array.from(
      html.querySelectorAll("section form fieldset"),
    ) as HTMLFieldSetElement[];
    expect(editorFieldsets.length).toBeGreaterThan(0);
    expect(editorFieldsets.every((field) => field.disabled)).toBe(true);
  });

  it("envía filtros y paginación al backend y respeta su total", async () => {
    api.get.mockResolvedValue({
      data: [record()],
      total: 61,
      page: 2,
      per_page: 25,
    });
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
    api.write.mockResolvedValueOnce({
      id: record().id,
      version: "2",
      status: "confirmed",
      to_status: "confirmed",
    });
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
    api.get.mockResolvedValueOnce({
      data: [
        {
          id: "1",
          item_position: 1,
          from_status: "pending",
          to_status: "purchased",
          actor_id: "42",
          purchase_order_version: "2",
          created_at: "2026-10-11T16:00:00Z",
        },
      ],
      total: 26,
      page: 2,
      per_page: 25,
    });
    await component.loadHistory(2);
    expect(api.get.mock.calls[0][0]).toBe(
      "/purchase-orders/9007199254740993/status-history?page=2&per_page=25",
    );
    expect(component.historyPage).toBe(2);
    expect(component.historyTotal()).toBe(26);
    expect(component.history()[0].item_position).toBe(1);
    expect(component.selected()?.version).toBe("1");
  });

  it("clona una orden entregada con metadata explícita y conserva la clave tras fallo", async () => {
    const component = TestBed.createComponent(PurchaseOrders).componentInstance;
    const source = { ...record(), status: "delivered" as const };
    component.selected.set(source);
    component.form = component.editable(source);
    component.beginClone();
    component.cloneForm!.order_number = "OC-NEW";
    api.write
      .mockRejectedValueOnce(new Error("network"))
      .mockResolvedValueOnce({ id: "9007199254740995", version: "1" });
    await component.clone();
    await component.clone();
    const first = api.write.mock.calls[0],
      second = api.write.mock.calls[1];
    expect(first[1]).toBe("/purchase-orders/9007199254740993/clone");
    expect(first[3]).toBe(second[3]);
    expect(first[2]).toMatchObject({
      version: "1",
      order_number: "OC-NEW",
      required_delivery_date: null,
      additional_expenses: "0",
      comments: "",
    });
    expect(first[2]).not.toHaveProperty("items");
    expect(component.selected()?.status).toBe("delivered");
    expect(component.cloned()?.id).toBe("9007199254740995");
    await component.clone();
    expect(api.write).toHaveBeenCalledTimes(2);
  });
  it("conserva el destino confirmado tras fallo de listado y bloquea cambios pendientes", async () => {
    const component = TestBed.createComponent(PurchaseOrders).componentInstance;
    const source = record();
    component.selected.set(source);
    component.form = component.editable(source);
    component.form.comments = "sin guardar";
    component.beginClone();
    expect(component.cloneForm).toBeNull();
    component.form = component.editable(source);
    component.beginClone();
    component.cloneForm!.order_number = "OC-NEW";
    api.write.mockResolvedValue({ id: "99", version: "1" });
    api.get.mockRejectedValue(new Error("reload"));
    await component.clone();
    expect(component.cloned()).toEqual({ id: "99", version: "1" });
    await component.open("99");
    expect(component.cloned()?.id).toBe("99");
  });
  it("elimina con motivo y conserva respuesta servidor ante fallo de recarga", async () => {
    const component = TestBed.createComponent(PurchaseOrders).componentInstance;
    const source = record();
    component.selected.set(source);
    component.form = component.editable(source);
    component.trashReason = "Duplicado";
    api.write
      .mockRejectedValueOnce(new Error("lost ACK"))
      .mockResolvedValueOnce({
        id: source.id,
        version: "2",
        deleted_at: "2026-10-11T18:00:00Z",
      });
    api.get.mockRejectedValue(new Error("reload"));
    await component.trash();
    await component.trash();
    const first = api.write.mock.calls[0],
      second = api.write.mock.calls[1];
    expect(first[0]).toBe("DELETE");
    expect(first[2]).toEqual({ version: "1", reason: "Duplicado" });
    expect(first[3]).toBe(second[3]);
    expect(component.selected()?.deleted_at).toBe("2026-10-11T18:00:00Z");
    expect(component.selected()?.version).toBe("2");
    expect(component.writable()).toBe(false);
    expect(component.nextOrderStatus()).toBeNull();
    expect(component.nextItemStatus(0)).toBeNull();
    component.beginClone();
    expect(component.cloneForm).toBeNull();
  });
  it("restaura con versión vigente y exige motivo/cambios guardados", async () => {
    const component = TestBed.createComponent(PurchaseOrders).componentInstance;
    const source = {
      ...record(),
      deleted_at: "2026-10-11T18:00:00Z",
      version: "2",
    };
    component.selected.set(source);
    component.form = component.editable(source);
    await component.trash();
    expect(api.write).not.toHaveBeenCalled();
    component.trashReason = "Corrección";
    component.form.comments = "Sin guardar";
    await component.trash();
    expect(api.write).not.toHaveBeenCalled();
    component.form = component.editable(source);
    api.write.mockResolvedValue({
      id: source.id,
      version: "3",
      deleted_at: null,
    });
    api.get.mockResolvedValue({ ...source, version: "3", deleted_at: null });
    await component.trash();
    expect(api.write.mock.calls[0][0]).toBe("POST");
    expect(api.write.mock.calls[0][1]).toBe(
      "/purchase-orders/" + source.id + "/restore",
    );
    expect(component.selected()?.deleted_at).toBeNull();
  });
 it('preserva vínculo/versión de facturación confirmado y bloquea navegación durante petición',async()=>{const component=TestBed.createComponent(PurchaseOrders).componentInstance;const source=record();api.get.mockResolvedValue(source);await component.open(source.id);component.billingBusy.set(true);component.create();component.close();await component.open('2');expect(component.selected()?.id).toBe(source.id);component.billingBusy.set(false);api.get.mockRejectedValue(new Error('Recarga'));await component.billingConfirmed({id:source.id,version:'2',status:'delivered',invoice_id:'9007199254740994',invoice_version:'1'});expect(component.billingNeedsReload()).toBe(true);expect(component.selected()?.invoice_id).toBe('9007199254740994');expect(component.form.version).toBe('2');expect(component.message()).toContain('9007199254740994')});

});
