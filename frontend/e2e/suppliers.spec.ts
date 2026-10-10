import { test, expect } from "@playwright/test";
import { totp } from "./helpers";
test("Proveedores: catálogo, productos, notas, evidencia exacta, persistencia, duplicados y filtros con API real", async ({
  page,
}) => {
  await page.goto("/login");
  await page
    .getByLabel("Correo", { exact: true })
    .fill("suppliers@example.test");
  await page
    .getByLabel("Contraseña", { exact: true })
    .fill("synthetic browser password 42");
  await page.getByRole("button", { name: "Ingresar", exact: true }).click();
  await page
    .getByLabel("Código de autenticación o recuperación")
    .fill(totp("JBSWY3DPEHPK3PXP"));
  await page.getByRole("button", { name: "Verificar", exact: true }).click();
  await page.getByRole("link", { name: "Proveedores", exact: true }).click();
  await expect(
    page.getByRole("heading", { name: "Proveedores", exact: true }),
  ).toBeVisible();
  await expect(
    page.getByRole("link", { name: "Clientes", exact: true }),
  ).toHaveCount(0);
  await page
    .getByRole("button", { name: "Nuevo proveedor", exact: true })
    .click();
  await expect(
    page.getByRole("heading", { name: "Nuevo proveedor", exact: true }),
  ).toBeVisible();
  await page
    .getByLabel("Nombre del proveedor", { exact: true })
    .fill("Proveedor Sintético Industrial");
  await page.getByLabel("País del proveedor", { exact: true }).fill("Honduras");
  await page
    .getByLabel("Sitio web del proveedor", { exact: true })
    .fill("https://supplier.example.test");
  await page
    .getByLabel("Términos de pago", { exact: true })
    .fill("Condiciones sintéticas por confirmar");
  await page
    .getByLabel("Categorías, una por línea", { exact: true })
    .fill("Ferretería\nEléctrico");
  await page
    .getByRole("button", { name: "Agregar contacto", exact: true })
    .click();
  await page
    .getByLabel("Nombre del contacto", { exact: true })
    .fill("Contacto de ventas sintético");
  await page
    .getByRole("button", { name: "Agregar producto", exact: true })
    .click();
  await page
    .getByLabel("Nombre del producto", { exact: true })
    .fill("Producto sintético");
  await page
    .getByLabel("Referencia del producto", { exact: true })
    .fill("SKU-1");
  await page
    .getByLabel("Cantidad mínima del producto", { exact: true })
    .fill("10.25");
  await page
    .getByRole("button", { name: "Guardar proveedor", exact: true })
    .click();
  await expect(
    page.getByRole("status").filter({ hasText: "Proveedor guardado." }),
  ).toBeVisible();
  await expect(
    page.getByLabel("Cantidad mínima del producto", { exact: true }),
  ).toHaveValue("10.2500");
  await page
    .getByLabel("Nombre del proveedor", { exact: true })
    .fill("Proveedor Editado Industrial");
  await page
    .getByLabel("Nueva nota", { exact: true })
    .fill("Nota histórica del proveedor");
  await page
    .getByRole("button", { name: "Registrar nota", exact: true })
    .click();
  await expect(
    page.getByText("Nota histórica del proveedor", { exact: true }),
  ).toBeVisible();
  await expect(
    page.getByLabel("Nombre del proveedor", { exact: true }),
  ).toHaveValue("Proveedor Editado Industrial");
  await page
    .getByLabel("URL de la fuente", { exact: true })
    .fill("https://catalog.example.test/product/SKU-1");
  await page
    .getByLabel("Fecha y hora de la fuente (Honduras)", { exact: true })
    .fill("2026-01-01T10:30");
  await page.getByLabel("Referencia observada", { exact: true }).fill("SKU-1");
  await page.getByLabel("Precio observado", { exact: true }).fill("12.345678");
  await page.getByLabel("Moneda observada", { exact: true }).fill("USD");
  await page
    .getByRole("button", { name: "Registrar observación", exact: true })
    .click();
  await expect(
    page.getByRole("status").filter({ hasText: "Observación registrada." }),
  ).toBeVisible();
  await expect(
    page.getByText("SKU-1 · 12.345678 USD", { exact: false }),
  ).toBeVisible();
  await expect(
    page.getByLabel("Nombre del proveedor", { exact: true }),
  ).toHaveValue("Proveedor Editado Industrial");
  const edited = page.waitForResponse(
    (r) =>
      /\/suppliers\/\d+$/.test(new URL(r.url()).pathname) &&
      r.request().method() === "PATCH" &&
      r.status() === 200,
  );
  await page
    .getByRole("button", { name: "Guardar proveedor", exact: true })
    .click();
  await edited;
  await expect(
    page.getByRole("status").filter({ hasText: "Proveedor guardado." }),
  ).toBeVisible();
  await page.reload();
  await page
    .getByLabel("Buscar proveedor", { exact: true })
    .fill("proveedor editado industrial");
  await page.getByLabel("Filtrar por país", { exact: true }).fill("honduras");
  await page.getByRole("button", { name: "Buscar", exact: true }).click();
  await page
    .getByRole("button", {
      name: "Abrir proveedor Proveedor Editado Industrial",
      exact: true,
    })
    .click();
  await expect(
    page.getByLabel("Nombre del contacto", { exact: true }),
  ).toHaveValue("Contacto de ventas sintético");
  await expect(
    page.getByLabel("Referencia del producto", { exact: true }),
  ).toHaveValue("SKU-1");
  await expect(
    page.getByText("SKU-1 · 12.345678 USD", { exact: false }),
  ).toBeVisible();
  await page
    .getByRole("button", { name: "Nuevo proveedor", exact: true })
    .click();
  await expect(
    page.getByRole("heading", { name: "Nuevo proveedor", exact: true }),
  ).toBeVisible();
  await page
    .getByLabel("Nombre del proveedor", { exact: true })
    .fill("PROVEEDOR EDITADO INDUSTRIAL");
  await page
    .getByRole("button", { name: "Guardar proveedor", exact: true })
    .click();
  await expect(
    page.getByRole("alert").filter({ hasText: "registro cambió" }),
  ).toBeVisible();
  await page
    .getByRole("button", {
      name: "Abrir proveedor Proveedor Editado Industrial",
      exact: true,
    })
    .click();
  await expect(
    page.getByRole("heading", { name: "Ficha del proveedor", exact: true }),
  ).toBeVisible();
  await page
    .getByRole("checkbox", { name: "Proveedor activo", exact: true })
    .uncheck();
  await page
    .getByRole("button", { name: "Guardar proveedor", exact: true })
    .click();
  await expect(
    page.getByRole("status").filter({ hasText: "Proveedor guardado." }),
  ).toBeVisible();
  await expect(
    page.getByText("0 proveedores encontrados", { exact: false }),
  ).toBeVisible();
  await page
    .getByRole("combobox", { name: "Estado del proveedor", exact: true })
    .selectOption("false");
  await page.getByRole("button", { name: "Buscar", exact: true }).click();
  await expect(
    page.getByRole("button", {
      name: "Abrir proveedor Proveedor Editado Industrial",
      exact: true,
    }),
  ).toBeVisible();
  await page.screenshot({ path: "/tmp/bastion-suppliers.png", fullPage: true });
  await page
    .getByRole("button", { name: "Cerrar sesión", exact: true })
    .click();
});
