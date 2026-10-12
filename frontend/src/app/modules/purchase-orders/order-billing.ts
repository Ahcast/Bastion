import {Component,Input,Output,EventEmitter,OnChanges,inject,signal} from '@angular/core';
import {FormsModule} from '@angular/forms';
import {RouterLink} from '@angular/router';
import {Client} from '../../core/api/client';
import {Reauthentication} from '../../core/security/reauthentication';
export interface BillingOrder{id:string;version:string;status:string;deleted_at?:string|null;invoice_id?:string|null;invoice_number_snapshot?:string|null;items:{status:string}[]}
export interface BillingResult{id:string;version:string;status:string;invoice_id:string;invoice_version:string}
interface LinkedInvoice{id:string;version:string;status:string;invoice_number:string|null;currency:string;paid_total:string;balance:string}
@Component({selector:'bastion-order-billing',imports:[FormsModule,RouterLink,Reauthentication],template:`
<section aria-label="Facturación y cierre de OC">
@if(confirmed();as r){<p role="status">Operación de facturación confirmada · OC {{r.id}} · versión {{r.version}} · factura {{r.invoice_id}}</p>}
@if(error()){<p role="alert">{{error()}}</p>}
@if(order.invoice_id){<p>Factura vinculada: {{order.invoice_number_snapshot||'Borrador #'+order.invoice_id}}</p>@if(api.has('facturas.view')&&api.moduleEnabled('invoices')){<a routerLink="/invoices" [queryParams]="{id:order.invoice_id}">Ver factura vinculada</a><button type="button" (click)="load()" [disabled]="disabled||busy()">Consultar factura para cierre</button>}
@if(linked();as l){<p>Factura revisada · versión {{l.version}} · pagado {{l.currency}} {{l.paid_total}} · saldo {{l.currency}} {{l.balance}}</p>}
@if((order.status==='invoiced'&&api.has('ordenes.close'))||(order.status==='closed'&&api.has('ordenes.reopen'))){<p>Cerrar requiere factura emitida con saldo cero. Corregir pagos de una OC cerrada requiere reapertura.</p><bastion-reauthentication/><label>Motivo de cierre o reapertura<textarea [(ngModel)]="closureReason" maxlength="1000" [disabled]="disabled||busy()"></textarea></label>@if(order.status==='invoiced'){<button type="button" (click)="close(false)" [disabled]="!canClose(false)">Cerrar OC facturada</button>}@else{<button type="button" (click)="close(true)" [disabled]="!canClose(true)">Reabrir OC para revisar cobro</button>}}
}
@else if(available()){
<h3>Crear factura desde OC entregada</h3><p>El borrador copiará cliente, moneda, ítems, costos y gastos de la OC. Emitir y cobrar son acciones separadas.</p><fieldset [disabled]="disabled||busy()"><label>Fecha de factura de OC<input type="date" [(ngModel)]="date"></label><label for="order-invoice-term">Condición de factura de OC</label><select id="order-invoice-term" [(ngModel)]="term" (ngModelChange)="days=term==='credito'?30:null"><option value="contado">Contado</option><option value="credito">Crédito</option></select>@if(term==='credito'){<label>Días de crédito de factura de OC<input type="number" [(ngModel)]="days" min="1" max="65535"></label>}<label>Notas públicas de factura de OC<textarea [(ngModel)]="notes" maxlength="12000"></textarea></label><label>Motivo de facturación de OC<textarea [(ngModel)]="reason" maxlength="1000"></textarea></label><button type="button" (click)="convert()" [disabled]="changesPending||!date||!reason.trim()">Confirmar factura de OC</button></fieldset>@if(changesPending){<p>Guarde o recargue la OC antes de convertir.</p>}
}
</section>`})
export class OrderBilling implements OnChanges{
 @Input({required:true})order!:BillingOrder;
 @Input()disabled=false;
 @Input()changesPending=false;
 @Output()changed=new EventEmitter<BillingResult>();
 @Output()busyChange=new EventEmitter<boolean>();
 api=inject(Client);busy=signal(false);error=signal('');confirmed=signal<BillingResult|null>(null);linked=signal<LinkedInvoice|null>(null);date=new Intl.DateTimeFormat('en-CA',{timeZone:'America/Tegucigalpa',year:'numeric',month:'2-digit',day:'2-digit'}).format(new Date());term='contado';days:number|null=null;notes='';reason='';closureReason='';private orderKey='';private pending=new Map<string,string>();
 ngOnChanges(){if(!this.order)return;const key=this.order.id+':'+this.order.version;if(key!==this.orderKey){if(this.order.id!==this.orderKey.split(':')[0]){this.confirmed.set(null);this.reason='';this.notes=''}this.orderKey=key;this.linked.set(null);this.closureReason='';this.error.set('')}}
 available(){return !this.order.deleted_at&&this.order.status==='delivered'&&!this.order.invoice_id&&this.order.items.length>0&&this.order.items.every(x=>x.status==='delivered')&&this.api.moduleEnabled('invoices')&&['ordenes.view','ordenes.convert','facturas.view','facturas.create'].every(p=>this.api.has(p))}
 async load(){if(this.busy()||this.disabled||!this.order.invoice_id||!this.api.has('facturas.view')||!this.api.moduleEnabled('invoices'))return;const key=this.order.id+':'+this.order.version;this.busy.set(true);this.busyChange.emit(true);this.error.set('');try{const l=await this.api.get<LinkedInvoice>('/invoices/'+this.order.invoice_id);if(key===this.order.id+':'+this.order.version&&l.id===this.order.invoice_id){this.linked.set(l);this.confirmed.set(null)}}catch(e){if(key===this.order.id+':'+this.order.version)this.error.set(this.api.message(e))}finally{this.busy.set(false);this.busyChange.emit(false)}}
 canClose(reopen:boolean){const l=this.linked();return !this.busy()&&!this.disabled&&!this.changesPending&&!this.confirmed()&&!this.order.deleted_at&&this.order.status===(reopen?'closed':'invoiced')&&!!l&&l.id===this.order.invoice_id&&l.status==='issued'&&(reopen||l.balance==='0.00')&&this.api.moduleEnabled('invoices')&&this.api.has('facturas.view')&&this.api.has('ordenes.view')&&this.api.has(reopen?'ordenes.reopen':'ordenes.close')&&!!this.closureReason.trim()}
 async convert(){if(!this.available()||this.disabled||this.busy()||this.changesPending||this.confirmed()||!this.date||!this.reason.trim())return;await this.mutate('convert-to-invoice',{version:this.order.version,invoice_date:this.date,payment_term_type:this.term,credit_days:this.term==='credito'?this.days:null,public_notes:this.notes,reason:this.reason})}
 async close(reopen:boolean){if(!this.canClose(reopen))return;await this.mutate(reopen?'reopen':'close',{version:this.order.version,invoice_version:this.linked()!.version,reason:this.closureReason})}
 private async mutate(action:string,body:unknown){const path='/purchase-orders/'+this.order.id+'/'+action;const serialized=path+JSON.stringify(body);let key=this.pending.get(serialized);if(!key){key=crypto.randomUUID();this.pending.set(serialized,key)}this.busy.set(true);this.busyChange.emit(true);this.error.set('');try{const r=await this.api.write<BillingResult>('POST',path,body,key);this.pending.delete(serialized);this.confirmed.set(r);this.reason='';this.closureReason='';this.changed.emit(r)}catch(e){this.error.set(this.api.message(e))}finally{this.busy.set(false);this.busyChange.emit(false)}}
}
