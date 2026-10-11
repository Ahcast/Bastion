import {Component,inject,signal} from '@angular/core';
import {FormsModule} from '@angular/forms';
import {Client} from '../../core/api/client';
import {Reauthentication} from '../../core/security/reauthentication';
export interface FiscalValue {cai:string;prefix:string;number_width:number|null;range_start:string;range_end:string;valid_from:string;valid_until:string;currency:string;enabled:boolean}
interface Settings {value:FiscalValue;version:string;next_value:string}
const empty=():FiscalValue=>({cai:'',prefix:'',number_width:null,range_start:'',range_end:'',valid_from:'',valid_until:'',currency:'',enabled:false});
@Component({selector:'bastion-invoice-fiscal-settings',imports:[FormsModule,Reauthentication],template:`
@if(api.has('facturas.fiscal.read')){<section aria-label="Configuración fiscal de facturas"><h2>Configuración fiscal</h2>
<p>Configure únicamente datos de una autorización validada. Guardar no emite facturas. El siguiente correlativo no se puede editar desde esta configuración.</p>
<button type="button" (click)="load()" [disabled]="busy()">Consultar configuración fiscal</button>
@if(error()){<p role="alert">{{error()}}</p>}
@if(loaded()){<p>Versión fiscal: {{version}}</p>@if(next()){<p>Siguiente correlativo: {{next()}}</p>}@else{<p>No existe configuración fiscal guardada.</p>}
@if(needsReload()){<p role="status">Configuración guardada. Consulte la configuración fiscal para continuar.</p>}
@if(api.has('facturas.fiscal.manage')){<bastion-reauthentication />}
<form (ngSubmit)="save()"><fieldset [disabled]="busy()||needsReload()||!api.has('facturas.fiscal.manage')"><legend>Autorización configurada</legend>
<label>CAI configurado<input name="fiscalCAI" [(ngModel)]="form.cai" required maxlength="80"></label>
<label>Prefijo fiscal<input name="fiscalPrefix" [(ngModel)]="form.prefix" maxlength="60"></label>
<label>Ancho del correlativo<input name="fiscalWidth" type="number" [(ngModel)]="form.number_width" min="1" max="20" required></label>
<label>Inicio del rango fiscal<input name="fiscalStart" [(ngModel)]="form.range_start" inputmode="numeric" required></label>
<label>Fin del rango fiscal<input name="fiscalEnd" [(ngModel)]="form.range_end" inputmode="numeric" required></label>
<label>Inicio de vigencia fiscal<input name="fiscalFrom" type="date" [(ngModel)]="form.valid_from" required></label>
<label>Límite de emisión fiscal<input name="fiscalUntil" type="date" [(ngModel)]="form.valid_until" required></label>
<label for="fiscal-currency">Moneda autorizada</label><select id="fiscal-currency" name="fiscalCurrency" [(ngModel)]="form.currency" required><option value="">Seleccione</option><option>HNL</option><option>USD</option></select>
<label><input name="fiscalEnabled" type="checkbox" [(ngModel)]="form.enabled">Autorización habilitada</label>
<label>Motivo del cambio fiscal<textarea name="fiscalReason" [(ngModel)]="reason" maxlength="1000" required></textarea></label>
<button type="submit" [disabled]="!reason.trim()">Guardar configuración fiscal</button>
</fieldset></form>}
</section>}`})
export class InvoiceFiscalSettings{
 api=inject(Client);busy=signal(false);loaded=signal(false);error=signal('');next=signal('');needsReload=signal(false);form=empty();version='0';reason='';private pending=new Map<string,string>();
 async load(){if(this.busy()||!this.api.has('facturas.fiscal.read'))return;this.busy.set(true);this.error.set('');try{const r=await this.api.get<Settings>('/invoices/fiscal-settings');this.form=structuredClone(r.value);this.version=r.version;this.next.set(r.next_value);this.loaded.set(true);this.needsReload.set(false)}catch(e){
 if(!this.needsReload()&&typeof e==='object'&&e!==null&&'status' in e&&e.status===404){this.form=empty();this.version='0';this.next.set('');this.loaded.set(true)}else this.error.set(this.api.message(e))
 }finally{this.busy.set(false)}}
 async save(){if(this.busy()||this.needsReload()||!this.loaded()||!this.api.has('facturas.fiscal.read')||!this.api.has('facturas.fiscal.manage')||!this.reason.trim())return;
 const body={version:this.version,value:structuredClone(this.form),reason:this.reason};const serialized=JSON.stringify(body);let key=this.pending.get(serialized);if(!key){key=crypto.randomUUID();this.pending.set(serialized,key)}
 this.busy.set(true);this.error.set('');try{const r=await this.api.write<{version:string;next_value:string}>('PUT','/invoices/fiscal-settings',body,key);this.version=r.version;this.next.set(r.next_value);this.needsReload.set(true);this.reason='';this.pending.delete(serialized)}catch(e){this.error.set(this.api.message(e))}finally{this.busy.set(false)}
 }
}
