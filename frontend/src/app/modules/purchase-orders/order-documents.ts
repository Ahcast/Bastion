import { Component, effect, inject, input, output, signal } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { Client } from '../../core/api/client';
import { Reauthentication } from '../../core/security/reauthentication';
import type { PurchaseOrder } from './purchase-orders';
interface Document { id:string; version:string; filename:string; status:string; item_id:string|null; item_position:number|null }
@Component({
 selector:'app-order-documents', imports:[FormsModule,Reauthentication],
 template:`<section aria-label="Documentos privados de la orden"><h3>Documentos privados</h3>
 @if(error()){<p role="alert">{{error()}}</p>}
 <label>Documentos del ítem<select [(ngModel)]="filterItem" (change)="page=1;load()"><option value="">Todos</option>@for(item of order().items;track item.id){<option [value]="item.id">Ítem {{$index+1}}: {{item.description}}</option>}</select></label>
 <label><input type="checkbox" [(ngModel)]="trash" (change)="page=1;load()">Papelera de documentos</label>
 <button (click)="load()" [disabled]="busy()">Consultar documentos</button>
 @for(d of documents();track d.id){<p>{{d.filename}} · {{d.item_position ? 'Ítem '+d.item_position : 'Cabecera'}} · {{status(d.status)}}
 @if(!trash&&d.status==='available'){<button (click)="deliver(d.id,'preview')">Vista previa {{d.filename}}</button><button (click)="deliver(d.id,'download')">Descargar {{d.filename}}</button>}
 @if(writable()){<button (click)="remove(d)" [disabled]="busy()||changesPending()">{{trash?'Restaurar':'Eliminar'}} documento {{d.filename}}</button>}</p>}
 <button (click)="page=page-1;load()" [disabled]="busy()||page<=1">Documentos anteriores</button><button (click)="page=page+1;load()" [disabled]="busy()||page*25>=total()">Más documentos</button>
 @if(writable()){
 <button (click)="showReauth=!showReauth">Confirmar identidad para documentos</button>@if(showReauth){<bastion-reauthentication />}
 <p>Guarde o descarte las ediciones antes de adjuntar. La carga permanece en cuarentena hasta comprobar integridad y análisis de seguridad.</p>
 <label>Destino del adjunto<select [(ngModel)]="target"><option value="">Cabecera</option>@for(item of order().items;track item.id){<option [value]="item.id">Ítem {{$index+1}}: {{item.description}}</option>}</select></label>
 <label>Archivo de la orden<input type="file" accept=".pdf,.png,.jpg,.jpeg,.txt" (change)="selectFile($event)" [disabled]="busy()"></label>
 <button (click)="upload()" [disabled]="busy()||changesPending()||!file">Subir documento de la orden</button>
 <label>ID de documento existente<input [(ngModel)]="documentId"></label><label>Versión del documento<input [(ngModel)]="documentVersion"></label>
 <button (click)="link()" [disabled]="busy()||changesPending()||!documentId">Vincular documento a la orden</button>
 }</section>`
})
export class OrderDocuments {
 api=inject(Client); order=input.required<PurchaseOrder>(); changesPending=input(false); linked=output<{id:string;version:string}>();
 documents=signal<Document[]>([]); total=signal(0); busy=signal(false); error=signal('');
 showReauth=false; page=1; trash=false; filterItem=''; target=''; documentId=''; documentVersion='1'; file:File|null=null;
 private pending=new Map<string,{payload:string;key:string}>();
 private uploadState:{file:File;id:string;stage:'created'|'uploaded'|'verified'}|null=null;
 private orderId='';
 constructor(){effect(()=>{const id=this.order().id;if(id!==this.orderId){this.orderId=id;this.documents.set([]);this.total.set(0);this.page=1;this.trash=false;this.filterItem='';this.target='';this.file=null;this.uploadState=null;this.documentId='';this.pending.clear();this.error.set('')}})}
 writable(){return this.api.has('ordenes.documents.write')&&this.api.has('core.documents.write')&&!['invoiced','closed'].includes(this.order().status)}
 status(s:string){return ({pending:'Carga pendiente',quarantined:'En cuarentena',available:'Disponible',rejected:'Rechazado'} as Record<string,string>)[s]||s}
 private key(path:string,body:unknown){const payload=JSON.stringify(body),old=this.pending.get(path);if(old?.payload===payload)return old.key;const key=crypto.randomUUID();this.pending.set(path,{payload,key});return key}
 async load(){const id=this.order().id;try{const r=await this.api.get<{data:Document[];total:number}>(`/purchase-orders/${this.order().id}/documents?page=${this.page}&per_page=25&deleted=${this.trash}&item_id=${encodeURIComponent(this.filterItem)}`);if(this.order().id===id){this.documents.set(r.data);this.total.set(r.total)}}catch(e){this.error.set(this.api.message(e))}}
 selectFile(e:Event){this.file=(e.target as HTMLInputElement).files?.[0]||null;if(this.file!==this.uploadState?.file)this.uploadState=null}
 async link(){if(this.busy()||this.changesPending())return;this.busy.set(true);this.error.set('');try{await this.bind()}catch(e){this.error.set(this.api.message(e))}finally{this.busy.set(false)}}
 private async bind(){const id=this.order().id;const path=`/purchase-orders/${this.order().id}/documents`,body={document_id:this.documentId,document_version:this.documentVersion,version:this.order().version,item_id:this.target||null};const r=await this.api.write<{version:string}>('POST',path,body,this.key(path,body));this.pending.delete(path);if(this.order().id===id){this.linked.emit({id,version:r.version});await this.load()}}
 async upload(){const id=this.order().id;const f=this.file;if(!f||this.busy()||this.changesPending())return;this.busy.set(true);this.error.set('');try{
 if(f.size<1||f.size>20*1024*1024)throw new Error('Archivo debe tener entre 1 byte y 20 MB.');
 const buffer=await f.arrayBuffer(),hash=await crypto.subtle.digest('SHA-256',buffer),checksum=Array.from(new Uint8Array(hash)).map(x=>x.toString(16).padStart(2,'0')).join('');
 const media=f.type||({pdf:'application/pdf',png:'image/png',jpg:'image/jpeg',jpeg:'image/jpeg',txt:'text/plain'} as Record<string,string>)[f.name.split('.').pop()?.toLowerCase()||''];
 if(this.order().id!==id)return;
 const body={filename:f.name,media_type:media,size_bytes:f.size,checksum,version:'0'},op='order-upload:'+this.order().id;
 if(!this.uploadState||this.uploadState.stage==='created'){const r=await this.api.write<{id:string;upload_url:string;checksum_base64:string}>('POST','/documents',body,this.key(op,body));this.uploadState={file:f,id:r.id,stage:'created'};const response=await fetch(r.upload_url,{method:'PUT',credentials:'omit',headers:{'Content-Type':media,'x-amz-checksum-sha256':r.checksum_base64},body:buffer});if(!response.ok)throw new Error('No se confirmó la carga privada.');this.uploadState.stage='uploaded'}
 if(this.order().id!==id)return;
 if(this.uploadState.stage==='uploaded'){await this.api.write('POST',`/documents/${this.uploadState.id}/finalize`);this.uploadState.stage='verified'}
 if(this.order().id!==id)return;
 this.documentId=this.uploadState.id;this.documentVersion='1';await this.bind();this.pending.delete(op);this.uploadState=null;this.file=null;
 }catch(e){this.error.set(this.api.message(e))}finally{this.busy.set(false)}}
 async remove(d:Document){if(this.busy()||this.changesPending())return;this.busy.set(true);this.error.set('');const path=`/documents/${d.id}`+(this.trash?'/restore':''),body={version:d.version};try{await this.api.write(this.trash?'POST':'DELETE',path,body,this.key(path,body));this.pending.delete(path);await this.load()}catch(e){this.error.set(this.api.message(e))}finally{this.busy.set(false)}}
 async deliver(id:string,action:'preview'|'download'){try{const r=await this.api.get<{url:string}>(`/documents/${id}/${action}`);window.open(r.url,'_blank','noopener,noreferrer')}catch(e){this.error.set(this.api.message(e))}}
}
