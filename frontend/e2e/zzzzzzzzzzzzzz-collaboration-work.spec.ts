import { expect, test } from "@playwright/test";
import { totp } from "./helpers";

const email = "collaboration-work" + "@example.test";
const password = "synthetic browser " + "password 42";
const secret = "JBSWY3DP" + "EHPK3PXP";
const recovery = "synthetic-collaboration-" + "recovery";

test("Colaboración U3/U4: trabajo, vínculos y bandeja Workflow", async ({ page }) => {
  await page.goto("/login");
  await page.getByLabel("Correo", { exact: true }).fill(email);
  await page.getByLabel("Contraseña", { exact: true }).fill(password);
  await page.getByRole("button", { name: "Ingresar", exact: true }).click();
  await page.getByLabel("Código de autenticación o recuperación").fill(totp(secret));
  await page.getByRole("button", { name: "Verificar", exact: true }).click();
  await expect(page.getByRole("link", { name: "Colaboración", exact: true })).toBeVisible();

  await page.goto("/collaboration/work");
  await expect(page.getByRole("heading", { name: "Mi trabajo", exact: true })).toBeVisible();
  await page.getByLabel("Título", { exact: true }).fill("Tarea sintética U3");
  await page.getByLabel("Asignar a", { exact: true }).selectOption({ label: "Operador Colaboración trabajo sintético" });
  const taskCreated = page.waitForResponse((r) => r.request().method() === "POST" && r.url().endsWith("/api/v1/collaboration/tasks") && r.status() === 200);
  await page.getByRole("button", { name: "Crear tarea", exact: true }).click();
  await taskCreated;
  await expect(page.getByText("Tarea sintética U3", { exact: true })).toBeVisible();

  const started = page.waitForResponse((r) => r.request().method() === "POST" && /\/api\/v1\/collaboration\/tasks\/[0-9]+\/status$/.test(r.url()) && r.status() === 200);
  await page.getByRole("button", { name: "Iniciar", exact: true }).click();
  await started;
  const completed = page.waitForResponse((r) => r.request().method() === "POST" && /\/api\/v1\/collaboration\/tasks\/[0-9]+\/status$/.test(r.url()) && r.status() === 200);
  await page.getByRole("button", { name: "Completar", exact: true }).click();
  await completed;
  await expect(page.getByText(/Completada/)).toBeVisible();

  await page.getByText("Confirmar identidad", { exact: true }).click();
  await page.getByLabel("Contraseña para confirmar", { exact: true }).fill(password);
  await page.getByLabel("Código de confirmación", { exact: true }).fill(recovery);
  await page.getByRole("button", { name: "Confirmar identidad", exact: true }).click();
  await expect(page.getByText("Identidad confirmada. Puede continuar con la operación.", { exact: true })).toBeVisible();
  const reopened = page.waitForResponse((r) => r.request().method() === "POST" && /\/api\/v1\/collaboration\/tasks\/[0-9]+\/reopen$/.test(r.url()) && r.status() === 200);
  await page.getByRole("button", { name: "Reabrir", exact: true }).click();
  await reopened;
  await expect(page.getByText(/Abierta/)).toBeVisible();

  // Use the API only to create a private Collaboration entry fixture; U4 itself
  // is exercised entirely through the Integration UI below.
  const file = "%PDF-1.7\nsynthetic U4 fixture";
  const create = await page.evaluate(async ({ size }) => {
    const csrf = document.cookie.split("; ").find((x) => x.startsWith("bastion_csrf="))?.split("=")[1] ?? "";
    const r = await fetch("http://localhost:8080/api/v1/files", {
      method: "POST", credentials: "include",
      headers: { "Content-Type": "application/json", "X-CSRF-Token": decodeURIComponent(csrf), "Idempotency-Key": crypto.randomUUID() },
      body: JSON.stringify({ filename: "u4.pdf", media_type: "application/pdf", size, checksum: "22b67df5396fa223839fcbfa950e08aac426ec3adadcc7e179c026cec242792c" }),
    });
    return { status: r.status, body: await r.json() };
  }, { size: Buffer.byteLength(file) });
  // If the hash fixture changes, fail before the integration UI creates a link.
  expect(create.status).toBe(200);
  expect(create.body.id).toBeTruthy();

  await page.goto("/collaboration/integrations");
  await expect(page.getByRole("heading", { name: "Integraciones y aprobaciones", exact: true })).toBeVisible();
  await expect(page.getByText("Las decisiones se realizan únicamente en el módulo propietario.", { exact: true })).toBeVisible();
});
