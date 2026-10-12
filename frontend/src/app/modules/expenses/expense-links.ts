import {Component,EventEmitter,Input,OnChanges,Output,inject,signal} from '@angular/core';
import {FormsModule} from '@angular/forms';
import {Client} from '../../core/api/client';

export interface ExpenseLinkTarget{id:string;version:string;deleted_at:string|null}
export interface ExpenseLinkChange{id:string;version:string;message:string}
interface Reference{entity_type:string;entity_id:string;entity_version:string;label:string;linked_by:string;linked_at:string}
interface DocumentRow{id:string;version:string;file_version:number;filename:string;media_type:string;size_bytes:number;checksum:string;status:string;linked_document_version:string;linked_by:string;linked_at:string}
interface DocumentPage{data:DocumentRow[];total:number;page:number;per_page:number}
interface ReferenceType{id:'processes'|'quotes'|'purchase_orders'|'invoices';label:string;module:string;path:string}
interface UploadStart{id:string;number:number;status:string;upload_url:string;checksum_base64:string}

const referenceTypes:ReferenceType[]=[
 {id:'processes',label:'Proceso',module:'processes',path:'/processes'},
 {id:'quotes',label:'Cotización',module:'quotes',path:'/quotes'},
 {id:'purchase_orders',label:'Orden de compra',module:'purchase_orders',path:'/purchase-orders'},
 {id:'invoices',label:'Factura',module:'invoices',path:'/invoices'},
];

@Component({
 selector:'bastion-expense-links',
 imports:[FormsModule],
 template:`<section aria-label="Referencias y comprobantes del gasto"><h3>Referencias y comprobantes</h3>
 @if(error()){<p role="alert">{{error()}}</p>}@if(message()){<p role="status">{{message()}}</p>}
 <section aria-label="Referencias documentales del gasto"><h4>Referencias documentales</h4>
 <p>Estas referencias son evidencia documental; no modifican costos, saldos, impuestos ni estados de otros módulos.</p>
 <ul>@for(r of references();track r.entity_type){<li>{{typeLabel(r.entity_type)}}: {{r.label}} · ID {{r.entity_id}} · versión capturada {{r.entity_version}}</li>}@empty{<li>Sin referencias documentales.</li>}</ul>
 @if(canWriteReferences()){
 <form (ngSubmit)="saveReferences()"><fieldset [disabled]="busy()||changesPending"><legend>Reemplazar referencias</legend>
 @for(t of types;track t.id){<label>{{t.label}} (ID)<input [name]="'reference-'+t.id" [(ngModel)]="referenceIds[t.id]" inputmode="numeric"></label>@if(referenceIds[t.id]&&!api.moduleEnabled(t.module)){<small>El módulo {{t.label}} está deshabilitado; quite esta referencia para guardar.</small>}}
 <label>Motivo del cambio<textarea name="referenceReason" [(ngModel)]="referenceReason" maxlength="1000" required></textarea></label>
 <button type="submit" [disabled]="!referenceReason.trim()">Guardar referencias</button></fieldset></form>
 }
 </section>
 <section aria-label="Comprobantes del gasto"><h4>Comprobantes</h4>
 @if(expense.deleted_at){<p>Los comprobantes están ocultos mientras el gasto permanezca en papelera.</p>}@else{
 <button type="button" (click)="loadDocuments()" [disabled]="busy()">Actualizar comprobantes</button>
 <p>{{documentTotal()}} comprobantes visibles · Página {{documentPage}}</p>
 <ul>@for(row of documents();track row.id){<li>{{row.filename}} · {{statusLabel(row.status)}} · {{row.media_type}} · versión {{row.version}}
 @if(row.status==='available'&&api.has('core.documents.read')){<button type="button" (click)="deliver(row,'preview')">Vista previa</button><button type="button" (click)="deliver(row,'download')">Descargar</button>}</li>}@empty{<li>No hay comprobantes visibles.</li>}</ul>
 <button type="button" (click)="documentPage=documentPage-1;loadDocuments()" [disabled]="busy()||documentPage<=1">Comprobantes anteriores</button><button type="button" (click)="documentPage=documentPage+1;loadDocuments()" [disabled]="busy()||documentPage*25>=documentTotal()">Más comprobantes</button>
 @if(canUpload()){
 <label>Archivo comprobante<input type="file" accept=".pdf,.png,.jpg,.jpeg,.txt" (change)="selectFile($event)" [disabled]="busy()||changesPending"></label>
 @if(file();as selected){<p>Seleccionado: {{selected.name}} · {{selected.size}} bytes</p>}
 <button type="button" (click)="uploadAndLink()" [disabled]="busy()||changesPending||!file()">Subir y vincular comprobante</button>
 @if(pendingDocument()){<button type="button" (click)="retryPendingLink()" [disabled]="busy()||changesPending">Reintentar vínculo del comprobante subido</button>}
 <p>Los archivos nuevos quedan en cuarentena hasta que el escáner los apruebe.</p>
 }
 }
 </section>
 @if(changesPending){<p>Guarde o recargue el gasto antes de modificar referencias o comprobantes.</p>}
 </section>`
})
export class ExpenseLinks implements OnChanges{
 @Input({required:true})expense!:ExpenseLinkTarget;
 @Input()changesPending=false;
 @Output()changed=new EventEmitter<ExpenseLinkChange>();
 api=inject(Client);types=referenceTypes;references=signal<Reference[]>([]);documents=signal<DocumentRow[]>([]);documentTotal=signal(0);busy=signal(false);error=signal('');message=signal('');file=signal<File|null>(null);pendingDocument=signal<{id:string;version:string;body:{version:string;document_id:string;document_version:string};key:string}|null>(null);referenceReason='';referenceIds:Record<string,string>={processes:'',quotes:'',purchase_orders:'',invoices:''};documentPage=1;
 private expenseKey='';private pendingKeys=new Map<string,{body:string;key:string}>();
 ngOnChanges(){if(!this.expense)return;const key=this.expense.id+':'+this.expense.version+':'+String(!!this.expense.deleted_at);if(key===this.expenseKey)return;this.expenseKey=key;this.error.set('');this.message.set('');this.referenceReason='';this.pendingKeys.clear();this.pendingDocument.set(null);void this.load()}
 typeLabel(kind:string){return this.types.find(x=>x.id===kind)?.label||kind}
 statusLabel(status:string){return ({pending:'Pendiente de carga',quarantined:'En cuarentena',available:'Disponible',rejected:'Rechazado'} as Record<string,string>)[status]||status}
 canWriteReferences(){return !this.expense.deleted_at&&!this.changesPending&&this.api.has('gastos.references.write')}
 canUpload(){return !this.expense.deleted_at&&!this.changesPending&&this.api.has('gastos.documents.write')&&this.api.has('core.documents.write')}
 private key(path:string,body:unknown){const serialized=JSON.stringify(body),old=this.pendingKeys.get(path);if(old?.body===serialized)return old.key;const key=crypto.randomUUID();this.pendingKeys.set(path,{body:serialized,key});return key}
 async load(){if(this.busy())return;this.busy.set(true);this.error.set('');try{await this.loadReferences(false);if(!this.expense.deleted_at)await this.loadDocuments(false)}catch(e){this.error.set(this.api.message(e))}finally{this.busy.set(false)}}
 async loadReferences(manageBusy=true){if(manageBusy){if(this.busy())return;this.busy.set(true);this.error.set('')}try{const rows=await this.api.get<Reference[]>(`/expenses/${this.expense.id}/references`);this.references.set(rows);for(const t of this.types)this.referenceIds[t.id]=rows.find(r=>r.entity_type===t.id)?.entity_id||''}catch(e){if(manageBusy)this.error.set(this.api.message(e));else throw e}finally{if(manageBusy)this.busy.set(false)}}
 async loadDocuments(manageBusy=true){if(this.expense.deleted_at)return;if(manageBusy){if(this.busy())return;this.busy.set(true);this.error.set('')}try{const page=await this.api.get<DocumentPage>(`/expenses/${this.expense.id}/documents?page=${this.documentPage}&per_page=25`);this.documents.set(page.data);this.documentTotal.set(page.total);this.documentPage=page.page}catch(e){if(manageBusy)this.error.set(this.api.message(e));else throw e}finally{if(manageBusy)this.busy.set(false)}}
 private async resolveReference(t:ReferenceType,id:string){if(!this.api.moduleEnabled(t.module))throw new Error(`El módulo ${t.label} está deshabilitado.`);const row=await this.api.get<{version:string}>(`${t.path}/${encodeURIComponent(id)}`);if(!row.version)throw new Error(`No se pudo resolver la versión de ${t.label}.`);return {entity_type:t.id,entity_id:id,entity_version:row.version}}
 async saveReferences(){if(this.busy()||!this.canWriteReferences()||!this.referenceReason.trim())return;this.busy.set(true);this.error.set('');this.message.set('');try{const references=[];for(const t of this.types){const id=this.referenceIds[t.id].trim();if(id)references.push(await this.resolveReference(t,id))}const body={version:this.expense.version,reason:this.referenceReason.trim(),references};const path=`/expenses/${this.expense.id}/references`;const key=this.key(path,body);const r=await this.api.write<{id:string;version:string}>('POST',path,body,key);this.pendingKeys.delete(path);this.message.set('Referencias actualizadas.');this.changed.emit({...r,message:'Referencias del gasto actualizadas.'})}catch(e){this.error.set(this.api.message(e))}finally{this.busy.set(false)}}
 selectFile(event:Event){const input=event.target as HTMLInputElement;this.file.set(input.files?.item(0)||null);this.pendingDocument.set(null);this.error.set('');this.message.set('')}
 private media(file:File){const allowed=new Set(['application/pdf','image/png','image/jpeg','text/plain']);if(allowed.has(file.type))return file.type;const ext=file.name.toLowerCase().split('.').pop();return ({pdf:'application/pdf',png:'image/png',jpg:'image/jpeg',jpeg:'image/jpeg',txt:'text/plain'} as Record<string,string>)[ext||'']||''}
 private async digest(file:File){const hash=new Uint8Array(await crypto.subtle.digest('SHA-256',await file.arrayBuffer()));const hex=Array.from(hash,b=>b.toString(16).padStart(2,'0')).join('');let binary='';for(const b of hash)binary+=String.fromCharCode(b);return {hex,base64:btoa(binary)}}
 async uploadAndLink(){const selected=this.file();if(!selected||this.busy()||!this.canUpload())return;if(selected.size<1||selected.size>20*1024*1024){this.error.set('El comprobante debe pesar entre 1 byte y 20 MB.');return}const media=this.media(selected);if(!media){this.error.set('Tipo de archivo no permitido.');return}this.busy.set(true);this.error.set('');this.message.set('');try{const checksum=await this.digest(selected);const createBody={filename:selected.name,media_type:media,size_bytes:selected.size,checksum:checksum.hex,version:'0'};const createPath='/documents';const createKey=this.key(createPath+':'+selected.name+':'+checksum.hex,createBody);const upload=await this.api.write<UploadStart>('POST',createPath,createBody,createKey);const put=await fetch(upload.upload_url,{method:'PUT',headers:{'Content-Type':media,'x-amz-checksum-sha256':upload.checksum_base64||checksum.base64},body:selected});if(!put.ok)throw new Error('La carga del comprobante a almacenamiento privado falló.');await this.api.write('POST',`/documents/${upload.id}/finalize`,{});const body={version:this.expense.version,document_id:upload.id,document_version:'1'};const path=`/expenses/${this.expense.id}/documents`;const key=this.key(path+':'+upload.id,body);this.pendingDocument.set({id:upload.id,version:'1',body,key});await this.linkPending(path);this.file.set(null)}catch(e){this.error.set(this.api.message(e))}finally{this.busy.set(false)}}
 async retryPendingLink(){if(this.busy()||!this.canUpload()||!this.pendingDocument())return;this.busy.set(true);this.error.set('');try{await this.linkPending(`/expenses/${this.expense.id}/documents`)}catch(e){this.error.set(this.api.message(e))}finally{this.busy.set(false)}}
 private async linkPending(path:string){const pending=this.pendingDocument();if(!pending)return;const r=await this.api.write<{id:string;version:string}>('POST',path,pending.body,pending.key);this.pendingKeys.delete(path+':'+pending.id);this.pendingDocument.set(null);this.message.set('Comprobante vinculado; permanecerá en cuarentena hasta el escaneo.');this.changed.emit({...r,message:'Comprobante del gasto vinculado.'})}
 async deliver(row:DocumentRow,action:'preview'|'download'){if(row.status!=='available'||!this.api.has('core.documents.read'))return;this.error.set('');try{const r=await this.api.get<{url:string}>(`/documents/${row.id}/${action}`);window.open(r.url,'_blank','noopener,noreferrer')}catch(e){this.error.set(this.api.message(e))}}
}
