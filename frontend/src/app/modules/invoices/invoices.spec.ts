import { TestBed } from "@angular/core/testing";
import { vi } from "vitest";
import { Client } from "../../core/api/client";
import { Invoices, Invoice, Financials, emptyInvoice } from "./invoices";
const financials = (): Financials => ({
  total: "201.25",
  profit: "85.01",
  investment_total: "89.99",
  products_investment: "80.00",
  additional_expenses: "9.99",
  base_15: "175.00",
  base_18: "0.00",
  exempt: "0.00",
  exonerated: "0.00",
  isv_15: "26.25",
  isv_18: "0.00",
});
const invoice = (): Invoice => {
  const p = emptyInvoice();
  return {
    ...p,
    id: "9007199254740993",
    status: "draft",
    invoice_number: null,
    sar_snapshot_json: null,
    customer_version: "0",
    customer: { ...p.customer, name: "Cliente sintético" },
    calculation: financials(),
    paid_total: "0.00",
    balance: "201.25",
    version: "1",
    created_by: "1",
    items: [
      {
        ...p.items[0],
        id: "9007199254740995",
        description: "Equipo sintético",
      },
    ],
  };
};
function setup(
  permissions = ["facturas.view", "facturas.create", "facturas.update"],
) {
  const api = {
    moduleEnabled: vi.fn(() => true),
    profile: vi.fn(() => null),
    has: vi.fn((p: string) => permissions.includes(p)),
    get: vi.fn(async (path: string): Promise<unknown> =>
      path.includes("?") ? { data: [], total: 0 } : invoice(),
    ),
    write: vi.fn(),
    message: vi.fn(() => "Fallo sintético"),
  };
  TestBed.configureTestingModule({
    providers: [{ provide: Client, useValue: api }],
  });
  const component = TestBed.runInInjectionContext(() => new Invoices());
  return { api, component };
}
describe("Facturas Borradores", () => {
  it("calcula en backend con cadenas exactas y descarta una vista previa al cambiar sus datos", async () => {
    const { api, component: c } = setup();
    c.newDraft();
    c.form.items[0] = {
      ...c.form.items[0],
      id: "9007199254740995",
      quantity: "2.125",
      unit_price: "900719925474.125125",
    };
    api.write.mockResolvedValue({ calculation: financials() });
    await c.preview();
    const body = api.write.mock.calls[0][2];
    expect(body.items[0].quantity).toBe("2.125");
    expect(body.items[0].unit_price).toBe("900719925474.125125");
    expect(body.items[0].id).toBeUndefined();
    expect(c.form.items[0].id).toBe("9007199254740995");
    expect(c.currentCalculation()?.profit).toBe("85.01");
    c.form.currency = "USD";
    expect(c.currentCalculation()).toBeNull();
  });
  it("retiene la intención después de respuesta incierta y respeta IDs/versiones string", async () => {
    const { api, component: c } = setup();
    c.newDraft();
    c.form.customer.name = "Cliente sintético";
    c.form.items[0].description = "Equipo sintético";
    api.write
      .mockRejectedValueOnce(new Error("ACK perdido"))
      .mockResolvedValueOnce({ id: invoice().id, version: "1" });
    await c.save();
    const key = api.write.mock.calls[0][3];
    expect(c.form.version).toBe("0");
    expect(c.confirmed()).toBeNull();
    await c.save();
    expect(api.write.mock.calls[1][3]).toBe(key);
    expect(c.selected()?.id).toBe("9007199254740993");
    expect(c.form.items[0].id).toBe("9007199254740995");
    expect(c.form.version).toBe("1");
    expect(c.needsReload()).toBe(false);
  });
  it("conserva el resultado confirmado y bloquea otro write hasta recuperar la ficha real", async () => {
    const { api, component: c } = setup();
    c.newDraft();
    api.write.mockResolvedValue({ id: invoice().id, version: "1" });
    api.get.mockRejectedValue(new Error("recarga fallida"));
    await c.save();
    expect(c.confirmed()).toEqual({ id: invoice().id, version: "1" });
    expect(c.needsReload()).toBe(true);
    expect(c.writable()).toBe(false);
    await c.save();
    expect(api.write).toHaveBeenCalledTimes(1);
    api.get.mockResolvedValue(invoice());
    await c.open(invoice().id);
    expect(c.needsReload()).toBe(false);
    expect(c.writable()).toBe(true);
  });
  it("no ofrece writes sin permiso y cambia crédito/contado sin conservar días incompatibles", async () => {
    const { api, component: c } = setup(["facturas.view"]);
    c.newDraft();
    expect(c.editing()).toBe(false);
    await c.open(invoice().id);
    expect(c.writable()).toBe(false);
    await c.save();
    expect(api.write).not.toHaveBeenCalled();
    c.form.payment_term_type = "credito";
    c.terms();
    expect(c.form.credit_days).toBe(30);
    c.form.payment_term_type = "contado";
    c.terms();
    expect(c.form.credit_days).toBeNull();
  });
  it("guarda edición con PATCH, conserva los IDs y la versión confirmada", async () => {
    const { api, component: c } = setup();
    await c.open(invoice().id);
    c.form.public_notes = "Nueva nota";
    api.write.mockResolvedValue({ id: invoice().id, version: "2" });
    api.get.mockImplementation(async (path: string) =>
      path.includes("?")
        ? { data: [], total: 0 }
        : { ...invoice(), version: "2" },
    );
    await c.save();
    expect(api.write.mock.calls[0][0]).toBe("PATCH");
    expect(api.write.mock.calls[0][1]).toBe("/invoices/" + invoice().id);
    expect(api.write.mock.calls[0][2].version).toBe("1");
    expect(api.write.mock.calls[0][2].items[0].id).toBe(invoice().items[0].id);
    expect(c.form.version).toBe("2");
  });
  it("etiqueta los selectores sin incluir las opciones y permite elegir Crédito", async () => {
    setup();
    const fixture = TestBed.createComponent(Invoices);
    fixture.componentInstance.newDraft();
    fixture.detectChanges();
    await fixture.whenStable();
    const select = fixture.nativeElement.querySelector(
      "#invoice-term",
    ) as HTMLSelectElement;
    expect(select.labels![0].textContent).toBe("Condición de pago");
    select.value = "credito";
    select.dispatchEvent(new Event("change"));
    await fixture.whenStable();
    expect(fixture.componentInstance.form.payment_term_type).toBe("credito");
    expect(fixture.componentInstance.form.credit_days).toBe(30);
  });

  it("consulta clientes autorizados, conserva la identidad seleccionada y no refresca el mismo snapshot", async () => {
    const { api, component: c } = setup([
      "facturas.view",
      "facturas.create",
      "clientes.view",
    ]);
    c.newDraft();
    c.customerSearch = "Cliente autorizado";
    api.get.mockResolvedValueOnce({
      data: [{ id: "9", name: "Cliente autorizado" }],
      total: 1,
    });
    await c.findCustomers();
    expect(api.get.mock.calls[0][0]).toContain("is_active=true");
    api.get.mockResolvedValueOnce({
      id: "9",
      name: "Cliente autorizado",
      rtn: "",
      contact_name: "Compras",
      phone: "",
      email: "contact@example.test",
      address: "Dirección sintética",
    });
    c.chosenCustomerID = "9";
    await c.chooseCustomer();
    expect(c.form.customer.name).toBe("Cliente autorizado");
    expect(c.form.customer.contact).toBe("Compras");
    const calls = api.get.mock.calls.length;
    await c.chooseCustomer();
    expect(api.get.mock.calls.length).toBe(calls);
    api.get.mockRejectedValueOnce(new Error("acceso revocado"));
    c.chosenCustomerID = "10";
    await c.chooseCustomer();
    expect(c.form.customer.id).toBe("9");
    expect(c.chosenCustomerID).toBe("9");
  });
  it("no consulta catálogos ni equipos sin permisos", async () => {
    const { api, component: c } = setup();
    await c.findCustomers();
    await c.loadTeams();
    expect(api.get).not.toHaveBeenCalled();
  });
});
