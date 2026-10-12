import {Component,Input,Output,EventEmitter,OnChanges,inject,signal} from '@angular/core';
import {FormsModule} from '@angular/forms';
import {Client} from '../../core/api/client';
import {Reauthentication} from '../../core/security/reauthentication';

export interface CancellationInvoice{id:string;version:string;status:string;source_purchase_order_id?:string|null}
export interface CancellationResult{id:string;version:string;status:string;invoice_number:string;cancelled_at:string}

@Component({selector:'bastion-invoice-cancellation',imports:[FormsModule,Reauthentication],template:`
@if(invoice.status==='issued'&&api.has('facturas.cancel')&&api.has('facturas.view')&&!invoice.source_purchase_order_id){<section aria-label="Anulación de factura">
<h3>Anular factura</h3><p>Conserva el número y los datos fiscales. Sólo se permite sin pagos ni documentos generados.</p>
<bastion-reauthentication/>
<label>Motivo de anulación<textarea [(ngModel)]="reason" maxlength="1000" [disabled]="busy()||disabled"></textarea></label>
<label><input type="checkbox" [(ngModel)]="attested" [disabled]="busy()||disabled">Confirmo que marqué «Anulado» y conservé el original y la copia.</label>
<button type="button" (click)="cancel()" [disabled]="busy()||disabled||changesPending||!reason.trim()||!attested">Confirmar anulación de factura</button>
@if(error()){<p role="alert">{{error()}}</p>}@if(confirmed()){<p role="status">Factura anulada. Número {{confirmed()!.invoice_number}} conservado.</p>}
</section>}`})
export class InvoiceCancellation implements OnChanges{
 @Input({required:true})invoice!:CancellationInvoice;
 @Input()disabled=false;
 @Input()changesPending=false;
 @Output()busyChange=new EventEmitter<boolean>();
 @Output()cancelled=new EventEmitter<CancellationResult>();
 api=inject(Client);busy=signal(false);error=signal('');confirmed=signal<CancellationResult|null>(null);reason='';attested=false;
 private invoiceKey='';private pending=new Map<string,string>();
 ngOnChanges(){if(!this.invoice)return;const key=this.invoice.id+':'+this.invoice.version;if(key!==this.invoiceKey){this.invoiceKey=key;this.reason='';this.attested=false;this.error.set('');this.confirmed.set(null);this.pending.clear()}}
 async cancel(){if(this.busy()||this.confirmed()||this.disabled||this.changesPending||this.invoice.status!=='issued'||this.invoice.source_purchase_order_id||!this.api.has('facturas.cancel')||!this.api.has('facturas.view')||!this.reason.trim()||!this.attested)return;
 const path='/invoices/'+this.invoice.id+'/cancel',body={version:this.invoice.version,reason:this.reason,original_and_copy_marked_and_archived:true},intent=path+JSON.stringify(body);let key=this.pending.get(intent);if(!key){key=crypto.randomUUID();this.pending.set(intent,key)}
 this.busy.set(true);this.busyChange.emit(true);this.error.set('');try{const result=await this.api.write<CancellationResult>('POST',path,body,key);this.pending.delete(intent);this.confirmed.set(result);this.cancelled.emit(result)}catch(e){this.error.set(this.api.message(e))}finally{this.busy.set(false);this.busyChange.emit(false)}
 }
}
