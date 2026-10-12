import {Component,Input,OnChanges,inject,signal} from '@angular/core';
import {Client} from '../../core/api/client';
import type {Invoice} from './invoices';
import type {PaymentRow,PaymentsPage} from './invoice-payments';

type DocumentKind='invoice'|'delivery_note'|'receipt';
interface GeneratedDocument{id:string;document_id:string|null;kind:DocumentKind;status:string;invoice_version:string;payment_id:string|null;payment_version:string|null;created_at:string;available_at:string|null}
interface DocumentPage{data:GeneratedDocument[];total:number;page:number;per_page:number}
interface GeneratedResult{id:string;document_id:string;kind:DocumentKind;status:string;invoice_version:string}

@Component({
 selector:'bastion-invoice-documents',
 template:`@if(api.has('facturas.view')){<section aria-label="Documentos de factura"><h3>Documentos</h3>
 @if(error()){<p role="alert">{{error()}}</p>}@if(message()){<p role="status">{{message()}}</p>}
 <button type="button" (click)="load()" [disabled]="busy()||disabled">Consultar documentos</button>
 @if(canGenerate()){
 <button type="button" (click)="generate('invoice')" [disabled]="busy()||disabled||changesPending">Generar {{invoice.status==='draft'?'borrador de factura PDF':'factura PDF'}}</button>
 @if(invoice.status==='issued'){<button type="button" (click)="generate('delivery_note')" [disabled]="busy()||disabled||changesPending">Generar nota de entrega</button>}
 }
 <p>{{total()}} documentos generados · Página {{page}}</p>
 <ul>@for(row of documents();track row.id){<li>{{kindLabel(row.kind)}}@if(row.payment_id){ · pago #{{row.payment_id}}} · {{statusLabel(row.status)}}@if(row.available_at){ · {{row.available_at}}}
 @if(row.status==='available'&&row.document_id&&api.has('core.documents.read')){<button type="button" (click)="deliver(row,'preview')">Vista previa</button><button type="button" (click)="deliver(row,'download')">Descargar</button>}</li>}@empty{<li>No hay documentos generados para esta factura.</li>}</ul>
 <button type="button" (click)="page=page-1;loadDocuments()" [disabled]="busy()||disabled||page<=1">Documentos anteriores</button><button type="button" (click)="page=page+1;loadDocuments()" [disabled]="busy()||disabled||page*25>=total()">Más documentos</button>
 @if(invoice.status==='issued'){
 <h4>Recibos por pago</h4><button type="button" (click)="loadPayments()" [disabled]="busy()||disabled">Consultar pagos para recibos</button>
 @if(payments();as ledger){<p>{{ledger.total}} pagos · Página {{paymentPage}}</p><ul>@for(row of ledger.data;track row.id){<li>Pago #{{row.id}} · {{row.date}} · {{row.method}} · {{row.currency}} {{row.amount}}@if(row.voided_at){ · Anulado}
 @if(canGenerate()){<button type="button" (click)="generate('receipt',row)" [disabled]="busy()||disabled||changesPending">Generar {{row.voided_at?'recibo anulado':'recibo'}} #{{row.id}}</button>}</li>}@empty{<li>No hay pagos para generar recibos.</li>}</ul>
 <button type="button" (click)="paymentPage=paymentPage-1;loadPayments()" [disabled]="busy()||disabled||paymentPage<=1">Pagos anteriores</button><button type="button" (click)="paymentPage=paymentPage+1;loadPayments()" [disabled]="busy()||disabled||paymentPage*25>=ledger.total">Más pagos</button>}
 }
 @if(changesPending){<p>Guarde o recargue la factura antes de generar documentos para evitar usar una versión anterior.</p>}
 </section>}`
})
export class InvoiceDocuments implements OnChanges{
 @Input({required:true})invoice!:Invoice;
 @Input()disabled=false;
 @Input()changesPending=false;
 api=inject(Client);documents=signal<GeneratedDocument[]>([]);total=signal(0);payments=signal<PaymentsPage|null>(null);busy=signal(false);error=signal('');message=signal('');page=1;paymentPage=1;
 private invoiceKey='';private pending=new Map<string,{body:string;key:string}>();
 ngOnChanges(){if(!this.invoice)return;const key=this.invoice.id+':'+this.invoice.version;if(key!==this.invoiceKey){this.invoiceKey=key;this.documents.set([]);this.total.set(0);this.payments.set(null);this.page=1;this.paymentPage=1;this.error.set('');this.message.set('');this.pending.clear()}}
 canGenerate(){return !this.changesPending&&this.api.has('facturas.documents.generate')&&this.api.has('core.documents.write')}
 kindLabel(kind:DocumentKind){return ({invoice:'Factura PDF',delivery_note:'Nota de entrega',receipt:'Recibo'} as Record<DocumentKind,string>)[kind]}
 statusLabel(status:string){return ({pending:'Pendiente',available:'Disponible'} as Record<string,string>)[status]||status}
 private intent(path:string,body:unknown){const serialized=JSON.stringify(body),old=this.pending.get(path);if(old?.body===serialized)return old.key;const key=crypto.randomUUID();this.pending.set(path,{body:serialized,key});return key}
 async load(){if(this.busy()||this.disabled)return;this.busy.set(true);this.error.set('');try{await this.loadDocuments(false);if(this.invoice.status==='issued')await this.loadPayments(false)}catch(e){this.error.set(this.api.message(e))}finally{this.busy.set(false)}}
 async loadDocuments(manageBusy=true){if(this.disabled)return;if(manageBusy){if(this.busy())return;this.busy.set(true);this.error.set('')}const key=this.invoice.id+':'+this.invoice.version;try{const r=await this.api.get<DocumentPage>(`/invoices/${this.invoice.id}/documents?page=${this.page}&per_page=25`);if(key===this.invoice.id+':'+this.invoice.version){this.documents.set(r.data);this.total.set(r.total);this.page=r.page}}catch(e){if(manageBusy)this.error.set(this.api.message(e));else throw e}finally{if(manageBusy)this.busy.set(false)}}
 async loadPayments(manageBusy=true){if(this.disabled||this.invoice.status!=='issued')return;if(manageBusy){if(this.busy())return;this.busy.set(true);this.error.set('')}const key=this.invoice.id+':'+this.invoice.version;try{const r=await this.api.get<PaymentsPage>(`/invoices/${this.invoice.id}/payments?page=${this.paymentPage}&per_page=25`);if(key===this.invoice.id+':'+this.invoice.version){this.payments.set(r);this.paymentPage=r.page}}catch(e){if(manageBusy)this.error.set(this.api.message(e));else throw e}finally{if(manageBusy)this.busy.set(false)}}
 async generate(kind:DocumentKind,payment?:PaymentRow){if(this.busy()||this.disabled||!this.canGenerate())return;if(kind==='delivery_note'&&this.invoice.status!=='issued')return;if(kind==='receipt'&&(!payment||this.invoice.status!=='issued'))return;const body={kind,version:this.invoice.version,payment_id:payment?.id??null,payment_version:payment?.version??null};const path=`/invoices/${this.invoice.id}/documents`,key=this.intent(path+':'+kind+':'+(payment?.id??''),body),invoiceKey=this.invoice.id+':'+this.invoice.version;this.busy.set(true);this.error.set('');this.message.set('');try{const r=await this.api.write<GeneratedResult>('POST',path,body,key);if(invoiceKey!==this.invoice.id+':'+this.invoice.version)return;this.pending.delete(path+':'+kind+':'+(payment?.id??''));this.message.set(this.kindLabel(r.kind)+' generado.');await this.loadDocuments(false)}catch(e){if(invoiceKey===this.invoice.id+':'+this.invoice.version)this.error.set(this.api.message(e))}finally{this.busy.set(false)}}
 async deliver(row:GeneratedDocument,action:'preview'|'download'){if(row.status!=='available'||!row.document_id||!this.api.has('core.documents.read'))return;this.error.set('');try{const r=await this.api.get<{url:string}>(`/documents/${row.document_id}/${action}`);window.open(r.url,'_blank','noopener,noreferrer')}catch(e){this.error.set(this.api.message(e))}}
}
