import { TestBed } from "@angular/core/testing";
import { describe, expect, it, vi } from "vitest";
import { Client } from "../../core/api/client";
import { Collaboration, type Entry } from "./collaboration";

function api() {
  return {
    profile: () => ({ user: { id: "101" } }),
    has: vi.fn(() => true),
    get: vi.fn(async (path: string) => {
      if (path.startsWith("/collaboration/folders?")) return { data: [] };
      if (path === "/collaboration/teams") return { data: [] };
      return { data: [], total: 0, page: 1, per_page: 25 };
    }),
    write: vi.fn(
      async (_method: string, path: string, _body?: unknown, _key?: string) => {
      if (path === "/documents" || path === "/documents/201")
        return {
          id: "201",
          version: "1",
          upload_url: "https://files.example.test/signed",
          checksum_base64: "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=",
        };
      return {};
      },
    ),
    message: vi.fn(() => "No se pudo completar la operación."),
  };
}

function fixture(client: ReturnType<typeof api>) {
  TestBed.configureTestingModule({
    providers: [{ provide: Client, useValue: client }],
  });
  return TestBed.runInInjectionContext(() => new Collaboration());
}

describe("Colaboración", () => {
  it("aplica empresa/persona y carpeta raíz a cada consulta", async () => {
    const client = api();
    const component = fixture(client);
    await component.load();
    const company = new URL(
      "https://local.test" + client.get.mock.calls.at(-2)?.[0],
    );
    expect(company.searchParams.get("scope")).toBe("company");
    expect(company.searchParams.get("folder_id")).toBe("root");
    expect(company.searchParams.has("person_id")).toBe(false);

    component.scope = "person";
    await component.changeScope();
    const personal = new URL(
      "https://local.test" + client.get.mock.calls.at(-2)?.[0],
    );
    expect(personal.searchParams.get("scope")).toBe("person");
    expect(personal.searchParams.get("person_id")).toBe("101");
  });

  it("sube por Core.Files y vincula solo metadata del archivo", async () => {
    const client = api();
    const component = fixture(client);
    const digest = vi
      .spyOn(globalThis.crypto.subtle, "digest")
      .mockResolvedValue(new ArrayBuffer(32));
    const originalFetch = globalThis.fetch;
    globalThis.fetch = vi.fn(async () => ({ ok: true }) as Response);
    const file = {
      name: "manual.pdf",
      size: 6,
      arrayBuffer: async () => new TextEncoder().encode("%PDF-x").buffer,
    } as File;
    component.file = file;
    component.title = "Manual";
    await component.upload();
    expect(digest).toHaveBeenCalled();
    expect(client.write.mock.calls.map((call) => call[1])).toEqual([
      "/documents",
      "/documents/201/finalize",
      "/collaboration",
    ]);
    const bind = client.write.mock.calls[2][2] as Record<string, unknown>;
    expect(bind).toMatchObject({
      document_id: "201",
      document_version: "1",
      scope: "company",
      folder_id: "",
      title: "Manual",
    });
    expect(JSON.stringify(bind)).not.toContain("object_key");
    expect(component.message()).toContain("Core.Files");
    globalThis.fetch = originalFetch;
    digest.mockRestore();
  });

  it("agrega versiones a Core.Files sin crear un segundo vínculo", async () => {
    const client = api();
    const component = fixture(client);
    const digest = vi
      .spyOn(globalThis.crypto.subtle, "digest")
      .mockResolvedValue(new ArrayBuffer(32));
    const originalFetch = globalThis.fetch;
    globalThis.fetch = vi.fn(async () => ({ ok: true }) as Response);
    const entry = {
      id: "301",
      document_id: "201",
      document_version: "2",
      title: "Manual",
    } as Entry;
    component.versionFile = {
      name: "manual-v2.pdf",
      size: 7,
      arrayBuffer: async () => new TextEncoder().encode("%PDF-v2").buffer,
    } as File;
    await component.uploadVersion(entry);
    expect(client.write.mock.calls.map((call) => call[1])).toEqual([
      "/documents/201",
      "/documents/201/finalize",
    ]);
    expect(client.write.mock.calls[0][2]).toMatchObject({
      filename: "manual-v2.pdf",
      version: "2",
    });
    expect(component.message()).toContain("Nueva versión");
    globalThis.fetch = originalFetch;
    digest.mockRestore();
  });

  it("publica texto plano con menciones y carga hilo/actividad tras la respuesta", async () => {
    const client = api();
    const component = fixture(client);
    const entry = {
      id: "301",
      document_id: "201",
      scope: "company",
      title: "Manual",
      version: "1",
      document_version: "2",
      file_version: 1,
      filename: "manual.pdf",
      media_type: "application/pdf",
      size_bytes: 20,
      status: "available",
      updated_at: "2026-10-12T12:00:00Z",
      deleted: false,
    } as Entry;
    await component.openConversation(entry);
    component.commentBody = "<script>texto literal</script>";
    component.mentionUserIDs = ["9", "7", "9"];
    await component.postComment();

    const post = client.write.mock.calls.find(
      (call) =>
        call[0] === "POST" &&
        call[1] === "/collaboration/resources/collaboration/301/comments",
    );
    expect(post?.[2]).toEqual({
      body: "<script>texto literal</script>",
      mention_user_ids: ["7", "9"],
    });
    expect(post?.[3]).toBeTruthy();
    expect(component.conversationEntry()?.id).toBe("301");
    expect(component.commentBody).toBe("");
    expect(client.get).toHaveBeenCalledWith(
      "/collaboration/resources/collaboration/301/comments?page=1&per_page=50",
    );
    expect(client.get).toHaveBeenCalledWith(
      "/collaboration/resources/collaboration/301/activity?page=1&per_page=50",
    );
  });
});
