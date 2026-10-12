import { expect, test } from "@playwright/test";
import { totp } from "./helpers";

test("Colaboración: carga privada, cuarentena y papelera reversible", async ({
  page,
  browser,
}) => {
  await page.goto("/login");
  await page
    .getByLabel("Correo", { exact: true })
    .fill("collaboration@example.test");
  await page
    .getByLabel("Contraseña", { exact: true })
    .fill("synthetic browser password 42");
  await page.getByRole("button", { name: "Ingresar", exact: true }).click();
  await page
    .getByLabel("Código de autenticación o recuperación")
    .fill(totp("JBSWY3DPEHPK3PXP"));
  await page.getByRole("button", { name: "Verificar", exact: true }).click();
  await expect(
    page.getByRole("link", { name: "Colaboración", exact: true }),
  ).toBeVisible();
  await page.getByRole("link", { name: "Colaboración", exact: true }).click();
  await expect(
    page.getByRole("heading", { name: "Colaboración", exact: true }),
  ).toBeVisible();

  await page
    .getByLabel("Nueva carpeta", { exact: true })
    .fill("Biblioteca sintética");
  const folderCreated = page.waitForResponse(
    (r) =>
      r.request().method() === "POST" &&
      r.url().endsWith("/api/v1/collaboration/folders") &&
      r.status() === 200,
  );
  await page
    .getByRole("button", { name: "Crear carpeta", exact: true })
    .click();
  const folder = await (await folderCreated).json();
  await page
    .getByRole("button", { name: /Biblioteca sintética \(0\)/ })
    .click();

  await page
    .getByText(
      "Confirmar identidad para vincular, editar, eliminar o moderar contenido",
      { exact: true },
    )
    .click();
  await page
    .getByLabel("Contraseña para confirmar", { exact: true })
    .fill("synthetic browser password 42");
  await page
    .getByLabel("Código de confirmación", { exact: true })
    .fill("synthetic-collaboration-recovery");
  await page
    .getByRole("button", { name: "Confirmar identidad", exact: true })
    .click();
  await expect(
    page.getByText("Identidad confirmada. Puede continuar con la operación.", {
      exact: true,
    }),
  ).toBeVisible();

  await page.getByLabel("Título", { exact: true }).fill("Manual sintético");
  await page.getByLabel("Archivo", { exact: true }).setInputFiles({
    name: "manual.pdf",
    mimeType: "application/pdf",
    buffer: Buffer.from("%PDF-1.7\nsynthetic collaboration file"),
  });
  const linked = page
    .waitForResponse(
      (r) =>
        r.request().method() === "POST" &&
        r.url().endsWith("/api/v1/collaboration") &&
        r.status() === 200,
    )
    .then((r) => r.json());
  await page
    .getByRole("button", {
      name: "Cargar y registrar en biblioteca",
      exact: true,
    })
    .click();
  const entry = await linked;
  expect(entry.document_id).toBeTruthy();
  await expect(page.getByText("manual.pdf", { exact: true })).toBeVisible();
  await expect(page.getByText(/En cuarentena/)).toBeVisible();

  const mentionableUsers = page.waitForResponse(
    (r) =>
      r.url().includes("/api/v1/collaboration/resources/collaboration/") &&
      r.url().endsWith("/mentionable-users") &&
      r.status() === 200,
  );
  await page
    .getByRole("button", { name: "Conversación y actividad", exact: true })
    .click();
  const mentionable = await (await mentionableUsers).json();
  const reviewer = mentionable.data.find(
    (user: { name: string }) => user.name === "Revisor Colaboración sintético",
  );
  expect(reviewer?.id).toBeTruthy();
  const body = "<script>alert('xss')</script> comentario sintético";
  await page.getByLabel("Comentario", { exact: true }).fill(body);
  await page
    .getByLabel("Destinatarios", { exact: true })
    .selectOption(reviewer.id);
  const posted = page.waitForResponse(
    (r) =>
      r.request().method() === "POST" &&
      r.url().endsWith("/collaboration/resources/collaboration/" + entry.id + "/comments") &&
      r.status() === 200,
  );
  await page
    .getByRole("button", { name: "Publicar comentario", exact: true })
    .click();
  await posted;
  await expect(page.getByText(body, { exact: true })).toBeVisible();
  await expect(
    page.locator('[aria-label="Conversación del recurso"] script'),
  ).toHaveCount(0);

  const reviewerContext = await browser.newContext();
  const reviewerPage = await reviewerContext.newPage();
  await reviewerPage.goto("http://localhost:4200/login");
  await reviewerPage
    .getByLabel("Correo", { exact: true })
    .fill("collaboration-reviewer@example.test");
  await reviewerPage
    .getByLabel("Contraseña", { exact: true })
    .fill("synthetic browser password 42");
  await reviewerPage
    .getByRole("button", { name: "Ingresar", exact: true })
    .click();
  await reviewerPage
    .getByLabel("Código de autenticación o recuperación")
    .fill(totp("JBSWY3DPEHPK3PXP"));
  await reviewerPage
    .getByRole("button", { name: "Verificar", exact: true })
    .click();
  const notifications = reviewerPage.waitForResponse(
    (r) =>
      r.url().includes("/api/v1/collaboration/notifications?") &&
      r.status() === 200,
  );
  await reviewerPage
    .getByRole("link", { name: "Colaboración", exact: true })
    .click();
  const noticePage = await (await notifications).json();
  expect(noticePage.data).toHaveLength(1);
  await reviewerPage.getByRole("button", { name: "Abrir conversación" }).click();
  await expect(reviewerPage.getByText(body, { exact: true })).toBeVisible();
  const markedRead = reviewerPage.waitForResponse(
    (r) =>
      r.request().method() === "POST" &&
      /\/api\/v1\/collaboration\/notifications\/[0-9]+\/read$/.test(r.url()) &&
      r.status() === 200,
  );
  await reviewerPage
    .getByRole("button", { name: "Marcar como leída", exact: true })
    .click();
  await markedRead;
  await reviewerContext.close();

  const trashed = page.waitForResponse(
    (r) =>
      r.request().method() === "DELETE" &&
      /\/api\/v1\/collaboration\/[0-9]+$/.test(r.url()) &&
      r.status() === 200,
  );
  await page
    .getByRole("button", { name: "Enviar a papelera", exact: true })
    .click();
  await trashed;
  await page.getByLabel("Papelera", { exact: true }).check();
  await expect(page.getByText("manual.pdf", { exact: true })).toBeVisible();

  const restored = page.waitForResponse(
    (r) =>
      r.request().method() === "POST" &&
      /\/api\/v1\/collaboration\/[0-9]+\/restore$/.test(r.url()) &&
      r.status() === 200,
  );
  await page.getByRole("button", { name: "Restaurar", exact: true }).click();
  await restored;
  await page.getByLabel("Papelera", { exact: true }).uncheck();
  await page.getByRole("button", { name: /Biblioteca sintética/ }).click();
  await expect(page.getByText("manual.pdf", { exact: true })).toBeVisible();
  expect(folder.id).toBeTruthy();
});
