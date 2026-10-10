import {Component,EventEmitter,Input,OnChanges,Output,inject,signal} from '@angular/core';
import {FormsModule} from '@angular/forms';
import {Client} from '../../core/api/client';

export interface QuoteBulkRow{id:string;number:string;customer_name:string;reference:string;date:string;currency:string;total:string;status:string;version:string}
interface BulkItemResult{position:number;id:string;version:string;status:string;error_code?:string}
interface BulkResponse{batch_id:string;action:string;status:string;results:BulkItemResult[];filename?:string;content?:string}

@Component({
 selector:'bastion-quote-bulk',
 imports:[FormsModule],
 template:`
@if (rows.length && (api.has('cotizaciones.transition') || api.has('cotizaciones.create') || api.has('cotizaciones.view'))) {
  <section class="card" aria-labelledby="quote-bulk-title">
    <h2 id="quote-bulk-title">Acciones en lote</h2>
    <p>Seleccione cotizaciones visibles. Cada lote conserva identidad y resultado por cotización.</p>

    <label>
      <input type="checkbox" [checked]="allSelected()" (change)="toggleAll($event)" [disabled]="busy()">
      Seleccionar todas las cotizaciones visibles
    </label>

    <details>
      <summary>Elegir cotizaciones ({{count()}} seleccionadas)</summary>
      @for (q of rows; track q.id) {
        <label>
          <input type="checkbox" [checked]="selected().has(q.id)" (change)="toggle(q.id,$event)" [disabled]="busy()">
          {{q.number}} · {{q.customer_name}} · versión {{q.version}}
        </label>
      }
    </details>

    @if (api.has('cotizaciones.transition')) {
      <label>
        Estado de destino
        <select [(ngModel)]="target" [disabled]="busy()">
          <option value="draft">Borrador</option>
          <option value="costing">Costeo</option>
          <option value="review">Revisión</option>
        </select>
      </label>
      <button type="button" (click)="run('status')" [disabled]="busy() || count() === 0">Cambiar estado de seleccionadas</button>
    }

    @if (api.has('cotizaciones.create')) {
      <button type="button" (click)="run('clone')" [disabled]="busy() || count() === 0">Clonar seleccionadas</button>
    }

    @if (api.has('cotizaciones.view')) {
      <button type="button" (click)="run('export')" [disabled]="busy() || count() === 0">Exportar seleccionadas a CSV</button>
    }

    @if (error()) {
      <p role="alert">{{error()}}</p>
    }
    @if (notice()) {
      <p role="status">{{notice()}}</p>
    }
    @if (result(); as batch) {
      <p>Lote {{batch.batch_id}} · {{batch.status}}</p>
      @if (failures().length) {
        <ul>
          @for (item of failures(); track item.position) {
            <li>{{number(item.id)}}: {{errorLabel(item.error_code || '')}}</li>
          }
        </ul>
      }
    }
  </section>
}
`
})
export class QuoteBulk implements OnChanges{
 api=inject(Client);
 @Input() rows:QuoteBulkRow[]=[];
 @Output() changed=new EventEmitter<void>();
 selected=signal<Set<string>>(new Set());busy=signal(false);error=signal('');notice=signal('');result=signal<BulkResponse|null>(null);target='costing';private pending=new Map<string,{body:string;key:string}>();
 ngOnChanges(){this.selected.set(new Set())}
 count(){return this.selected().size}
 allSelected(){return this.rows.length>0&&this.rows.every(q=>this.selected().has(q.id))}
 private checked(event:Event){return (event.target as HTMLInputElement).checked}
 toggle(id:string,event:Event){const next=new Set(this.selected());if(this.checked(event))next.add(id);else next.delete(id);this.selected.set(next)}
 toggleAll(event:Event){this.selected.set(this.checked(event)?new Set(this.rows.map(q=>q.id)):new Set())}
 private items(){const ids=this.selected();return this.rows.filter(q=>ids.has(q.id)).map(q=>({id:q.id,version:q.version}))}
 private key(path:string,body:unknown){const encoded=JSON.stringify(body),old=this.pending.get(path);if(old?.body===encoded)return old.key;const key=crypto.randomUUID();this.pending.set(path,{body:encoded,key});return key}
 failures(){return this.result()?.results.filter(x=>x.status!=='success')||[]}
 number(id:string){return this.rows.find(q=>q.id===id)?.number||id}
 errorLabel(code:string){return ({invalid:'Datos o transición inválidos',conflict:'La cotización cambió; recargue la lista',not_found:'Cotización no disponible',forbidden:'Sin permiso vigente'} as Record<string,string>)[code]||code||'No se pudo completar'}
 private download(r:BulkResponse){if(!r.content)return;const blob=new Blob([r.content],{type:'text/csv;charset=utf-8'}),url=URL.createObjectURL(blob),a=document.createElement('a');a.href=url;a.download=r.filename||`cotizaciones-${r.batch_id}.csv`;a.rel='noopener';a.click();URL.revokeObjectURL(url)}
 async run(action:'status'|'clone'|'export'){const items=this.items();if(this.busy()||!items.length)return;const path=`/quotes/bulk/${action}`,body=action==='status'?{items,status:this.target}:{items};this.busy.set(true);this.error.set('');this.notice.set('');try{const r=await this.api.write<BulkResponse>('POST',path,body,this.key(path,body));this.pending.delete(path);this.result.set(r);const failed=r.results.filter(x=>x.status!=='success').length,ok=r.results.length-failed;this.notice.set(`Lote ${r.batch_id}: ${ok} correctas, ${failed} con error.`);if(action==='export')this.download(r);else this.changed.emit();this.selected.set(new Set())}catch(e){this.error.set(this.api.message(e))}finally{this.busy.set(false)}}
}
