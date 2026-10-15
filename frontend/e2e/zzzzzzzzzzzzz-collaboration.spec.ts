import { expect, test } from "@playwright/test";
import { totp } from "./helpers";

test("Colaboración: carga privada, cuarentena y papelera reversible", async ({
  page,
  browser,
}) => {
  await page.goto("/login");
  await page.getByLabel("Correo", { exact: true }).fill("collaboration@example.test");
  await page.getByLabel("Contraseña", { exact: true }).fill("synthetic browser password 42");
  await page.getByRole("button", { name: "Ingresar", exact: true }).click();
  await page.getByLabel("Código de autenticación o recuperación").fill(totp("JBSWY3DPEHPK3PXP"));
  await page.getByRole("button", { name: "Verificar", exact: true }).click();
  await expect(page.getByRole("link", { name: "Colaboración", exact: true })).toBeVisible();
  await page.getByRole("link", { name: "Colaboración", exact: true }).click();
  await expect(page.getByRole("heading", { name: "Colaboración", exact: true })).toBeVisible();

  await page.getByLabel("Nueva carpeta", { exact: true }).fill("Biblioteca sintética");
  const folderCreated = page.waitForResponse((r) => r.request().method() === "POST" && r.url().endsWith("/api/v1/collaboration/folders") && r.status() === 200);
  await page.getByRole("button", { name: "Crear carpeta", exact: true }).click();
  await folderCreated;
  const foldersResponse = await page.request.get(
    "http://localhost:8080/api/v1/collaboration/folders?scope=company&parent_id=root",
  );
  expect(foldersResponse.ok()).toBeTruthy();
  const folder = (await foldersResponse.json()).data.find(
    (item: { name: string }) => item.name === "Biblioteca sintética",
  );
  expect(folder?.id).toBeTruthy();
  await page.getByRole("button", { name: /Biblioteca sintética \(0\)/ }).click();

  await page.getByText("Confirmar identidad para vincular, editar, eliminar o moderar contenido", { exact: true }).click();
  await page.getByLabel("Contraseña para confirmar", { exact: true }).fill("synthetic browser password 42");

  await page.getByRole("button", { name: "Confirmar identidad", exact: true }).click();
  await expect(page.getByText("Identidad confirmada. Puede continuar con la operación.", { exact: true })).toBeVisible();

  await page.getByLabel("Título", { exact: true }).fill("Manual sintético");
  await page.getByLabel("Archivo", { exact: true }).setInputFiles({
    name: "manual.pdf",
    mimeType: "application/pdf",
    buffer: Buffer.from("%PDF-1.7\nsynthetic collaboration file"),
  });
  const linked = page.waitForResponse((r) => r.request().method() === "POST" && r.url().endsWith("/api/v1/collaboration") && r.status() === 200);
  await page.getByRole("button", { name: "Cargar y registrar en biblioteca", exact: true }).click();
  await linked;
  const entriesResponse = await page.request.get(
    "http://localhost:8080/api/v1/collaboration?scope=company&folder_id=" +
      folder.id + "&search=Manual&page=1&per_page=100",
  );
  expect(entriesResponse.ok()).toBeTruthy();
  const entry = (await entriesResponse.json()).data.find(
    (item: { title: string }) => item.title === "Manual sintético",
  );
  expect(entry?.id).toBeTruthy();
  expect(entry.document_id).toBeTruthy();
  await expect(page.getByText("manual.pdf", { exact: true })).toBeVisible();
  await expect(page.getByText(/En cuarentena/)).toBeVisible();

  const mentionableUsers = page.waitForResponse((r) => r.url().includes("/api/v1/collaboration/resources/collaboration/") && r.url().endsWith("/mentionable-users") && r.status() === 200);
  await page.getByRole("button", { name: "Conversación y actividad", exact: true }).click();
  const mentionableResponse = await page.request.get((await mentionableUsers).url());
  expect(mentionableResponse.ok()).toBeTruthy();
  const mentionable = await mentionableResponse.json();
  const reviewer = mentionable.data.find((user: { name: string }) => user.name === "Revisor Colaboración sintético");
  expect(reviewer?.id).toBeTruthy();
  const body = "<b>texto plano</b> comentario sintético";
  await page.getByLabel("Comentario", { exact: true }).fill(body);
  const recipients = page.getByRole("listbox", { name: "Destinatarios", exact: true });
  await expect(recipients).toBeVisible();
  const reviewerOption = recipients.getByRole("option", { name: reviewer.name, exact: true });
  await expect(reviewerOption).toBeVisible();
  await recipients.selectOption({ label: reviewer.name });
  await expect(recipients.locator("option:checked")).toHaveText(reviewer.name);
  const posted = page.waitForResponse((r) => r.request().method() === "POST" && r.url().endsWith("/collaboration/resources/collaboration/" + entry.id + "/comments") && r.status() === 200);
  await page.getByRole("button", { name: "Publicar comentario", exact: true }).click();
  await posted;
  await expect(page.getByText(body, { exact: true })).toBeVisible();
  await expect(page.locator('[aria-label="Conversación del recurso"] b')).toHaveCount(0);

  const reviewerContext = await browser.newContext();
  const reviewerPage = await reviewerContext.newPage();
  await reviewerPage.goto("http://localhost:4200/login");
  await reviewerPage.getByLabel("Correo", { exact: true }).fill("collaboration-reviewer@example.test");
  await reviewerPage.getByLabel("Contraseña", { exact: true }).fill("synthetic browser password 42");
  await reviewerPage.getByRole("button", { name: "Ingresar", exact: true }).click();
  await reviewerPage.getByLabel("Código de autenticación o recuperación").fill(totp("JBSWY3DPEHPK3PXP"));
  await reviewerPage.getByRole("button", { name: "Verificar", exact: true }).click();
  const notifications = reviewerPage.waitForResponse((r) => r.url().includes("/api/v1/collaboration/notifications?") && r.status() === 200);
  await reviewerPage.getByRole("link", { name: "Colaboración", exact: true }).click();
  const noticeResponse = await reviewerPage.request.get((await notifications).url());
  expect(noticeResponse.ok()).toBeTruthy();
  const noticePage = await noticeResponse.json();
  expect(noticePage.data).toHaveLength(1);
  await reviewerPage.getByRole("button", { name: "Abrir conversación" }).click();
  await expect(reviewerPage.getByText(body, { exact: true })).toBeVisible();
  const markedRead = reviewerPage.waitForResponse((r) => r.request().method() === "POST" && /\/api\/v1\/collaboration\/notifications\/[0-9]+\/read$/.test(r.url()) && r.status() === 200);
  await reviewerPage.getByRole("button", { name: "Marcar como leída", exact: true }).click();
  await markedRead;
  await reviewerContext.close();

  await page.goto("/collaboration/work");
  await expect(page.getByRole("heading", { name: "Mi trabajo", exact: true })).toBeVisible();
  await page.getByLabel("Título", { exact: true }).fill("Revisar archivo sintético");
  await page.getByLabel("Asignar a", { exact: true }).selectOption({
    label: "Operador Colaboración sintético",
  });
  const taskCreated = page.waitForResponse(
    (r) => r.request().method() === "POST" &&
      r.url().endsWith("/api/v1/collaboration/tasks") && r.status() === 200,
  );
  await page.getByRole("button", { name: "Crear tarea", exact: true }).click();
  await taskCreated;
  await expect(page.getByText("Revisar archivo sintético", { exact: true })).toBeVisible();
  await page.getByRole("button", {name:"Editar tarea",exact:true}).click();
  await page.getByLabel("Título", {exact:true}).fill("Revisar entrega sintética");
  const taskEdited = page.waitForResponse(r => r.request().method() === "PATCH" && /\/api\/v1\/collaboration\/tasks\/[0-9]+$/.test(r.url()) && r.status() === 200);
  await page.getByRole("button", {name:"Guardar tarea",exact:true}).click();
  await taskEdited;
  await expect(page.getByText("Revisar entrega sintética", {exact:true})).toBeVisible();
  const taskDone = page.waitForResponse(
    (r) => r.request().method() === "POST" &&
      /\/api\/v1\/collaboration\/tasks\/[0-9]+\/status$/.test(r.url()) &&
      r.status() === 200,
  );
  await page.getByRole("button", { name: "Completar", exact: true }).click();
  await taskDone;
  await expect(page.getByRole("button", { name: "Reabrir", exact: true })).toBeVisible();
  const taskReopened = page.waitForResponse(
    (r) => r.request().method() === "POST" &&
      /\/api\/v1\/collaboration\/tasks\/[0-9]+\/reopen$/.test(r.url()) &&
      r.status() === 200,
  );
  await page.getByRole("button", { name: "Reabrir", exact: true }).click();
  await taskReopened;
  await expect(page.getByRole("button", { name: "Completar", exact: true })).toBeVisible();

  const processResponse = await page.request.get("http://localhost:8080/api/v1/processes/1");
  expect(processResponse.ok()).toBeTruthy();
  const process = await processResponse.json();
  await page.goto("/collaboration/integrations");
  await expect(page.getByRole("heading", { name: "Integraciones y aprobaciones" })).toBeVisible();
  await expect(page.getByText("Las decisiones se realizan únicamente en el módulo propietario.")).toBeVisible();
  await page.getByLabel("ID de biblioteca", { exact: true }).fill(entry.id);
  await page.getByRole("button", { name: "Cargar vínculos", exact: true }).click();
  await expect(page.getByText("Archivo: Manual sintético", { exact: false })).toBeVisible();
  await page.getByLabel("ID", { exact: true }).fill("1");
  await page.getByLabel("Versión actual", { exact: true }).fill(String(process.version));
  const linkSaved = page.waitForResponse(
    (r) => r.request().method() === "PUT" &&
      r.url().endsWith("/api/v1/collaboration/" + entry.id + "/links") &&
      r.status() === 200,
  );
  await page.getByRole("button", { name: "Agregar y guardar", exact: true }).click();
  await linkSaved;
  await expect(page.getByRole("button", { name: "Quitar", exact: true })).toBeVisible();
  const linkRemoved = page.waitForResponse(
    (r) => r.request().method() === "PUT" &&
      r.url().endsWith("/api/v1/collaboration/" + entry.id + "/links") &&
      r.status() === 200,
  );
  await page.getByRole("button", { name: "Quitar", exact: true }).click();
  await linkRemoved;
  await expect(page.getByText("No hay vínculos cargados.", { exact: true })).toBeVisible();
  await page.goto("/collaboration");
  await page.getByRole("button", { name: /Biblioteca sintética/ }).click();

  const trashed = page.waitForResponse((r) => r.request().method() === "DELETE" && /\/api\/v1\/collaboration\/[0-9]+$/.test(r.url()) && r.status() === 200);
  await page.getByRole("button", { name: "Enviar a papelera", exact: true }).click();
  await trashed;
  await page.getByLabel("Papelera", { exact: true }).check();
  await expect(page.getByText("manual.pdf", { exact: true })).toBeVisible();

  const restored = page.waitForResponse((r) => r.request().method() === "POST" && /\/api\/v1\/collaboration\/[0-9]+\/restore$/.test(r.url()) && r.status() === 200);
  await page.getByRole("button", { name: "Restaurar", exact: true }).click();
  await restored;
  await page.getByLabel("Papelera", { exact: true }).uncheck();
  await page.getByRole("button", { name: /Biblioteca sintética/ }).click();
  await expect(page.getByText("manual.pdf", { exact: true })).toBeVisible();
  expect(folder.id).toBeTruthy();
});
