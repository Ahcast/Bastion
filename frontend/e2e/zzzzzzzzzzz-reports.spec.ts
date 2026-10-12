import { test, expect } from "@playwright/test";
import { totp } from "./helpers";
test("Reportes: filas y totales autorizados por moneda coinciden con Inicio", async ({
  page,
}) => {
  await page.goto("/login");
  await page.getByLabel("Correo", { exact: true }).fill("reports@example.test");
  await page
    .getByLabel("Contraseña", { exact: true })
    .fill("synthetic browser password 42");
  await page.getByRole("button", { name: "Ingresar", exact: true }).click();
  await page
    .getByLabel("Código de autenticación o recuperación")
    .fill(totp("JBSWY3DPEHPK3PXP"));
  await page.getByRole("button", { name: "Verificar", exact: true }).click();
  await expect(
    page.getByRole("heading", { name: "Resumen comercial", exact: true }),
  ).toBeVisible();
  await page.getByRole("link", { name: "Reportes", exact: true }).click();
  await expect(
    page.getByRole("heading", { name: "Reportes", exact: true }),
  ).toBeVisible();
  const response = page.waitForResponse(
    (r) => r.url().includes("/api/v1/reports/expenses?") && r.status() === 200,
  );
  await page.getByLabel("Reporte", { exact: true }).selectOption("expenses");
  const expenses = await (await response).json();
  expect(expenses.total).toBeGreaterThan(0);
  expect(
    expenses.totals.map((x: { currency: string }) => x.currency),
  ).toContain("HNL");
  expect(
    expenses.totals.map((x: { currency: string }) => x.currency),
  ).toContain("USD");
  await expect(
    page.getByRole("heading", { name: "Totales USD", exact: true }),
  ).toBeVisible();
  await page.getByLabel("Desde", { exact: true }).fill("2026-10-12");
  await page.getByLabel("Hasta", { exact: true }).fill("2026-10-12");
  await page.getByLabel("Moneda", { exact: true }).selectOption("USD");
  const filtered = page.waitForResponse(
    (r) =>
      r.url().includes("/reports/expenses?") &&
      r.url().includes("currency=USD") &&
      r.status() === 200,
  );
  await page.getByRole("button", { name: "Consultar", exact: true }).click();
  const report = await (await filtered).json();
  await expect(
    page.getByRole("heading", { name: "Totales HNL", exact: true }),
  ).toHaveCount(0);
  await page.getByRole("link", { name: "Inicio", exact: true }).click();
  await page.getByLabel("Desde", { exact: true }).fill("2026-10-12");
  await page.getByLabel("Hasta", { exact: true }).fill("2026-10-12");
  await page.getByLabel("Moneda", { exact: true }).selectOption("USD");
  const home = page.waitForResponse(
    (r) =>
      r.url().includes("/reports/home?") &&
      r.url().includes("currency=USD") &&
      r.status() === 200,
  );
  await page
    .getByRole("button", { name: "Actualizar resumen", exact: true })
    .click();
  expect((await (await home).json()).reports.expenses).toEqual(report.totals);
  await page.getByRole("link", { name: "Reportes", exact: true }).click();
  await page.getByLabel("Reporte", { exact: true }).selectOption("orders");
  await expect(
    page.getByRole("columnheader", { name: "Inversión total", exact: true }),
  ).toBeVisible();
  await expect(
    page.getByRole("columnheader", { name: "Utilidad", exact: true }),
  ).toBeVisible();
  await page.reload();
  await expect(
    page.getByRole("heading", { name: "Reportes", exact: true }),
  ).toBeVisible();
});
