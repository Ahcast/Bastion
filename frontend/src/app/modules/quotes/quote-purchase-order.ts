import {Component, input, output, inject, signal} from '@angular/core';
import {FormsModule} from '@angular/forms';
import {RouterLink} from '@angular/router';
import {Client} from '../../core/api/client';
import type {Quote} from './quotes';

export interface ConversionResult {id:string;version:string;status:string;purchase_order_id:string;purchase_order_version:string}
@Component({selector:'app-quote-purchase-order',imports:[FormsModule,RouterLink],template:`
@if(quote().purchase_order_id){
 <p>Cotización convertida a OC {{quote().purchase_order_id}}.</p>
 @if(api.moduleEnabled('purchase-orders')&&api.has('ordenes.view')){<a routerLink="/purchase-orders" [queryParams]="{id:quote().purchase_order_id}">Ver orden de compra</a>}
} @else if(available()){
 <section aria-label="Conversión a orden de compra"><h3>Crear OC desde cotización aceptada</h3>
 <p>Se copiarán cliente, moneda, ítems, descuentos, impuestos y costos guardados. La cotización quedará convertida; sus documentos permanecen en el origen.</p>
 <label>Número de OC del cliente<input [(ngModel)]="orderNumber" maxlength="160" [disabled]="busy()"></label>
 <label>Destino de la OC<input [(ngModel)]="destination" maxlength="1000" [disabled]="busy()"></label>
 <label>Fecha de la OC<input type="date" [(ngModel)]="orderDate" [disabled]="busy()"></label>
 <label>Entrega requerida de la OC<input type="date" [(ngModel)]="deliveryDate" [disabled]="busy()"></label>
 <label>Comentarios de la OC<textarea [(ngModel)]="comments" maxlength="12000" [disabled]="busy()"></textarea></label>
 <label>Gastos adicionales de la OC<input [(ngModel)]="expenses" inputmode="decimal" [disabled]="busy()"></label>
 <button (click)="convert()" [disabled]="busy()||!orderNumber.trim()||!orderDate">Confirmar conversión a OC</button>
 </section>
}
@if(error()){<p role="alert">{{error()}}</p>}
`})
export class QuotePurchaseOrder {
 api=inject(Client);quote=input.required<Quote>();changesPending=input(false);converted=output<ConversionResult>();busy=signal(false);error=signal('');
 orderNumber='';destination='';orderDate=new Intl.DateTimeFormat('en-CA',{timeZone:'America/Tegucigalpa',year:'numeric',month:'2-digit',day:'2-digit'}).format(new Date());deliveryDate='';comments='';expenses='0';
 private pending:{body:string;key:string}|null=null;
 available(){return this.quote().status==='accepted'&&!this.quote().deleted_at&&this.api.moduleEnabled('purchase-orders')&&['cotizaciones.convert','ordenes.create','ordenes.view'].every(p=>this.api.has(p))}
 async convert(){
  if(!this.available()||this.busy())return;
  if(this.changesPending()){this.error.set('Guarde o descarte los cambios antes de convertir.');return}
  const q=this.quote(),body={version:q.version,order_number:this.orderNumber.trim(),destination:this.destination,order_date:this.orderDate,required_delivery_date:this.deliveryDate||null,comments:this.comments,additional_expenses:this.expenses};
  const serialized=JSON.stringify({id:q.id,...body});if(this.pending?.body!==serialized)this.pending={body:serialized,key:crypto.randomUUID()};
  this.busy.set(true);this.error.set('');
  try{const r=await this.api.write<ConversionResult>('POST',`/quotes/${q.id}/convert-to-purchase-order`,body,this.pending!.key);this.pending=null;this.converted.emit(r)}catch(e){this.error.set(this.api.message(e))}finally{this.busy.set(false)}
 }
}
