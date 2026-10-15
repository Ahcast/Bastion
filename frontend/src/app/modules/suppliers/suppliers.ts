import {NgTemplateOutlet} from "@angular/common";
import {WorkspaceSummary} from "../../shared/ui/workspace-summary";
import { Component, inject, signal, OnInit } from "@angular/core";
import { FormsModule } from "@angular/forms";
import { DatePipe } from "@angular/common";
import { Client } from "../../core/api/client";

export interface Contact {
  name: string;
  role: string;
  phone: string;
  email: string;
}
export interface Product {
  name: string;
  brand: string;
  part_number: string;
  description: string;
  lead_time: string;
  moq: string;
}
export interface Supplier {
  id: string;
  name: string;
  rtn: string;
  contact_name: string;
  phone: string;
  email: string;
  address: string;
  country: string;
  website: string;
  notes: string;
  payment_terms: string;
  lead_time: string;
  evaluation: number | null;
  reliability: string;
  response_hours: number | null;
  categories: string[];
  brands: string[];
  is_active: boolean;
  team_id: string;
  version: string;
  contacts: Contact[];
  products: Product[];
}
type SupplierSummary = Pick<
  Supplier,
  | "id"
  | "name"
  | "rtn"
  | "email"
  | "country"
  | "website"
  | "is_active"
  | "version"
>;
interface Page<T> {
  data: T[];
  total: number;
  page: number;
  per_page: number;
}
interface Note {
  id: string;
  body: string;
  user_name: string;
  created_at: string;
}
interface History {
  id: string;
  action: string;
  actor_name: string;
  created_at: string;
  details: { version?: string };
}
interface Document {
  id: string;
  filename: string;
  status: string;
  version: string;
}
interface Price {
  id: string;
  source_url: string;
  source_type: string;
  source_observed_at: string;
  observed_price: string;
  currency: string;
  part_number: string;
  availability: string;
  lead_time: string;
  moq: string;
  taxes: string;
  shipping: string;
  captured_at: string;
  user_name: string;
}
const emptyPrice = () => ({
  source_url: "",
  source_type: "web",
  observed_at: "",
  part_number: "",
  observed_price: "",
  currency: "",
  availability: "",
  lead_time: "",
  moq: "",
  taxes: "unknown",
  shipping: "unknown",
});
const empty = (): Supplier => ({
  id: "",
  name: "",
  rtn: "",
  contact_name: "",
  phone: "",
  email: "",
  address: "",
  country: "",
  website: "",
  notes: "",
  payment_terms: "",
  lead_time: "",
  evaluation: null,
  reliability: "unknown",
  response_hours: null,
  categories: [],
  brands: [],
  is_active: true,
  team_id: "",
  version: "0",
  contacts: [],
  products: [],
});

@Component({
  imports:[NgTemplateOutlet,WorkspaceSummary,FormsModule, DatePipe],
  templateUrl: "./suppliers.html",
})
export class Suppliers implements OnInit {
  api = inject(Client);
  rows = signal<SupplierSummary[]>([]);
  total = signal(0);
  loading = signal(false);
  busy = signal(false);
  error = signal("");
  message = signal("");
  editor = signal<Supplier | null>(null);
  teams = signal<{ id: string; name: string }[]>([]);
  notes = signal<Note[]>([]);
  history = signal<History[]>([]);
  documents = signal<Document[]>([]);
  personalFiles = signal<Document[]>([]);
  search = "";
  country = "";
  categoriesText = "";
  brandsText = "";
  prices = signal<Price[]>([]);
  priceTotal = signal(0);
  pricePage = 1;
  pricePart = "";
  price = emptyPrice();
  active = "true";
  page = 1;
  perPage = 25;
  note = "";
  notePage = 1;
  noteTotal = signal(0);
  historyPage = 1;
  historyTotal = signal(0);
  documentPage = 1;
  documentTotal = signal(0);
  personalPage = 1;
  selectedDocument = "";
  password = "";
  private pending = new Map<string, { body: string; key: string }>();
  async ngOnInit() {
    await this.load();
    try {
      this.teams.set(
        (
          await this.api.get<{ data: { id: string; name: string }[] }>(
            "/suppliers/teams",
          )
        ).data,
      );
    } catch (e) {
      this.error.set(this.api.message(e));
    }
  }
  async load(reset = false) {
    if (reset) this.page = 1;
    this.loading.set(true);
    this.error.set("");
    try {
      const q = new URLSearchParams({
        search: this.search,
        country: this.country,
        page: String(this.page),
        per_page: String(this.perPage),
      });
      if (this.active !== "") q.set("is_active", this.active);
      const p = await this.api.get<Page<SupplierSummary>>("/suppliers?" + q);
      this.rows.set(p.data);
      this.total.set(p.total);
    } catch (e) {
      this.error.set(this.api.message(e));
      this.rows.set([]);
      this.total.set(0);
    } finally {
      this.loading.set(false);
    }
  }
  create() {
    this.editor.set(empty());
    this.clearDetails();
    this.categoriesText = "";
    this.brandsText = "";
    this.error.set("");
    this.message.set("");
  }
  clearDetails() {
    this.prices.set([]);
    this.priceTotal.set(0);
    this.pricePage = 1;
    this.pricePart = "";
    this.price = emptyPrice();
    this.notes.set([]);
    this.history.set([]);
    this.documents.set([]);
    this.note = "";
    this.notePage = 1;
    this.historyPage = 1;
    this.documentPage = 1;
    this.selectedDocument = "";
    this.personalFiles.set([]);
    this.personalPage = 1;
    this.noteTotal.set(0);
    this.historyTotal.set(0);
    this.documentTotal.set(0);
  }
  async open(id: string) {
    this.busy.set(true);
    this.error.set("");
    try {
      this.setEditor(await this.api.get<Supplier>("/suppliers/" + id));
      this.clearDetails();
      await this.loadNotes();
      await this.loadHistory();
      await this.loadPrices();
    } catch (e) {
      this.editor.set(null);
      this.error.set(this.api.message(e));
    } finally {
      this.busy.set(false);
    }
  }
  key(operation: string, body: unknown) {
    const encoded = JSON.stringify(body),
      old = this.pending.get(operation);
    if (old?.body === encoded) return old.key;
    const key = crypto.randomUUID();
    this.pending.set(operation, { body: encoded, key });
    return key;
  }
  async save() {
    const c = this.editor();
    if (!c || this.busy()) return;
    this.busy.set(true);
    this.error.set("");
    this.message.set("");
    const id = c.id,
      body = {
        name: c.name,
        rtn: c.rtn,
        contact_name: c.contact_name,
        phone: c.phone,
        email: c.email,
        address: c.address,
        country: c.country,
        website: c.website,
        notes: c.notes,
        payment_terms: c.payment_terms,
        lead_time: c.lead_time,
        evaluation: c.evaluation,
        reliability: c.reliability,
        response_hours: c.response_hours,
        categories: this.lines(this.categoriesText),
        brands: this.lines(this.brandsText),
        is_active: c.is_active,
        team_id: c.team_id,
        version: c.version,
        contacts: c.contacts.map((x) => ({
          name: x.name,
          role: x.role,
          phone: x.phone,
          email: x.email,
        })),
        products: c.products.map((x) => ({
          name: x.name,
          brand: x.brand,
          part_number: x.part_number,
          description: x.description,
          lead_time: x.lead_time,
          moq: x.moq,
        })),
      };
    const path = "/suppliers" + (id ? "/" + id : "");
    try {
      const result = await this.api.write<{ id: string; version: string }>(
        id ? "PATCH" : "POST",
        path,
        body,
        this.key(path, body),
      );
      this.pending.delete(path);
      this.setEditor({ ...c, ...body, id: result.id, version: result.version });
      this.setEditor(await this.api.get<Supplier>("/suppliers/" + result.id));
      await this.load();
      await this.loadHistory();
      this.message.set("Proveedor guardado.");
    } catch (e) {
      this.error.set(this.api.message(e));
    } finally {
      this.busy.set(false);
    }
  }
  contact() {
    this.editor()?.contacts.push({ name: "", role: "", phone: "", email: "" });
  }
  product() {
    this.editor()?.products.push({
      name: "",
      brand: "",
      part_number: "",
      description: "",
      lead_time: "",
      moq: "",
    });
  }
  async reauthenticate() {
    if (this.busy()) return;
    this.busy.set(true);
    this.error.set("");
    try {
      await this.api.write("POST", "/auth/reauth", {
        password: this.password,

      });
      this.message.set("Identidad confirmada durante cinco minutos.");
    } catch (e) {
      this.error.set(this.api.message(e));
    } finally {
      this.password = "";
      this.busy.set(false);
    }
  }
  async loadNotes() {
    const id = this.editor()?.id;
    if (!id) return;
    try {
      const p = await this.api.get<Page<Note>>(
        "/suppliers/" + id + "/notes?page=" + this.notePage,
      );
      this.notes.set(p.data);
      this.noteTotal.set(p.total);
    } catch (e) {
      this.notes.set([]);
      this.error.set(this.api.message(e));
    }
  }
  async addNote() {
    const c = this.editor();
    if (!c?.id || this.busy()) return;
    this.busy.set(true);
    this.error.set("");
    const path = "/suppliers/" + c.id + "/notes",
      body = { body: this.note, version: c.version };
    try {
      const result = await this.api.write<{ id: string; version: string }>(
        "POST",
        path,
        body,
        this.key(path, body),
      );
      this.pending.delete(path);
      this.note = "";
      this.editor.set({ ...c, version: result.version });
      this.notePage = 1;
      await this.loadNotes();
      await this.loadHistory();
      this.message.set("Nota registrada.");
    } catch (e) {
      this.error.set(this.api.message(e));
    } finally {
      this.busy.set(false);
    }
  }
  async loadHistory() {
    const id = this.editor()?.id;
    if (!id) return;
    try {
      const p = await this.api.get<Page<History>>(
        "/suppliers/" + id + "/history?page=" + this.historyPage,
      );
      this.history.set(p.data);
      this.historyTotal.set(p.total);
    } catch (e) {
      this.history.set([]);
      this.error.set(this.api.message(e));
    }
  }
  historyLabel(action: string) {
    return (
      (
        {
          "suppliers.created": "Proveedor creado",
          "suppliers.updated": "Ficha actualizada",
          "suppliers.noted": "Nota registrada",
          "suppliers.document_linked": "Documento vinculado",
          "suppliers.price_observed": "Observación de precio registrada",
          "mcp.get_supplier_price_history":
            "Precios consultados mediante integración",
          "mcp.get_supplier": "Ficha consultada mediante integración",
          "mcp.resolve_supplier_exact":
            "Proveedor resuelto mediante integración",
        } as Record<string, string>
      )[action] || "Actividad de consulta"
    );
  }
  async loadDocuments() {
    const id = this.editor()?.id;
    if (!id) return;
    try {
      const p = await this.api.get<Page<Document>>(
        "/suppliers/" + id + "/documents?page=" + this.documentPage,
      );
      this.documents.set(p.data);
      this.documentTotal.set(p.total);
    } catch (e) {
      this.documents.set([]);
      this.error.set(this.api.message(e));
    }
  }
  async loadPersonalFiles() {
    try {
      this.personalFiles.set(
        (
          await this.api.get<{ data: Document[] }>(
            "/documents?page=" + this.personalPage,
          )
        ).data,
      );
    } catch (e) {
      this.error.set(this.api.message(e));
    }
  }
  async linkDocument() {
    const c = this.editor(),
      doc = this.personalFiles().find((d) => d.id === this.selectedDocument);
    if (!c?.id || !doc || this.busy()) return;
    this.busy.set(true);
    this.error.set("");
    const path = "/suppliers/" + c.id + "/documents",
      body = { document_id: doc.id, version: doc.version };
    try {
      await this.api.write("POST", path, body, this.key(path, body));
      this.pending.delete(path);
      this.selectedDocument = "";
      await this.loadDocuments();
      await this.loadPersonalFiles();
      await this.loadHistory();
      this.message.set("Documento vinculado.");
    } catch (e) {
      this.error.set(this.api.message(e));
    } finally {
      this.busy.set(false);
    }
  }
  async download(id: string) {
    try {
      const value = await this.api.get<{ url: string }>(
        "/documents/" + id + "/download",
      );
      const url = new URL(value.url);
      if (url.protocol !== "https:")
        throw new Error("Enlace de descarga inválido.");
      window.open(url.href, "_blank", "noopener,noreferrer");
    } catch (e) {
      this.error.set(this.api.message(e));
    }
  }
  state(status: string) {
    return (
      (
        {
          pending: "Carga pendiente",
          quarantined: "En cuarentena",
          available: "Disponible",
          rejected: "Rechazado",
        } as Record<string, string>
      )[status] || "No disponible"
    );
  }
  lines(value: string) {
    return value
      .split("\n")
      .map((x) => x.trim())
      .filter(Boolean);
  }
  setEditor(value: Supplier) {
    this.editor.set(value);
    this.categoriesText = value.categories.join("\n");
    this.brandsText = value.brands.join("\n");
  }
  async loadPrices(reset = false) {
    const id = this.editor()?.id;
    if (!id) return;
    if (reset) this.pricePage = 1;
    try {
      const q = new URLSearchParams({
        page: String(this.pricePage),
        part_number: this.pricePart,
      });
      const p = await this.api.get<Page<Price>>(
        "/suppliers/" + id + "/price-history?" + q,
      );
      this.prices.set(p.data);
      this.priceTotal.set(p.total);
    } catch (e) {
      this.prices.set([]);
      this.error.set(this.api.message(e));
    }
  }
  async recordPrice() {
    const c = this.editor();
    if (!c?.id || this.busy()) return;
    this.busy.set(true);
    this.error.set("");
    const path = "/suppliers/" + c.id + "/price-history";
    try {
      const observed = new Date(this.price.observed_at + "-06:00");
      if (Number.isNaN(observed.getTime()))
        throw new Error("Indique una fecha y hora válidas para la fuente.");
      const source_observed_at = observed.toISOString();
      const { observed_at, ...value } = this.price;
      const body = { ...value, source_observed_at, version: c.version };
      const result = await this.api.write<{ id: string; version: string }>(
        "POST",
        path,
        body,
        this.key(path, body),
      );
      this.pending.delete(path);
      this.price = emptyPrice();
      this.editor.set({ ...c, version: result.version });
      this.pricePage = 1;
      await this.loadPrices();
      await this.loadHistory();
      this.message.set("Observación registrada.");
    } catch (e) {
      this.error.set(this.api.message(e));
    } finally {
      this.busy.set(false);
    }
  }
  inclusion(value: string) {
    return (
      (
        {
          unknown: "Sin confirmar",
          included: "Incluido",
          excluded: "No incluido",
        } as Record<string, string>
      )[value] || "Sin confirmar"
    );
  }

 inlineId(){return this.editor()?.id||''}
 inlineInPage(){return this.rows().some(row=>row.id===this.inlineId())}
 toggleInline(id:string,event:Event){const target=event.target as HTMLElement;if(target.closest('button,a,input,select,textarea'))return;if(event.type==='keydown'){if(event.target!==event.currentTarget)return;event.preventDefault()}if(this.busy())return;if(this.inlineId()===id){this.editor.set(null)}else{void this.open(id)}}
}
