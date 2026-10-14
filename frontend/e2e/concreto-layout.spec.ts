import { expect, Page, test } from "@playwright/test";
import { resolve } from "node:path";
// Synthetic browser data is confined to tests; production uses the real Client.
const customers = [
  {
    id: "101",
    name: "Cliente de prueba Norte",
    rtn: "08019999000123",
    email: "norte@example.test",
    is_active: true,
    version: "1",
    contacts: [],
    addresses: [],
    institution_type: "private",
    contact_name: "",
    phone: "",
    address: "",
    team_id: "",
  },
  {
    id: "102",
    name: "Cliente de prueba Sur",
    rtn: "",
    email: "",
    is_active: false,
    version: "1",
    contacts: [],
    addresses: [],
    institution_type: "other",
    contact_name: "",
    phone: "",
    address: "",
    team_id: "",
  },
];
const processes = [
  {
    id: "201",
    entity: "Institución de prueba",
    reference: "PROC-PRUEBA-001",
    description: "Suministros para revisión de diseño",
    deadline: "2026-10-20T15:00:00Z",
    is_source_active: true,
    is_current: true,
    is_interesting: false,
    note: "",
    version: "1",
    personal_version: "1",
    documents: [],
  },
];
async function fixture(page: Page, limited = false) {
  const permissions = limited
    ? ["clientes.view"]
    : [
        "clientes.view",
        "clientes.create",
        "clientes.update",
        "proveedores.view",
        "proveedores.create",
        "procesos.view",
        "cotizaciones.view",
        "cotizaciones.create",
        "ordenes.view",
        "ordenes.create",
        "facturas.view",
        "facturas.create",
        "gastos.view",
        "gastos.create",
        "reportes.view",
        "colaboracion.view",
      ];
  const modules = limited
    ? ["customers"]
    : [
        "customers",
        "suppliers",
        "processes",
        "quotes",
        "purchase-orders",
        "invoices",
        "expenses",
        "reports",
        "collaboration",
      ];
  await page.route("**/config.json", (r) =>
    r.fulfill({ json: { apiUrl: "http://localhost:4200" } }),
  );
  await page.route("**/api/v1/**", async (r) => {
    const path = new URL(r.request().url()).pathname.replace("/api/v1", "");
    const empty = { data: [], total: 0, page: 1, per_page: 25 };
    let json: unknown = empty;
    if (path === "/auth/csrf") json = { csrf_token: "synthetic-test" };
    else if (path === "/auth/status")
      json = { mfa_complete: true, mfa_enabled: true };
    else if (path === "/auth/me")
      json = {
        user: {
          id: "1",
          name: "Usuario de prueba",
          email: "ui@example.test",
          active: true,
          mfa_enabled: true,
          version: "1",
          roles: [],
        },
        grants: permissions.map((permission) => ({ permission, scope: "all" })),
        enabled_modules: modules,
      };
    else if (path === "/auth/preferences")
      json = { theme: "light", page_size: 25 };
    else if (path === "/customers")
      json = { ...empty, data: customers, total: 2 };
    else if (/^\/customers\/\d+$/.test(path))
      json = customers.find((x) => x.id === path.split("/").at(-1));
    else if (path === "/processes")
      json = { ...empty, data: processes, total: 1 };
    else if (path === "/processes/201") json = processes[0];
    else if (path === "/expenses/totals") json = [];
    else if (path.startsWith("/reports/") && path !== "/reports/home")
      json = { ...empty, report: "invoices", filters: {}, totals: [] };
    else if (path === "/reports/home")
      json = {
        filters: {},
        reports: {
          orders: [],
          invoices: [
            {
              currency: "HNL",
              count: 1,
              values: {
                total: "1200.00",
                paid_total: "200.00",
                balance: "1000.00",
              },
            },
          ],
          expenses: [],
        },
      };
    await r.fulfill({ json });
  });
}
async function fits(page: Page) {
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= innerWidth,
    ),
  ).toBe(true);
}
async function capture(page: Page, name: string) {
  if (process.env["BASTION_CAPTURE_UI"]) {
    await page.evaluate(() => scrollTo(0, 0));
    await page.screenshot({
      path: resolve("../docs/ui/capturas", name + ".png"),
      fullPage: true,
    });
  }
}
for (const width of [1440, 390]) {
  test(`Concreto: navigation, workspace selectors, inline records and forms at ${width}px`, async ({
    page,
  }) => {
    await page.setViewportSize({ width, height: 1000 });
    const errors: string[] = [];
    page.on("pageerror", (e) => errors.push(e.message));
    await fixture(page);
    await page.goto("/");
    await expect(
      page.getByRole("heading", { name: "Inicio", exact: true }),
    ).toBeVisible();
    await expect(page.getByRole("tab", { name: /Procesos/ })).toHaveAttribute(
      "aria-selected",
      "true",
    );
    await expect(
      page.getByRole("button", { name: "Ver proceso PROC-PRUEBA-001" }),
    ).toBeVisible();
    await fits(page);
    await capture(page, `inicio-${width}`);
    await page.getByRole("tab", { name: /Cotizaciones/ }).click();
    await expect(
      page
        .locator("#dashboard-work")
        .getByRole("button", { name: "Nueva cotización", exact: true }),
    ).toBeVisible();
    await page.getByRole("tab", { name: /Cotizaciones/ }).press("ArrowLeft");
    await expect(page.getByRole("tab", { name: /Procesos/ })).toHaveAttribute(
      "aria-selected",
      "true",
    );
    await page.getByRole("link", { name: "Clientes", exact: true }).click();
    await expect(
      page.getByRole("button", {
        name: "Abrir cliente Cliente de prueba Norte",
      }),
    ).toBeVisible();
    const first = page.locator(".bs-workspace-table tbody>tr").first();
    await first.press("Enter");
    await expect(page.locator("tr.bs-xp-row")).toHaveCount(1);
    await expect(
      page
        .locator("tr.bs-xp-row")
        .getByRole("heading", { name: "Ficha del cliente" }),
    ).toBeVisible();
    expect(
      await first.evaluate((el) =>
        el.nextElementSibling?.classList.contains("bs-xp-row"),
      ),
    ).toBe(true);
    await fits(page);
    await capture(page, `clientes-${width}`);
    await first.press("Space");
    await expect(page.locator("tr.bs-xp-row")).toHaveCount(0);
    await page
      .getByRole("button", { name: "Nuevo cliente", exact: true })
      .click();
    await expect(page.locator(".bs-inline-new")).toBeVisible();
    await fits(page);
    await page.goto("/processes");
    await expect(
      page.getByRole("button", { name: "Ver proceso PROC-PRUEBA-001" }),
    ).toBeVisible();
    await page
      .getByRole("group", { name: "Filtrar estados de esta página" })
      .getByRole("button", { name: "Vigente", exact: true })
      .click();
    await expect(
      page
        .getByRole("group", { name: "Filtrar estados de esta página" })
        .getByRole("button", { name: "Vigente", exact: true }),
    ).toHaveAttribute("aria-pressed", "true");
    await capture(page, `procesos-${width}`);
    await page.getByRole("link", { name: "Colaboración", exact: true }).click();
    await page
      .getByRole("navigation", { name: "Colaboración", exact: true })
      .getByRole("link", { name: "Mi trabajo", exact: true })
      .click();
    await expect(
      page
        .getByRole("navigation", { name: "Colaboración", exact: true })
        .getByRole("link", { name: "Mi trabajo", exact: true }),
    ).toHaveAttribute("aria-current", "page");
    await fits(page);
    await capture(page, `colaboracion-${width}`);
    await page.getByRole("link", { name: "Reportes", exact: true }).click();
    await expect(
      page.getByRole("heading", { name: "Reportes", exact: true }),
    ).toBeVisible();
    await fits(page);
    await capture(page, `reportes-${width}`);
    await page
      .getByRole("link", { name: "Configuración", exact: true })
      .click();
    await expect(
      page.getByRole("link", { name: "Personal", exact: true }),
    ).toBeVisible();
    await expect(page.getByLabel("Apariencia")).toHaveCount(0);
    await fits(page);
    await capture(page, `configuracion-${width}`);
    for (const [path, title, action] of [
      ["/suppliers", "Proveedores", "Nuevo proveedor"],
      ["/quotes", "Cotizaciones", "Nueva cotización"],
      ["/purchase-orders", "Órdenes de compra", "Nueva orden de compra"],
      ["/invoices", "Facturas", "Nuevo borrador de factura"],
      ["/expenses", "Gastos", "Nuevo gasto"],
    ]) {
      await page.goto(path);
      await expect(
        page.getByRole("heading", { name: title, exact: true }),
      ).toBeVisible();
      await page.getByRole("button", { name: action, exact: true }).click();
      await expect(page.locator(".bs-inline-new")).toBeVisible();
      await fits(page);
      await capture(page, `${path.slice(1)}-${width}`);
    }
    await page
      .getByRole("button", { name: "Menú de usuario", exact: true })
      .click();
    await expect(
      page.getByRole("link", { name: "Mi cuenta", exact: true }),
    ).toBeVisible();
    expect(errors).toEqual([]);
  });
}
test("Concreto keeps restricted menus and customer fields read-only", async ({
  page,
}) => {
  await fixture(page, true);
  await page.goto("/customers");
  await expect(
    page.getByRole("link", { name: "Cotizaciones", exact: true }),
  ).toHaveCount(0);
  await expect(
    page.getByRole("button", { name: "Nuevo cliente", exact: true }),
  ).toHaveCount(0);
  await page
    .getByRole("button", { name: "Abrir cliente Cliente de prueba Norte" })
    .click();
  await expect(page.getByLabel("Nombre del cliente")).toBeDisabled();
});
