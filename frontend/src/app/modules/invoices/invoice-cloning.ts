import {Component,Input,Output,EventEmitter,OnChanges,inject,signal} from '@angular/core';
import {FormsModule} from '@angular/forms';
import {Client} from '../../core/api/client';

export interface CloningInvoice {id:string;version:string;status:string;invoice_date:string;public_notes:string}
export interface CloningResult {id:string;version:string}

@Component({selector:'bastion-invoice-cloning',imports:[FormsModule],template:`
@if((invoice.status==='draft'||invoice.status==='issued')&&api.has('facturas.create')&&api.has('facturas.view')){
<section aria-label="Clonación de factura"><h3>Clonar factura</h3>
<p>Se creará un borrador independiente con los datos comerciales históricos. Revise la fecha y las notas nuevas antes de guardarlo.</p>
<label>Fecha del nuevo borrador<input type="date" [(ngModel)]="date" [disabled]="busy()||disabled"></label>
<label>Notas públicas del nuevo borrador<textarea [(ngModel)]="notes" maxlength="12000" [disabled]="busy()||disabled"></textarea></label>
<button type="button" (click)="clone()" [disabled]="busy()||disabled||changesPending||!date">Crear borrador clonado</button>
@if(error()){<p role="alert">{{error()}}</p>}
@if(confirmed();as result){<p role="status">Borrador clonado #{{result.id}} creado.</p><button type="button" (click)="openClone.emit(result.id)" [disabled]="busy()||disabled">Abrir borrador clonado #{{result.id}}</button>}
</section>}`})
export class InvoiceCloning implements OnChanges {
 @Input({required:true})invoice!:CloningInvoice;
 @Input()disabled=false;
 @Input()changesPending=false;
 @Output()busyChange=new EventEmitter<boolean>();
 @Output()cloned=new EventEmitter<CloningResult>();
 @Output()openClone=new EventEmitter<string>();
 api=inject(Client);busy=signal(false);error=signal('');confirmed=signal<CloningResult|null>(null);date='';notes='';
 private invoiceKey='';private pending=new Map<string,string>();
 ngOnChanges(){if(!this.invoice)return;const key=this.invoice.id+':'+this.invoice.version;if(key!==this.invoiceKey){this.invoiceKey=key;this.date=this.invoice.invoice_date;this.notes=this.invoice.public_notes;this.error.set('');this.confirmed.set(null);this.pending.clear()}}
 async clone(){if(this.busy()||this.confirmed()||this.disabled||this.changesPending||!this.date||!['draft','issued'].includes(this.invoice.status)||!this.api.has('facturas.create')||!this.api.has('facturas.view'))return;
 const path='/invoices/'+this.invoice.id+'/clone',body={version:this.invoice.version,invoice_date:this.date,public_notes:this.notes},intent=path+JSON.stringify(body);let key=this.pending.get(intent);if(!key){key=crypto.randomUUID();this.pending.set(intent,key)}
 this.busy.set(true);this.busyChange.emit(true);this.error.set('');try{const result=await this.api.write<CloningResult>('POST',path,body,key);this.pending.delete(intent);this.confirmed.set(result);this.cloned.emit(result)}catch(e){this.error.set(this.api.message(e))}finally{this.busy.set(false);this.busyChange.emit(false)}
 }
}
