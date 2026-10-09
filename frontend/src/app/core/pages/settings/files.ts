import { Component, inject, signal } from "@angular/core";
import { Client } from "../../api/client";
interface Doc {
  id: string;
  filename: string;
  status: string;
  number: number;
  version: string;
}
@Component({
  selector: "bastion-files",
  template: `<section class="card">
    <h2>Archivos privados</h2>
    <p>
      PDF, PNG, JPEG o TXT, máximo 20 MB. La descarga permanece bloqueada hasta
      comprobar integridad y análisis de seguridad.
    </p>
    <p role="status">{{ message() }}</p>
    @if (api.has("core.documents.write")) {
      <label
        >Archivo<input
          type="file"
          accept=".pdf,.png,.jpg,.jpeg,.txt"
          (change)="select($event)"
          [disabled]="busy()" /></label
      ><button (click)="upload()" [disabled]="busy() || !file">
        Cargar y verificar
      </button>
    }
    <button (click)="load()">Actualizar</button>
    <ul>
      @for (d of documents(); track d.id) {
        <li>
          {{ d.filename }} · versión {{ d.number }} · {{ status(d.status) }}
          @if (d.status === "available") {
            <button (click)="download(d.id)">Descargar</button>
          }
        </li>
      }
    </ul>
  </section>`,
})
export class FilePanel {
  api = inject(Client);
  file: File | null = null;
  busy = signal(false);
  message = signal("");
  documents = signal<Doc[]>([]);
  constructor() {
    if (this.api.has("core.documents.read")) void this.load();
  }
  select(e: Event) {
    this.file = (e.target as HTMLInputElement).files?.[0] || null;
  }
  async load() {
    try {
      this.documents.set(
        (await this.api.get<{ data: Doc[] }>("/documents")).data,
      );
    } catch (e) {
      this.message.set(this.api.message(e));
    }
  }
  async upload() {
    if (!this.file) return;
    this.busy.set(true);
    try {
      const f = this.file;
      if (f.size > 20 * 1024 * 1024)
        throw new Error("El archivo excede 20 MB.");
      const buffer = await f.arrayBuffer();
      const hash = await crypto.subtle.digest("SHA-256", buffer);
      const checksum = Array.from(new Uint8Array(hash))
        .map((x) => x.toString(16).padStart(2, "0"))
        .join("");
      const r = await this.api.write<{
        id: string;
        upload_url: string;
        checksum_base64: string;
      }>(
        "POST",
        "/documents",
        {
          filename: f.name,
          media_type: f.type || "text/plain",
          size_bytes: f.size,
          checksum,
          version: "0",
        },
        crypto.randomUUID(),
      );
      const response = await fetch(r.upload_url, {
        method: "PUT",
        credentials: "omit",
        headers: {
          "Content-Type": f.type || "text/plain",
          "x-amz-checksum-sha256": r.checksum_base64,
        },
        body: buffer,
      });
      if (!response.ok) throw new Error("S3 no confirmó la carga.");
      await this.api.write("POST", "/documents/" + r.id + "/finalize");
      this.message.set(
        "Integridad verificada. Archivo en cuarentena, pendiente de análisis.",
      );
      this.file = null;
      await this.load();
    } catch (e) {
      this.message.set(this.api.message(e));
    } finally {
      this.busy.set(false);
    }
  }
  async download(id: string) {
    try {
      const r = await this.api.get<{ url: string }>(
        "/documents/" + id + "/download",
      );
      window.open(r.url, "_blank", "noopener,noreferrer");
    } catch (e) {
      this.message.set(this.api.message(e));
    }
  }
  status(s: string) {
    return (
      (
        {
          pending: "Carga pendiente",
          quarantined: "En cuarentena",
          available: "Disponible",
          rejected: "Rechazado",
          failed: "Fallido",
        } as Record<string, string>
      )[s] || s
    );
  }
}
