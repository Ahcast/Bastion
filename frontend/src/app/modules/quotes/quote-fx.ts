import {Component,OnInit,inject,signal} from '@angular/core';
import {FormsModule} from '@angular/forms';
import {DatePipe} from '@angular/common';
import {Client} from '../../core/api/client';
import {Reauthentication} from '../../core/security/reauthentication';

export interface FXObservation {
 id:string;base_currency:string;quote_currency:string;rate_kind:string;rate:string;valid_from:string;valid_to:string|null;source:string;source_reference:string;source_published_at:string|null;evidence_hash:string;supersedes_id:string;superseded:boolean;reason:string;created_by:string;created_by_name:string;created_at:string;
}
interface FXPage{data:FXObservation[];total:number;page:number;per_page:number}
interface FXInput{rate_kind:string;rate:string;valid_from:string;valid_to:string;source:string;source_reference:string;source_published_at:string;evidence_hash:string;supersedes_id:string;reason:string}

@Component({
 selector:'bastion-quote-fx',
 imports:[FormsModule,DatePipe,Reauthentication],
 template:`
@if (canView()) {
<section class="card" aria-labelledby="fx-title">
 <h2 id="fx-title">Registro de tasas de cambio</h2>
 <p><strong>Este registro no convierte cotizaciones automáticamente.</strong> Conserva observaciones USD/HNL para uso futuro bajo una política comercial aprobada.</p>
 <form (ngSubmit)="load(true)">
  <label>Tipo de tasa
   <select name="fx-filter-kind" [(ngModel)]="filterKind"><option value="">Todos</option><option value="tcr">TCR</option><option value="buy">Compra</option><option value="sell">Venta</option><option value="manual">Manual</option></select>
  </label>
  <label>Fuente
   <select name="fx-filter-source" [(ngModel)]="filterSource"><option value="">Todas</option><option value="bch">BCH</option><option value="manual">Manual</option></select>
  </label>
  <button [disabled]="busy()">Consultar tasas</button>
 </form>
 <p>{{total()}} observaciones · Página {{page}}</p>
 <button type="button" [disabled]="page<=1||busy()" (click)="page=page-1;load()">Anterior tasa</button>
 <button type="button" [disabled]="page*25>=total()||busy()" (click)="page=page+1;load()">Más tasas</button>
 @for (x of rows(); track x.id) {
  <article class="card">
   <h3>{{kindLabel(x.rate_kind)}} · {{x.rate}} HNL por USD</h3>
   <p>Fuente: {{sourceLabel(x.source)}} · Vigente desde {{x.valid_from|date:'dd/MM/yyyy HH:mm':'-0600'}} (Honduras)</p>
   @if(x.valid_to){<p>Vigente hasta {{x.valid_to|date:'dd/MM/yyyy HH:mm':'-0600'}} (Honduras)</p>}
   <p>Referencia: {{x.source_reference}}</p>
   @if(x.source_published_at){<p>Publicada: {{x.source_published_at|date:'dd/MM/yyyy HH:mm':'-0600'}} (Honduras)</p>}
   @if(x.supersedes_id){<p>Corrige observación {{x.supersedes_id}}</p>}
   @if(x.superseded){<p><strong>Esta observación fue reemplazada por una corrección posterior.</strong></p>}
   @if(x.reason){<p>Motivo: {{x.reason}}</p>}
   @if(canManage()&&!x.superseded){<button type="button" (click)="prepareCorrection(x)" [disabled]="busy()">Preparar corrección de esta observación</button>}
  </article>
 } @empty {<p>No hay observaciones para estos filtros.</p>}
 @if(error()){<p role="alert">{{error()}}</p>}
 @if(notice()){<p role="status">{{notice()}}</p>}

 @if(canManage()) {
  <h3>{{supersedesId?'Registrar corrección':'Registrar observación'}}</h3>
  <p>El registro es append-only. Una corrección crea otra fila y no modifica la observación original.</p>
  <bastion-reauthentication/>
  <form (ngSubmit)="record()">
   <label>Tipo de tasa
    <select name="fx-kind" [(ngModel)]="rateKind" required><option value="">Seleccione</option><option value="tcr">TCR</option><option value="buy">Compra</option><option value="sell">Venta</option><option value="manual">Manual</option></select>
   </label>
   <label>Tasa HNL por USD<input name="fx-rate" [(ngModel)]="rate" inputmode="decimal" required placeholder="26.123456789012"></label>
   <label>Vigente desde (hora de Honduras)<input type="datetime-local" name="fx-from" [(ngModel)]="validFrom" required></label>
   <label>Vigente hasta (hora de Honduras, opcional)<input type="datetime-local" name="fx-to" [(ngModel)]="validTo"></label>
   <label>Fuente
    <select name="fx-source" [(ngModel)]="source" required><option value="">Seleccione</option><option value="bch">BCH</option><option value="manual">Manual</option></select>
   </label>
   <label>Referencia de fuente<input name="fx-reference" [(ngModel)]="sourceReference" required maxlength="1000"></label>
   <label>Publicada por la fuente (hora de Honduras, opcional)<input type="datetime-local" name="fx-published" [(ngModel)]="sourcePublishedAt"></label>
   <label>SHA-256 de evidencia (opcional)<input name="fx-hash" [(ngModel)]="evidenceHash" maxlength="64"></label>
   @if(supersedesId){<p>Corrige observación {{supersedesId}}. <button type="button" (click)="cancelCorrection()">Cancelar corrección</button></p>}
   <label>Motivo (obligatorio para tasa manual o corrección)<textarea name="fx-reason" [(ngModel)]="reason" maxlength="1000"></textarea></label>
   <button [disabled]="busy()||!rateKind||!rate||!validFrom||!source||!sourceReference">{{supersedesId?'Registrar corrección':'Registrar observación'}}</button>
  </form>
 }
</section>
}
`
})
export class QuoteFX implements OnInit{
 api=inject(Client);rows=signal<FXObservation[]>([]);total=signal(0);busy=signal(false);error=signal('');notice=signal('');page=1;filterKind='';filterSource='';
 rateKind='';rate='';validFrom='';validTo='';source='';sourceReference='';sourcePublishedAt='';evidenceHash='';supersedesId='';reason='';private pending:{body:FXInput;key:string}|null=null;
 ngOnInit(){if(this.canView())void this.load()}
 private global(permission:string){return this.api.profile()?.grants.some(g=>g.permission===permission&&g.scope==='all')||false}
 canView(){return this.global('cotizaciones.fx.view')}
 canManage(){return this.global('cotizaciones.fx.manage')}
 kindLabel(v:string){return ({tcr:'TCR',buy:'Compra',sell:'Venta',manual:'Manual'} as Record<string,string>)[v]||v}
 sourceLabel(v:string){return ({bch:'BCH',manual:'Manual'} as Record<string,string>)[v]||v}
 async load(reset=false){if(!this.canView())return;if(reset)this.page=1;this.busy.set(true);this.error.set('');try{const q=new URLSearchParams({page:String(this.page),per_page:'25'});if(this.filterKind)q.set('rate_kind',this.filterKind);if(this.filterSource)q.set('source',this.filterSource);const r=await this.api.get<FXPage>('/quote-fx/observations?'+q);this.rows.set(r.data);this.total.set(r.total)}catch(e){this.error.set(this.api.message(e))}finally{this.busy.set(false)}}
 private honduras(value:string){if(!value)return '';const normalized=value.length===16?value+':00-06:00':value+'-06:00',d=new Date(normalized);if(Number.isNaN(d.getTime()))throw new Error('Indique una fecha y hora válida de Honduras.');return d.toISOString()}
 private body():FXInput{return {rate_kind:this.rateKind,rate:this.rate.trim(),valid_from:this.honduras(this.validFrom),valid_to:this.honduras(this.validTo),source:this.source,source_reference:this.sourceReference.trim(),source_published_at:this.honduras(this.sourcePublishedAt),evidence_hash:this.evidenceHash.trim(),supersedes_id:this.supersedesId,reason:this.reason.trim()}}
 async record(){if(!this.canManage()||this.busy())return;let body:FXInput;try{body=this.body()}catch(e){this.error.set(this.api.message(e));return}const encoded=JSON.stringify(body);if(!this.pending||JSON.stringify(this.pending.body)!==encoded)this.pending={body,key:crypto.randomUUID()};this.busy.set(true);this.error.set('');this.notice.set('');try{const r=await this.api.write<{id:string}>('POST','/quote-fx/observations',this.pending.body,this.pending.key);this.pending=null;this.notice.set(`Observación ${r.id} registrada sin aplicar conversiones a cotizaciones.`);this.clearForm();await this.load(true)}catch(e){this.error.set(this.api.message(e))}finally{this.busy.set(false)}}
 prepareCorrection(x:FXObservation){if(!this.canManage()||x.superseded)return;this.rateKind=x.rate_kind;this.rate=x.rate;this.validFrom=this.localInput(x.valid_from);this.validTo=x.valid_to?this.localInput(x.valid_to):'';this.source=x.source;this.sourceReference=x.source_reference;this.sourcePublishedAt=x.source_published_at?this.localInput(x.source_published_at):'';this.evidenceHash=x.evidence_hash;this.supersedesId=x.id;this.reason='';this.pending=null}
 private localInput(value:string){const d=new Date(value),parts=new Intl.DateTimeFormat('en-CA',{timeZone:'America/Tegucigalpa',year:'numeric',month:'2-digit',day:'2-digit',hour:'2-digit',minute:'2-digit',hourCycle:'h23'}).formatToParts(d),get=(type:string)=>parts.find(p=>p.type===type)?.value||'';return `${get('year')}-${get('month')}-${get('day')}T${get('hour')}:${get('minute')}`}
 cancelCorrection(){this.supersedesId='';this.reason='';this.pending=null}
 private clearForm(){this.rateKind='';this.rate='';this.validFrom='';this.validTo='';this.source='';this.sourceReference='';this.sourcePublishedAt='';this.evidenceHash='';this.supersedesId='';this.reason=''}
}
