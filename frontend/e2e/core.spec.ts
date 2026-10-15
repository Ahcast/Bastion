import { test, expect } from "@playwright/test";
import { createHmac } from "node:crypto";
function totp(secret: string) {
  const alphabet = "ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
  let bits = "";
  for (const c of secret.toUpperCase().replace(/=+$/, ""))
    bits += alphabet.indexOf(c).toString(2).padStart(5, "0");
  const bytes = [];
  for (let i = 0; i + 8 <= bits.length; i += 8)
    bytes.push(parseInt(bits.slice(i, i + 8), 2));
  const counter = Buffer.alloc(8);
  counter.writeBigUInt64BE(BigInt(Math.floor(Date.now() / 30000)));
  const digest = createHmac("sha1", Buffer.from(bytes))
    .update(counter)
    .digest();
  const offset = digest[digest.length - 1] & 15;
  return ((digest.readUInt32BE(offset) & 0x7fffffff) % 1000000) + "";
}
test("real two-origin login, MFA, administration and logout", async ({
  page,
}) => {
  await page.goto("/login");
  await page.getByLabel("Correo", { exact: true }).fill("browser@example.test");
  await page
    .getByLabel("Contraseña", { exact: true })
    .fill("synthetic browser password 42");
  await page.getByRole("button", { name: "Ingresar", exact: true }).click();
  await expect(
    page.getByRole("heading", { name: "Verificación en dos pasos" }),
  ).toBeVisible();
  const qr = page.getByRole("img", {
    name: "Código QR para vincular Bastion con su autenticador",
  });
  await expect(qr).toBeVisible();
  await expect
    .poll(() => qr.evaluate((img: HTMLImageElement) => img.naturalWidth))
    .toBe(256);
  await page.getByText("Vincular manualmente", { exact: true }).click();
  const uri = await page.getByLabel("URI para su autenticador").inputValue();
  const secret = new URL(uri).searchParams.get("secret")!;
  await page
    .getByLabel("Código de autenticación o recuperación")
    .fill(totp(secret).padStart(6, "0"));
  await page.getByRole("button", { name: "Verificar", exact: true }).click();
  await expect(
    page.getByRole("heading", { name: "Guarde estos códigos de recuperación" }),
  ).toBeVisible();
  const recovery = (await page.locator("pre").innerText())
    .trim()
    .split(/\s+/)[0];
  await page.getByRole("button", { name: "Continuar", exact: true }).click();
  await expect(
    page.getByRole("heading", { name: "Inicio", exact: true }),
  ).toBeVisible();
  // Removing only the short session cookie simulates re-entry after session expiry.
  // The remembered verification is independent; no second OTP challenge appears.
  await page.context().clearCookies({ name: "bastion_dev_session" });
  await page.goto("/login");
  await page.getByLabel("Correo", { exact: true }).fill("browser@example.test");
  await page
    .getByLabel("Contraseña", { exact: true })
    .fill("synthetic browser password 42");
  await page.getByRole("button", { name: "Ingresar", exact: true }).click();
  await expect(
    page.getByRole("heading", { name: "Inicio", exact: true }),
  ).toBeVisible();
  await expect(
    page.getByLabel("Código de autenticación o recuperación"),
  ).toHaveCount(0);
  const remembered = (await page.context().cookies()).find(
    (c) => c.name === "bastion_dev_mfa",
  )!;
  expect(remembered.httpOnly).toBe(true);
  expect(await page.evaluate(() => document.cookie)).not.toContain(
    "bastion_dev_mfa",
  );
  await page.getByRole("link", { name: "Configuración", exact: true }).click();
  await page
    .getByLabel("Nombre de empresa", { exact: true })
    .fill("Empresa sintética de prueba");
  await page
    .getByRole("button", { name: "Guardar empresa", exact: true })
    .click();
  await expect(
    page.getByRole("status").filter({ hasText: "Operación completada." }),
  ).toBeVisible();
  await page.reload();
  await page
    .getByRole("heading", { name: "Empresa y SMTP", exact: true })
    .locator("..")
    .getByRole("button", { name: "Actualizar", exact: true })
    .click();
  await expect(
    page.getByLabel("Nombre de empresa", { exact: true }),
  ).toHaveValue("Empresa sintética de prueba");
  const storage = await page.evaluate(() => ({
    local: Object.keys(localStorage),
    session: Object.keys(sessionStorage),
    cookie: document.cookie,
  }));
  expect(storage.local).toEqual([]);
  expect(storage.session).toEqual([]);
  expect(storage.cookie).not.toContain("bastion_dev_session");
  await page.screenshot({
    path: "/tmp/bastion-core-settings.png",
    fullPage: true,
  });
  await page.getByRole("button", { name: "Cerrar sesión" }).click();
  await expect(
    page.getByRole("button", { name: "Ingresar", exact: true }),
  ).toBeVisible();
  expect(
    (await page.context().cookies()).find((c) => c.name === "bastion_dev_mfa"),
  ).toBeUndefined();
  await page.getByLabel("Correo", { exact: true }).fill("browser@example.test");
  await page
    .getByLabel("Contraseña", { exact: true })
    .fill("synthetic browser password 42");
  await page.getByRole("button", { name: "Ingresar", exact: true }).click();
  await expect(
    page.getByRole("heading", { name: "Verificación en dos pasos" }),
  ).toBeVisible();
  await page
    .getByLabel("Código de autenticación o recuperación")
    .fill(recovery);
  await page.getByRole("button", { name: "Verificar", exact: true }).click();
  await expect(
    page.getByRole("heading", { name: "Inicio", exact: true }),
  ).toBeVisible();
  await page.getByRole("link", { name: "Configuración", exact: true }).click();
  const devices = page.locator("bastion-devices");
  await devices
    .getByRole("button", { name: "Consultar equipos reconocidos", exact: true })
    .click();
  await expect(
    devices.getByText("· Este navegador", { exact: true }),
  ).toBeVisible();
  await devices
    .getByLabel("Contraseña para confirmar", { exact: true })
    .fill("synthetic browser password 42");
  await devices
    .getByRole("button", { name: "Confirmar identidad", exact: true })
    .click();
  await expect(
    devices.getByText(
      "Identidad confirmada. Puede continuar con la operación.",
      { exact: true },
    ),
  ).toBeVisible();
  await devices.getByRole("button", { name: /^Revocar / }).click();
  await expect(
    page.getByRole("button", { name: "Ingresar", exact: true }),
  ).toBeVisible();
  expect(
    (await page.context().cookies()).find((c) => c.name === "bastion_dev_mfa"),
  ).toBeUndefined();
});
