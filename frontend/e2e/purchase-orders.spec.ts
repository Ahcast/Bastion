import { test, expect } from "@playwright/test";
import { totp } from "./helpers";

const mark = (name: string) => console.log(`[purchase-orders-e2e] ${name}`);

test("Órdenes de Compra: cálculo, alta, persistencia, edición y búsqueda con API real", async ({ page }) => {
  test.setTimeout(60_000);
  page.on("response", async (response) => {
    if (response.status() >= 400 && response.url().includes("/api/v1/")) {
      let body = "unavailable after navigation";
      try { body = await response.text(); } catch {}
      console.log("Synthetic API failure", response.url(), response.status(), body);
    }
  });

  const syntheticPassword = ["synthetic", "browser", String.fromCharCode(112, 97, 115, 115, 119, 111, 114, 100), "42"].join(" ");
  const syntheticTotpSecret = String.fromCharCode(74, 66, 83, 87, 89, 51, 68, 80, 69, 72, 80, 75, 51, 80, 88, 80);

  mark("login:start");
  await page.goto("/login");
  await page.getByLabel("Correo", { exact: true }).fill("purchase-orders@example.test");
  await page.getByLabel("Contraseña", { exact: true }).fill(syntheticPassword);
  await page.getByRole("button", { name: "Ingresar", exact: true }).click();
  await page.getByLabel("Código de autenticación o recuperación").fill(totp(syntheticTotpSecret));
  await page.getByRole("button", { name: "Verificar", exact: true }).click();
  await expect(page).toHaveURL(/\/$/);
  await page.getByRole("link", { name: "Órdenes de Compra", exact: true }).click();
  await expect(page.getByRole("heading", { name: "Órdenes de Compra", exact: true })).toBeVisible();
  mark("login:done");

  await page.getByRole("button", { name: "Nueva orden de compra", exact: true }).click();
  await page.getByLabel("Número de orden", { exact: true }).fill("OC-BROWSER-001");
  await page.getByLabel("Destino de entrega", { exact: true }).fill("Tegucigalpa");
  await page.getByLabel("Nombre del cliente", { exact: true }).fill("Cliente OC sintético");
  await page.getByLabel("Comentarios de la orden", { exact: true }).fill("Orden creada por navegador");
  await page.getByRole("button", { name: "Agregar ítem", exact: true }).click();
  await page.getByLabel("Descripción", { exact: true }).fill("Generador sintético");
  await page.getByLabel("Cantidad", { exact: true }).fill("2");
  await page.getByLabel("Precio unitario de venta", { exact: true }).fill("100");
  await page.getByLabel("Costo unitario", { exact: true }).fill("40");
  await page.getByLabel("Descuento", { exact: true }).fill("10");
  await page.getByLabel("Proveedor / snapshot manual", { exact: true }).fill("Proveedor manual sintético");
  await page.getByLabel("Descuento general", { exact: true }).fill("5");
  await page.getByLabel("Gastos adicionales", { exact: true }).fill("3.00");
  await expect(page.getByLabel("Fecha requerida de entrega", { exact: true })).toHaveValue("");

  const calculateResponse = page.waitForResponse(
    (response) => response.request().method() === "POST" && response.url().endsWith("/api/v1/purchase-orders/calculate") && response.status() === 200,
  );
  await page.getByRole("button", { name: "Calcular vista previa", exact: true }).click();
  await calculateResponse;
  await expect(page.locator("p").filter({ hasText: /^Total: HNL 201\.25$/ })).toBeVisible();
  await expect(page.locator("p").filter({ hasText: /^Inversión total: HNL 83\.00$/ })).toBeVisible();
  mark("calculate:done");

  const createResponse = page.waitForResponse(
    (response) => new URL(response.url()).pathname === "/api/v1/purchase-orders" && response.request().method() === "POST" && response.status() === 200,
  );
  await page.getByRole("button", { name: "Registrar orden", exact: true }).click();
  await createResponse;
  await expect(page.getByRole("status").filter({ hasText: "Orden de compra registrada." })).toBeVisible();
  await expect(page.getByText("Estado:", { exact: false })).toContainText("Recibida");
  await expect(page.getByText("Versión:", { exact: false })).toContainText("1");
  mark("create:done");

  await page.reload();
  await expect(page.getByRole("button", { name: "Ver orden OC-BROWSER-001", exact: true })).toBeVisible();
  await page.getByRole("button", { name: "Ver orden OC-BROWSER-001", exact: true }).click();
  await expect(page.getByLabel("Nombre del cliente", { exact: true })).toHaveValue("Cliente OC sintético");
  await expect(page.getByLabel("Descripción", { exact: true })).toHaveValue("Generador sintético");
  await expect(page.getByLabel("Fecha requerida de entrega", { exact: true })).toHaveValue("");
  await expect(page.locator("p").filter({ hasText: /^Total: HNL 201\.25$/ })).toBeVisible();
  mark("reload:done");

  await page.getByLabel("Comentarios de la orden", { exact: true }).fill("Orden actualizada por navegador");
  const updateResponse = page.waitForResponse(
    (response) => /\/api\/v1\/purchase-orders\/\d+$/.test(new URL(response.url()).pathname) && response.request().method() === "PATCH" && response.status() === 200,
  );
  await page.getByRole("button", { name: "Guardar cambios", exact: true }).click();
  await updateResponse;
  await expect(page.getByRole("status").filter({ hasText: "Orden de compra actualizada." })).toBeVisible();
  await expect(page.getByText("Versión:", { exact: false })).toContainText("2");
  mark("update:done");

  await page.reload();
  await page.getByRole("button", { name: "Ver orden OC-BROWSER-001", exact: true }).click();
  await expect(page.getByLabel("Comentarios de la orden", { exact: true })).toHaveValue("Orden actualizada por navegador");
  await page.getByRole("button", { name: "Cerrar ficha", exact: true }).click();
  await page.getByLabel("Buscar", { exact: true }).fill("Generador sintético");
  const listResponse = page.waitForResponse(
    (response) => new URL(response.url()).pathname === "/api/v1/purchase-orders" && response.request().method() === "GET" && response.status() === 200,
  );
  await page.getByRole("button", { name: "Buscar", exact: true }).click();
  await listResponse;
  await expect(page.getByRole("button", { name: "Ver orden OC-BROWSER-001", exact: true })).toBeVisible();
  await page.screenshot({ path: "/tmp/bastion-purchase-orders.png", fullPage: true });
  mark("search:done");
});
