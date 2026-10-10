import { TestBed } from "@angular/core/testing";
import { vi } from "vitest";
import { Client } from "../../core/api/client";
import { Suppliers, Supplier } from "./suppliers";

describe("Proveedores", () => {
  const record = (): Supplier => ({
    id: "9007199254740993",
    name: "Proveedor sintético",
    rtn: "",
    contact_name: "",
    phone: "",
    email: "",
    address: "",
    country: "Honduras",
    website: "https://example.test",
    notes: "",
    payment_terms: "",
    lead_time: "",
    evaluation: null,
    reliability: "unknown",
    response_hours: null,
    categories: [],
    brands: [],
    is_active: true,
    team_id: "",
    version: "7",
    contacts: [],
    products: [
      {
        name: "Producto",
        brand: "",
        part_number: "SKU-1",
        description: "",
        lead_time: "",
        moq: "10.2500",
      },
    ],
  });
  let api: {
    get: ReturnType<typeof vi.fn>;
    write: ReturnType<typeof vi.fn>;
    has: ReturnType<typeof vi.fn>;
    message: ReturnType<typeof vi.fn>;
  };
  beforeEach(() => {
    api = {
      get: vi.fn(async (path: string) =>
        path.includes("?") || path.endsWith("/teams")
          ? { data: [], total: 0, page: 1, per_page: 25 }
          : { ...record(), created_by: "42" },
      ),
      write: vi.fn(async (_method: string, path: string) => ({
        id: record().id,
        version: path.includes("price-history") ? "9" : "8",
      })),
      has: vi.fn(() => true),
      message: vi.fn(() => "El registro cambió. Vuelva a abrirlo."),
    };
    TestBed.configureTestingModule({
      providers: [{ provide: Client, useValue: api }],
    });
  });
  it("envía DTO explícito, colecciones sin campos protegidos y decimales exactos", async () => {
    const c = TestBed.createComponent(Suppliers).componentInstance;
    const p = record();
    (p.products[0] as unknown as Record<string, unknown>)["id"] = "99";
    c.setEditor({ ...p, created_by: "42" } as Supplier);
    c.categoriesText = "Ferretería\nEléctrico";
    await c.save();
    const [method, path, body, key] = api.write.mock.calls[0] as unknown as [
      string,
      string,
      Record<string, unknown>,
      string,
    ];
    expect(method).toBe("PATCH");
    expect(path).toBe("/suppliers/9007199254740993");
    expect(body["version"]).toBe("7");
    expect(body).not.toHaveProperty("created_by");
    expect(body).not.toHaveProperty("id");
    expect(
      (body["products"] as Record<string, unknown>[])[0],
    ).not.toHaveProperty("id");
    expect((body["products"] as Record<string, unknown>[])[0]["moq"]).toBe(
      "10.2500",
    );
    expect(body["categories"]).toEqual(["Ferretería", "Eléctrico"]);
    expect(key.length).toBeGreaterThan(8);
  });
  it("conserva edición y clave ante fallo, cambia clave para una intención nueva", async () => {
    api.write.mockRejectedValue(new Error("conflict"));
    const c = TestBed.createComponent(Suppliers).componentInstance;
    c.setEditor(record());
    c.editor()!.name = "Edición pendiente";
    await c.save();
    await c.save();
    expect(api.write.mock.calls[0][3]).toBe(api.write.mock.calls[1][3]);
    expect(c.editor()?.name).toBe("Edición pendiente");
    c.editor()!.name = "Otra intención";
    await c.save();
    expect(api.write.mock.calls[2][3]).not.toBe(api.write.mock.calls[1][3]);
  });
  it("bloquea edición y observaciones para un lector", async () => {
    api.has.mockReturnValue(false);
    const f = TestBed.createComponent(Suppliers);
    f.componentInstance.setEditor(record());
    f.detectChanges();
    await f.whenStable();
    const html = f.nativeElement as HTMLElement;
    expect(html.textContent).not.toContain("Nuevo proveedor");
    expect(html.querySelector("fieldset")?.disabled).toBe(true);
    expect(html.textContent).not.toContain("Registrar observación");
    expect(html.textContent).not.toContain("Registrar nota");
  });
  it("solicita filtros y paginación SQL y usa el total del servidor", async () => {
    api.get.mockResolvedValue({
      data: [record()],
      total: 80,
      page: 3,
      per_page: 25,
    });
    const c = TestBed.createComponent(Suppliers).componentInstance;
    c.page = 3;
    c.country = "Honduras";
    c.active = "false";
    await c.load();
    const path = api.get.mock.calls[0][0] as string;
    expect(path).toContain("page=3");
    expect(path).toContain("country=Honduras");
    expect(path).toContain("is_active=false");
    expect(c.total()).toBe(80);
  });
  it("añadir nota o evidencia no borra cambios pendientes de la ficha", async () => {
    const c = TestBed.createComponent(Suppliers).componentInstance;
    c.setEditor(record());
    c.editor()!.name = "Edición todavía sin guardar";
    c.editor()!.products[0].description = "Descripción pendiente";
    c.categoriesText = "Nueva categoría";
    c.note = "Nota histórica";
    await c.addNote();
    expect(c.editor()?.version).toBe("8");
    expect(c.editor()?.name).toBe("Edición todavía sin guardar");
    c.price = {
      source_url: "https://example.test/product",
      source_type: "web",
      observed_at: "2026-01-01T10:30",
      part_number: "SKU-1",
      observed_price: "12.345678",
      currency: "USD",
      availability: "",
      lead_time: "",
      moq: "",
      taxes: "unknown",
      shipping: "unknown",
    };
    await c.recordPrice();
    expect(c.editor()?.version).toBe("9");
    expect(c.editor()?.products[0].description).toBe("Descripción pendiente");
    expect(c.categoriesText).toBe("Nueva categoría");
    const body = api.write.mock.calls[1][2] as unknown as Record<
      string,
      unknown
    >;
    expect(body["observed_price"]).toBe("12.345678");
    expect(body["version"]).toBe("8");
    expect(body["source_observed_at"]).toBe("2026-01-01T16:30:00.000Z");
    expect(body).not.toHaveProperty("observed_at");
  });
  it("conserva ID confirmado tras POST aunque falle la recarga; el próximo guardado es PATCH", async () => {
    api.get.mockRejectedValue(new Error("network"));
    const c = TestBed.createComponent(Suppliers).componentInstance;
    c.create();
    c.editor()!.name = "Nuevo proveedor";
    await c.save();
    expect(c.editor()?.id).toBe("9007199254740993");
    expect(c.editor()?.version).toBe("8");
    expect(c.editor()?.name).toBe("Nuevo proveedor");
    await c.save();
    expect(api.write.mock.calls[0][0]).toBe("POST");
    expect(api.write.mock.calls[1][0]).toBe("PATCH");
  });
});
