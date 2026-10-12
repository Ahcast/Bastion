import { TestBed } from "@angular/core/testing";
import { describe, it, expect, vi } from "vitest";
import { Client } from "../../core/api/client";
import { Reports, ReportPage } from "./reports";
import { HomeMetrics } from "./home-metrics";
function api() {
  return {
    has: vi.fn(() => true),
    moduleEnabled: vi.fn((_k: string) => true),
    get: vi.fn(),
    message: vi.fn(() => "Acceso denegado"),
  };
}
function fixture(a: ReturnType<typeof api>) {
  TestBed.configureTestingModule({
    providers: [{ provide: Client, useValue: a }],
  });
  return TestBed.runInInjectionContext(() => new Reports());
}
const result: ReportPage = {
  report: "invoices",
  filters: {
    date_from: "2026-10-01",
    date_to: "2026-10-12",
    currency: "USD",
    status: "issued",
  },
  data: [{ id: "9007199254740993", total: "9200000000000000.01" }],
  total: 80,
  page: 1,
  per_page: 25,
  totals: [
    { currency: "USD", count: 80, values: { total: "9200000000000000.01" } },
  ],
};
describe("Reportes", () => {
  it("página conserva filtros consultados y valores exactos aunque se edite formulario", async () => {
    const a = api();
    a.get.mockResolvedValue(result);
    const c = fixture(a);
    await c.load();
    c.from = "2030-01-01";
    c.result.set(result);
    await c.load(2, true);
    expect(a.get.mock.lastCall?.[0]).toContain("date_from=2026-10-01");
    expect(a.get.mock.lastCall?.[0]).toContain("page=2");
    expect(c.result()?.data[0]["id"]).toBe("9007199254740993");
    expect(c.result()?.totals[0].values["total"]).toBe("9200000000000000.01");
  });
  it("ignora respuesta tardía del reporte anterior", async () => {
    const a = api();
    let resolve!: (v: ReportPage) => void;
    a.get.mockReturnValue(new Promise<ReportPage>((r) => (resolve = r)));
    const c = fixture(a);
    a.get.mockResolvedValue({ ...result, report: "expenses" });
    c.kind = "expenses";
    await c.load();
    resolve(result);
    await Promise.resolve();
    expect(c.result()?.report).toBe("expenses");
  });
  it("limpia datos ante permiso revocado o error y no ofrece fuente deshabilitada", async () => {
    const a = api();
    a.get.mockResolvedValue(result);
    const c = fixture(a);
    await c.load();
    a.get.mockRejectedValue(new Error());
    await c.load();
    expect(c.result()).toBeNull();
    expect(c.error()).toBe("Acceso denegado");
    a.moduleEnabled.mockImplementation((k: string) => k !== "invoices");
    expect(c.available()).not.toContain("invoices");
    const calls = a.get.mock.calls.length;
    await c.load();
    expect(a.get).toHaveBeenCalledTimes(calls);
  });
  it("Inicio usa endpoint autorizado del mismo período sin calcular totales", async () => {
    const a = api();
    a.get.mockResolvedValue({
      filters: {},
      reports: { expenses: result.totals },
    });
    TestBed.configureTestingModule({
      providers: [{ provide: Client, useValue: a }],
    });
    const c = TestBed.runInInjectionContext(() => new HomeMetrics());
    c.from = "2026-10-01";
    c.currency = "USD";
    await c.load();
    expect(a.get.mock.lastCall?.[0]).toBe(
      "/reports/home?date_from=2026-10-01&currency=USD",
    );
    expect(c.metrics()?.reports["expenses"][0].values["total"]).toBe(
      "9200000000000000.01",
    );
    a.has.mockReturnValue(false);
    await c.load();
    expect(c.metrics()).toBeNull();
  });
});
