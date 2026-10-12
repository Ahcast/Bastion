import {Component,Input,Output,EventEmitter,OnChanges,inject,signal} from '@angular/core';
import {FormsModule} from '@angular/forms';
import {Client} from '../../core/api/client';
import {Reauthentication} from '../../core/security/reauthentication';
import {FiscalValue} from './fiscal-settings';
export interface EmissionInvoice{id:string;version:string;status:string;source_purchase_order_id?:string|null}
export interface EmissionReview{source_order_version?:string|null;invoice_version:string;fiscal_version:string;company_version:string;fiscal:FiscalValue;company:{name:string;tax_id:string;address:string};estimated_number:string;due_date:string}
export interface InvoiceEmissionResult{id:string;version:string;status:string;invoice_number:string;issued_at:string;due_date:string}
@Component({selector:'bastion-invoice-emission',imports:[FormsModule,Reauthentication],template:`
@if(api.has('facturas.emit')&&api.has('facturas.view')&&sourceAllowed()){<section aria-label="Emisión de factura">
@if(confirmed();as r){<p role="status">Emisión confirmada: {{r.invoice_number}} · versión {{r.version}} · vencimiento {{r.due_date}}</p>}
@if(invoice.status==='draft'&&!confirmed()){
<h3>Emitir factura</h3><p>La emisión congela número, autorización fiscal y empresa. Revise los datos guardados antes de continuar.</p>
@if(changesPending){<p>Guarde los cambios o recargue la ficha antes de revisar la emisión.</p>}
<button type="button" (click)="load()" [disabled]="busy()||disabled||changesPending">Revisar datos para emitir</button>
@if(error()){<p role="alert">{{error()}}</p>}
@if(review();as c){@if(!changesPending){<p>Empresa: {{c.company.name}} · RTN: {{c.company.tax_id}} · versión {{c.company_version}}</p><p>Dirección de empresa: {{c.company.address}}</p><p>CAI configurado: {{c.fiscal.cai}} · versión fiscal {{c.fiscal_version}} · moneda {{c.fiscal.currency}}</p><p>Vigencia: {{c.fiscal.valid_from}} a {{c.fiscal.valid_until}} · rango {{c.fiscal.range_start}} a {{c.fiscal.range_end}}</p><p>Número estimado: {{c.estimated_number}}. No está reservado; el servidor asigna el número al confirmar.</p>@if(c.source_order_version){<p>Versión de OC revisada: {{c.source_order_version}}. Emitir marcará la OC como Facturado.</p>}<p>Vencimiento revisado: {{c.due_date}}</p>
<bastion-reauthentication />
<label>Motivo de emisión<textarea [(ngModel)]="reason" [disabled]="busy()||disabled" maxlength="1000"></textarea></label>
<button type="button" (click)="emit()" [disabled]="!canEmit()">Confirmar emisión de factura</button>
}}
}
</section>}`})
export class InvoiceEmission implements OnChanges{
 @Input({required:true})invoice!:EmissionInvoice;
 @Input()changesPending=false;
 @Input()disabled=false;
 @Output()busyChange=new EventEmitter<boolean>();
 @Output()issued=new EventEmitter<InvoiceEmissionResult>();
 api=inject(Client);busy=signal(false);error=signal('');review=signal<EmissionReview|null>(null);confirmed=signal<InvoiceEmissionResult|null>(null);reason='';private invoiceKey='';private pending=new Map<string,string>();
 ngOnChanges(){if(!this.invoice)return;if(this.changesPending)this.review.set(null);const key=this.invoice.id+':'+this.invoice.version;if(key!==this.invoiceKey){if(this.invoice.id!==this.invoiceKey.split(':')[0])this.confirmed.set(null);this.invoiceKey=key;this.review.set(null);this.reason='';this.error.set('')}}
 sourceAllowed(){return !this.invoice.source_purchase_order_id||(this.api.has("ordenes.convert")&&this.api.has("ordenes.view")&&this.api.moduleEnabled("purchase-orders"))}
 async load(){if(!this.sourceAllowed()||this.busy()||this.disabled||this.changesPending||this.invoice.status!=='draft'||!this.api.has('facturas.emit')||!this.api.has('facturas.view'))return;
 const key=this.invoice.id+':'+this.invoice.version;this.busy.set(true);this.busyChange.emit(true);this.error.set('');try{const c=await this.api.get<EmissionReview>('/invoices/'+this.invoice.id+'/emission-context');if(key===this.invoice.id+':'+this.invoice.version&&!this.changesPending)this.review.set(c)}catch(e){if(key===this.invoice.id+':'+this.invoice.version)this.error.set(this.api.message(e))}finally{this.busy.set(false);this.busyChange.emit(false)}}
 canEmit(){return this.sourceAllowed()&&!this.busy()&&!this.disabled&&!this.changesPending&&!this.confirmed()&&this.invoice.status==='draft'&&this.api.has('facturas.emit')&&this.api.has('facturas.view')&&this.review()?.invoice_version===this.invoice.version&&!!this.reason.trim()}
 async emit(){if(!this.canEmit())return;const c=this.review()!;const body={...(c.source_order_version?{source_order_version:c.source_order_version}:{}),version:this.invoice.version,fiscal_version:c.fiscal_version,company_version:c.company_version,reason:this.reason};const path='/invoices/'+this.invoice.id+'/emit';const serialized=path+JSON.stringify(body);let key=this.pending.get(serialized);if(!key){key=crypto.randomUUID();this.pending.set(serialized,key)}
 this.busy.set(true);this.busyChange.emit(true);this.error.set('');try{const r=await this.api.write<InvoiceEmissionResult>('POST',path,body,key);this.pending.delete(serialized);this.confirmed.set(r);this.reason='';this.issued.emit(r)}catch(e){this.error.set(this.api.message(e))}finally{this.busy.set(false);this.busyChange.emit(false)}}
}
