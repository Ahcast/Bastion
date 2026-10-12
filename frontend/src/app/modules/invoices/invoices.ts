import {ActivatedRoute,RouterLink} from "@angular/router";
import {InvoicePayments,PaymentResult,collectionLabel} from "./invoice-payments";
import {InvoiceEmission,InvoiceEmissionResult} from "./invoice-emission";
import {InvoiceCancellation,CancellationResult} from "./invoice-cancellation";
import {InvoiceFiscalSettings} from "./fiscal-settings";
import { Component, OnInit, inject, signal } from "@angular/core";
import { FormsModule } from "@angular/forms";
import { Reauthentication } from "../../core/security/reauthentication";
import { Client } from "../../core/api/client";

export interface InvoiceItem {
  id?: string;
  description: string;
  unit: string;
  quantity: string;
  unit_price: string;
  unit_cost: string;
  discount_type: "amount" | "percent";
  discount: string;
  tax: "isv15" | "isv18" | "exempt" | "exonerated";
}
export interface DraftInput {
  customer: {
    id: string;
    name: string;
    rtn: string;
    contact: string;
    phone: string;
    email: string;
    address: string;
  };
  invoice_date: string;
  payment_term_type: "contado" | "credito";
  credit_days: number | null;
  currency: "HNL" | "USD";
  public_notes: string;
  general_discount_type: "amount" | "percent";
  general_discount: string;
  exoneration_reference: string;
  additional_expenses: string;
  team_id: string;
  version: string;
  items: InvoiceItem[];
}
export interface Financials {
  total: string;
  profit: string;
  investment_total: string;
  products_investment: string;
  additional_expenses: string;
  base_15: string;
  base_18: string;
  exempt: string;
  exonerated: string;
  isv_15: string;
  isv_18: string;
}
export interface Invoice extends Omit<DraftInput, "additional_expenses"> {
  id: string;
  status: string;
 commercial_status?:string|null;
  invoice_number: string | null;
  sar_snapshot_json: {configuration:{cai:string;currency:string};configuration_version:string} | null;
  customer_version: string;
 source_quote_id?:string|null;
 source_purchase_order_id?:string|null;
 source_purchase_order_version?:string|null;
 source_quote_version?:string|null;
  issued_at:string|null;
  issued_by:string|null;
  due_date:string|null;
  company_snapshot_json:{name:string;tax_id:string;address:string}|null;
  company_version:string|null;
  calculation: Financials;
  paid_total: string;
  balance: string;
  created_by: string;
}
interface Summary {
  id: string;
  invoice_number: string | null;
  customer: { id: string; name: string };
  invoice_date: string;
  currency: string;
  total: string;
  status: string;
 commercial_status?:string|null;
  version: string;
}
const businessDate = () => {
  const parts = new Intl.DateTimeFormat("en", {
    timeZone: "America/Tegucigalpa",
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
  }).formatToParts(new Date());
  const field = (name: string) => parts.find((p) => p.type === name)!.value;
  return [field("year"), field("month"), field("day")].join("-");
};
export const emptyInvoice = (): DraftInput => ({
  customer: {
    id: "",
    name: "",
    rtn: "",
    contact: "",
    phone: "",
    email: "",
    address: "",
  },
  invoice_date: businessDate(),
  payment_term_type: "contado",
  credit_days: null,
  currency: "HNL",
  public_notes: "",
  general_discount_type: "amount",
  general_discount: "0",
  exoneration_reference: "",
  additional_expenses: "0",
  team_id: "",
  version: "0",
  items: [
    {
      description: "",
      unit: "u",
      quantity: "1",
      unit_price: "0",
      unit_cost: "0",
      discount_type: "amount",
      discount: "0",
      tax: "isv15",
    },
  ],
});
@Component({
  selector: "bastion-invoices",
  imports: [RouterLink,InvoicePayments,InvoiceCancellation,FormsModule, Reauthentication, InvoiceFiscalSettings, InvoiceEmission],
  templateUrl: "./invoices.html",
})
export class Invoices implements OnInit {
 route=inject(ActivatedRoute,{optional:true});
 collectionLabel=collectionLabel;
 commercialFilter="";
  api = inject(Client);
  emissionBusy=signal(false);
  rows = signal<Summary[]>([]);
  total = signal(0);
  page = 1;
  search = "";
  customerFilter = "";
  customerSearch = "";
  chosenCustomerID = "";
  customerChoices = signal<{ id: string; name: string }[]>([]);
  teams = signal<{ id: string; name: string; members: string[] }[]>([]);
  selected = signal<Invoice | null>(null);
  editing = signal(false);
  form = emptyInvoice();
  calculation = signal<Financials | null>(null);
  busy = signal(false);
  error = signal("");
  message = signal("");
  confirmed = signal<{ id: string; version: string } | null>(null);
  needsReload = signal(false);
  private calculatedFor = "";
  private pending = new Map<string, { body: string; key: string }>();
  private baseline = "";
  async ngOnInit() {
    await Promise.all([this.load(), this.loadTeams()]);
 const id=this.route?.snapshot.queryParamMap.get("id");if(id)await this.open(id);
  }
  catalogEnabled() {
    return this.api.has("clientes.view") && this.api.moduleEnabled("customers");
  }
  async findCustomers() {
    if (!this.catalogEnabled() || this.busy()) return;
    this.busy.set(true);
    try {
      const q = new URLSearchParams({
        page: "1",
        per_page: "100",
        search: this.customerSearch,
        is_active: "true",
      });
      this.customerChoices.set(
        (
          await this.api.get<{ data: { id: string; name: string }[] }>(
            "/customers?" + q,
          )
        ).data,
      );
    } catch (e) {
      this.error.set(this.api.message(e));
    } finally {
      this.busy.set(false);
    }
  }
  async chooseCustomer() {
    if (!this.catalogEnabled() || this.busy() || !this.writable()) return;
    if (this.chosenCustomerID === this.form.customer.id) return;
    if (!this.chosenCustomerID) {
      this.form.customer = { ...this.form.customer, id: "" };
      return;
    }
    this.busy.set(true);
    try {
      const c = await this.api.get<{
        id: string;
        name: string;
        rtn: string;
        contact_name: string;
        phone: string;
        email: string;
        address: string;
      }>("/customers/" + this.chosenCustomerID);
      this.form.customer = {
        id: c.id,
        name: c.name,
        rtn: c.rtn,
        contact: c.contact_name,
        phone: c.phone,
        email: c.email,
        address: c.address,
      };
    } catch (e) {
      this.chosenCustomerID = this.form.customer.id;
      this.error.set(this.api.message(e));
    } finally {
      this.busy.set(false);
    }
  }
  async loadTeams() {
    if (!this.api.has("core.teams.read")) return;
    try {
      this.teams.set(
        (
          await this.api.get<{
            data: { id: string; name: string; members: string[] }[];
          }>("/teams")
        ).data,
      );
    } catch (e) {
      this.error.set(this.api.message(e));
    }
  }
  allowedTeams() {
    const profile = this.api.profile();
    if (!profile) return [];
    const action = this.selected() ? "facturas.update" : "facturas.create";
    const all = profile.grants.some(
      (g) => g.permission === action && g.scope === "all",
    );
    const owner = this.selected()?.created_by ?? profile.user.id;
    const view = profile.grants.some(
      (g) =>
        g.permission === "facturas.view" &&
        (g.scope === "all" ||
          (g.scope === "self" && owner === profile.user.id)),
    );
    return this.teams().filter(
      (t) => (all && view) || t.members.includes(profile.user.id),
    );
  }
  canTransferTeam() {
    return (
      !this.selected() ||
      this.api
        .profile()
        ?.grants.some(
          (g) => g.permission === "facturas.update" && g.scope === "all",
        )
    );
  }
  writable() {
    if (this.needsReload() || this.emissionBusy()) return false;
    return this.selected()
      ? this.selected()?.status === "draft" && this.api.has("facturas.update")
      : this.api.has("facturas.create");
  }
  dirty() {
    return this.editing() && JSON.stringify(this.form) !== this.baseline;
  }
  newDraft() {
    if (this.busy() || this.emissionBusy() || !this.api.has("facturas.create")) return;
    this.selected.set(null);
    this.needsReload.set(false);
    this.form = emptyInvoice();
    this.chosenCustomerID = "";
    this.baseline = JSON.stringify(this.form);
    this.calculation.set(null);
    this.confirmed.set(null);
    this.editing.set(true);
    this.error.set("");
    this.message.set("");
  }
  close() {
    if (this.busy() || this.emissionBusy()) return;
    this.editing.set(false);
    this.selected.set(null);
    this.calculation.set(null);
  }
  addItem() {
    if (this.writable() && !this.busy() && this.form.items.length < 200)
      this.form.items.push({ ...emptyInvoice().items[0] });
  }
  removeItem(i: number) {
    if (this.writable() && !this.busy() && this.form.items.length > 1)
      this.form.items.splice(i, 1);
  }
  terms() {
    this.form.credit_days =
      this.form.payment_term_type === "contado"
        ? null
        : this.form.credit_days || 30;
  }
  async load() {
    try {
      const q = new URLSearchParams({
        page: String(this.page),
        per_page: "25",
        search: this.search,
        customer_id: this.customerFilter,
 commercial_status:this.commercialFilter,
      });
      const r = await this.api.get<{ data: Summary[]; total: number }>(
        "/invoices?" + q,
      );
      this.rows.set(r.data);
      this.total.set(r.total);
    } catch (e) {
      this.error.set(this.api.message(e));
    }
  }
  async open(id: string) {
    if (this.busy() || this.emissionBusy()) return;
    this.busy.set(true);
    this.error.set("");
    try {
      await this.openConfirmed(id);
    } catch (e) {
      this.error.set(this.api.message(e));
    } finally {
      this.busy.set(false);
    }
  }
  async emissionConfirmed(r:InvoiceEmissionResult){
    this.confirmed.set({id:r.id,version:r.version});this.needsReload.set(true);this.form.version=r.version;
    this.message.set('Emisión confirmada: '+r.invoice_number+'. Abra la factura confirmada para consultar su ficha.');
    this.busy.set(true);try{await this.openConfirmed(r.id);this.message.set('Emisión confirmada: '+r.invoice_number);await this.load()}catch(e){this.error.set(this.api.message(e))}finally{this.busy.set(false)}
  }
  async paymentConfirmed(r:PaymentResult){
 this.confirmed.set({id:r.invoice_id,version:r.version});this.needsReload.set(true);this.form.version=r.version;this.message.set('Pago confirmado #'+r.payment_id+' · saldo '+r.currency+' '+r.balance);this.busy.set(true);try{await this.openConfirmed(r.invoice_id);await this.load()}catch(e){this.error.set(this.api.message(e))}finally{this.busy.set(false)}
 }
  async cancellationConfirmed(r:CancellationResult){
    this.confirmed.set({id:r.id,version:r.version});this.needsReload.set(true);this.form.version=r.version;this.message.set('Factura '+r.invoice_number+' anulada; número conservado.');this.busy.set(true);try{await this.openConfirmed(r.id);await this.load()}catch(e){this.error.set(this.api.message(e))}finally{this.busy.set(false)}
  }
  private async openConfirmed(id: string) {
    const o = await this.api.get<Invoice>("/invoices/" + id);
    this.selected.set(o);
    this.form = {
      customer: { ...o.customer },
      invoice_date: o.invoice_date,
      payment_term_type: o.payment_term_type,
      credit_days: o.credit_days,
      currency: o.currency,
      public_notes: o.public_notes,
      general_discount_type: o.general_discount_type,
      general_discount: o.general_discount,
      exoneration_reference: o.exoneration_reference,
      additional_expenses: o.calculation.additional_expenses,
      team_id: o.team_id,
      version: o.version,
      items: o.items.map((x) => ({
        id: x.id,
        description: x.description,
        unit: x.unit,
        quantity: x.quantity,
        unit_price: x.unit_price,
        unit_cost: x.unit_cost,
        discount_type: x.discount_type,
        discount: x.discount,
        tax: x.tax,
      })),
    };
    this.baseline = JSON.stringify(this.form);
    this.calculation.set(o.calculation);
    this.chosenCustomerID = o.customer.id;
    this.calculatedFor = JSON.stringify(this.financialInput());
    this.needsReload.set(false);
    this.editing.set(true);
  }
  financialInput() {
    const {
      currency,
      general_discount_type,
      general_discount,
      exoneration_reference,
      additional_expenses,
    } = this.form;
    const items = this.form.items.map(({ id, ...item }) => item);
    return {
      currency,
      general_discount_type,
      general_discount,
      exoneration_reference,
      additional_expenses,
      items,
    };
  }
  currentCalculation() {
    return this.calculatedFor === JSON.stringify(this.financialInput())
      ? this.calculation()
      : null;
  }
  async preview() {
    if (this.busy() || this.emissionBusy()) return;
    this.busy.set(true);
    this.error.set("");
    try {
      const body = this.financialInput();
      const r = await this.api.write<{ calculation: Financials }>(
        "POST",
        "/invoices/calculate",
        body,
      );
      this.calculation.set(r.calculation);
      this.calculatedFor = JSON.stringify(body);
    } catch (e) {
      this.error.set(this.api.message(e));
    } finally {
      this.busy.set(false);
    }
  }
  async save() {
    if (this.busy() || this.emissionBusy() || !this.writable()) return;
    const id = this.selected()?.id ?? "";
    const path = "/invoices" + (id ? "/" + id : "");
    const body = structuredClone(this.form);
    const serialized = JSON.stringify(body);
    let intent = this.pending.get(path);
    if (!intent || intent.body !== serialized) {
      intent = { body: serialized, key: crypto.randomUUID() };
      this.pending.set(path, intent);
    }
    this.busy.set(true);
    this.error.set("");
    this.message.set("");
    try {
      const result = await this.api.write<{ id: string; version: string }>(
        id ? "PATCH" : "POST",
        path,
        body,
        intent.key,
      );
      this.pending.delete(path);
      this.confirmed.set(result);
      this.needsReload.set(true);
      this.form.version = result.version;
      this.baseline = JSON.stringify(this.form);
      this.message.set(id ? "Borrador actualizado." : "Borrador registrado.");
      const saved = this.selected();
      if (saved) this.selected.set({ ...saved, version: result.version });
      await this.openConfirmed(result.id);
      await this.load();
    } catch (e) {
      this.error.set(this.api.message(e));
    } finally {
      this.busy.set(false);
    }
  }
}
