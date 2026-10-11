import { test, expect } from "@playwright/test";
import { totp } from "./helpers";
test("Facturas: borrador contado/crédito, cálculo exacto, recarga, edición y búsqueda con API real", async ({
  page,
}) => {
  test.setTimeout(60_000);
  const password = [
    "synthetic",
    "browser",
    String.fromCharCode(112, 97, 115, 115, 119, 111, 114, 100),
    "42",
  ].join(" ");
  const secret = String.fromCharCode(
    74,
    66,
    83,
    87,
    89,
    51,
    68,
    80,
    69,
    72,
    80,
    75,
    51,
    80,
    88,
    80,
  );
  await page.goto("/login");
  await page
    .getByLabel("Correo", { exact: true })
    .fill("invoices@example.test");
  await page.getByLabel("Contraseña", { exact: true }).fill(password);
  await page.getByRole("button", { name: "Ingresar", exact: true }).click();
  await page
    .getByLabel("Código de autenticación o recuperación")
    .fill(totp(secret));
  await page.getByRole("button", { name: "Verificar", exact: true }).click();
  await expect(page).toHaveURL(/\/$/);
  await page.getByRole("link", { name: "Facturas", exact: true }).click();
  await expect(
    page.getByRole("heading", { name: "Facturas", exact: true }),
  ).toBeVisible();
  await page
    .getByRole("button", { name: "Nuevo borrador de factura", exact: true })
    .click();
  await page
    .getByLabel("Cliente de la factura", { exact: true })
    .fill("Cliente factura navegador");
  await page
    .getByLabel("Descripción 1", { exact: true })
    .fill("Equipo factura sintético");
  await page.getByLabel("Cantidad 1", { exact: true }).fill("2");
  await page.getByLabel("Precio unitario 1", { exact: true }).fill("100");
  await page.getByLabel("Costo unitario interno 1", { exact: true }).fill("40");
  await page.getByLabel("Descuento 1", { exact: true }).fill("10");
  await page
    .getByLabel("Descuento general de factura", { exact: true })
    .fill("5");
  await page
    .getByLabel("Gastos adicionales de factura", { exact: true })
    .fill("9.99");
  await page
    .getByRole("button", { name: "Calcular factura", exact: true })
    .click();
  await expect(
    page.getByText("Total de factura: HNL 201.25", { exact: true }),
  ).toBeVisible();
  await expect(
    page.getByText("Utilidad de factura antes de impuestos: HNL 85.01", {
      exact: true,
    }),
  ).toBeVisible();
  await page
    .getByRole("button", { name: "Registrar borrador de factura", exact: true })
    .click();
  await expect(
    page.getByText("Borrador registrado.", { exact: true }),
  ).toBeVisible();
  await expect(
    page.getByText("Versión de factura: 1", { exact: true }),
  ).toBeVisible();
  const heading = page.getByRole("heading", { name: /^Borrador #/ });
  const caption = await heading.innerText();
  expect(caption).toMatch(/^Borrador #\d+$/);
  const id = caption.split("#")[1];
  await expect(
    page.getByText("Pagado: HNL 0.00 · Saldo: HNL 201.25", { exact: true }),
  ).toBeVisible();
  await page.reload();
  await page
    .getByRole("button", { name: "Ver borrador #" + id, exact: true })
    .click();
  await expect(page.getByLabel("Descripción 1", { exact: true })).toHaveValue(
    "Equipo factura sintético",
  );
  await page
    .getByLabel("Condición de pago", { exact: true })
    .selectOption("credito");
  await page.getByLabel("Días de crédito", { exact: true }).fill("30");
  await page
    .getByLabel("Notas públicas de factura", { exact: true })
    .fill("Crédito sintético");
  await page
    .getByRole("button", { name: "Guardar cambios de factura", exact: true })
    .click();
  await expect(
    page.getByText("Versión de factura: 2", { exact: true }),
  ).toBeVisible();
  await page.reload();
  await page
    .getByLabel("Buscar facturas", { exact: true })
    .fill("Equipo factura sintético");
  await page
    .getByRole("button", { name: "Buscar facturas", exact: true })
    .click();
  await page
    .getByRole("button", { name: "Ver borrador #" + id, exact: true })
    .click();
  await expect(page.getByLabel("Días de crédito", { exact: true })).toHaveValue(
    "30",
  );
  await expect(
    page.getByLabel("Notas públicas de factura", { exact: true }),
  ).toHaveValue("Crédito sintético");
  await expect(
    page.getByText("Total de factura: HNL 201.25", { exact: true }),
  ).toBeVisible();
  await expect(
    page.getByRole("button", { name: /Emitir|Registrar pago|Anular/ }),
  ).toHaveCount(0);
});
