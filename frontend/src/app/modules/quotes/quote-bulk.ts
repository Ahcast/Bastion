import {Component,EventEmitter,Input,OnChanges,Output,inject,signal} from '@angular/core';
import {FormsModule} from '@angular/forms';
import {Client} from '../../core/api/client';

export interface QuoteBulkRow{id:string;number:string;customer_name:string;reference:string;date:string;currency:string;total:string;status:string;version:string}
interface BulkItemResult{position:number;id:string;version:string;status:string;error_code?:string}
interface BulkResponse{batch_id:string;action:string;status:string;results:BulkItemResult[];filename?:string;content?:string}
type BulkAction='status'|'clone'|'export';
interface BulkRequest{action:BulkAction;body:{items:{id:string;version:string}[];status?:string};key:string}

@Component({selector:'bastion-quote-bulk',imports:[FormsModule],template:`
@if(rows.length||pending()||result()){
<section class="card" aria-labelledby="quote-bulk-title">
<h2 id="quote-bulk-title">Acciones en lote</h2>
@if(disabled){<p>Cierre la ficha para usar acciones en lote sobre las versiones guardadas.</p>}
<p>Seleccione cotizaciones visibles. Cada lote conserva identidad y resultado por cotización.</p>
<label><input type="checkbox" [checked]="allSelected()" (change)="toggleAll($event)" [disabled]="locked()">Seleccionar todas las cotizaciones visibles</label>
<details><summary>Elegir cotizaciones ({{count()}} seleccionadas)</summary>
@for(q of rows;track q.id){<label><input type="checkbox" [checked]="selected().has(q.id)" (change)="toggle(q.id,$event)" [disabled]="locked()">{{q.number}} · {{q.customer_name}} · versión {{q.version}}</label>}
</details>
@if(api.has('cotizaciones.transition')){<label>Estado de destino<select [(ngModel)]="target" [disabled]="locked()"><option value="draft">Borrador</option><option value="costing">Costeo</option><option value="review">Revisión</option></select></label><button type="button" (click)="run('status')" [disabled]="locked()||count()===0">Cambiar estado de seleccionadas</button>}
@if(api.has('cotizaciones.create')){<button type="button" (click)="run('clone')" [disabled]="locked()||count()===0">Clonar seleccionadas</button>}
@if(api.has('cotizaciones.view')){<button type="button" (click)="run('export')" [disabled]="locked()||count()===0">Exportar seleccionadas a CSV</button>}
@if(pending()){<p>Hay una solicitud sin confirmar. Reintentar conserva las cotizaciones y versiones originales.</p><button type="button" (click)="retry()" [disabled]="busy()||disabled">Reintentar lote</button>}
@if(error()){<p role="alert">{{error()}}</p>}
@if(notice()){<p role="status">{{notice()}}</p>}
@if(result();as batch){
<p>Lote {{batch.batch_id}} · {{batch.status==='completed'?'Completado':'En proceso'}}</p>
@if(batch.content){<button type="button" (click)="download(batch)">Descargar CSV del lote {{batch.batch_id}}</button>}
@if(failures().length){<ul>@for(item of failures();track item.position){<li>{{number(item.id)}}: {{errorLabel(item.error_code||'')}}</li>}</ul>}
}
</section>
}
`})
export class QuoteBulk implements OnChanges{
 api=inject(Client);
 @Input() rows:QuoteBulkRow[]=[];
 @Input() disabled=false;
 @Output() changed=new EventEmitter<void>();
 selected=signal<Set<string>>(new Set());busy=signal(false);error=signal('');notice=signal('');result=signal<BulkResponse|null>(null);target='costing';pending=signal<BulkRequest|null>(null);
 ngOnChanges(){this.selected.set(new Set())}
 count(){return this.selected().size}
 locked(){return this.busy()||this.disabled||!!this.pending()}
 allSelected(){return this.rows.length>0&&this.rows.every(q=>this.selected().has(q.id))}
 private checked(event:Event){return (event.target as HTMLInputElement).checked}
 toggle(id:string,event:Event){if(this.locked())return;const next=new Set(this.selected());if(this.checked(event))next.add(id);else next.delete(id);this.selected.set(next)}
 toggleAll(event:Event){if(this.locked())return;this.selected.set(this.checked(event)?new Set(this.rows.map(q=>q.id)):new Set())}
 private items(){const ids=this.selected();return this.rows.filter(q=>ids.has(q.id)).map(q=>({id:q.id,version:q.version}))}
 failures(){return this.result()?.results.filter(x=>x.status!=='success')||[]}
 number(id:string){return this.rows.find(q=>q.id===id)?.number||id}
 errorLabel(code:string){return ({validation:'Datos o transición inválidos',conflict:'La cotización cambió; recargue la lista',not_found:'Cotización no disponible',forbidden:'Sin permiso vigente'} as Record<string,string>)[code]||'No se pudo completar'}
 download(r:BulkResponse){if(!r.content)return;try{const blob=new Blob([r.content],{type:'text/csv;charset=utf-8'}),url=URL.createObjectURL(blob),a=document.createElement('a');a.href=url;a.download=r.filename||`cotizaciones-${r.batch_id}.csv`;a.rel='noopener';document.body.appendChild(a);a.click();a.remove();setTimeout(()=>URL.revokeObjectURL(url),0)}catch{this.error.set('El lote está confirmado; vuelva a descargar el CSV.')}}
 async run(action:BulkAction){
  if(this.busy()||this.disabled)return;
  const existing=this.pending();if(existing){if(existing.action===action)await this.retry();return}
  const items=this.items();if(!items.length)return;
  this.pending.set({action,body:action==='status'?{items,status:this.target}:{items},key:crypto.randomUUID()});
  await this.retry();
 }
 async retry(){
  const request=this.pending();if(!request||this.busy()||this.disabled)return;
  this.busy.set(true);this.error.set('');this.notice.set('');
  try{
   const r=await this.api.write<BulkResponse>('POST',`/quotes/bulk/${request.action}`,request.body,request.key);
   this.result.set(r);
   if(r.status!=='completed'){this.notice.set(`Lote ${r.batch_id} en proceso. Reintente para consultar el resultado.`);return}
   this.pending.set(null);
   const failed=r.results.filter(x=>x.status!=='success').length,ok=r.results.length-failed;
   this.notice.set(`Lote ${r.batch_id}: ${ok} correctas, ${failed} con error.`);
   this.selected.set(new Set());
   if(request.action==='export')this.download(r);else this.changed.emit();
  }catch(e){this.error.set(this.api.message(e))}finally{this.busy.set(false)}
 }
}
