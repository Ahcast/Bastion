import {Component,EventEmitter,Input,OnChanges,Output,inject,signal} from '@angular/core';
import {RouterLink} from '@angular/router';
import {HttpErrorResponse} from '@angular/common/http';
import {FormsModule} from '@angular/forms';
import {Client} from '../../core/api/client';

export interface ExpenseLinkTarget{id:string;version:string;deleted_at:string|null}
export interface ExpenseLinkChange{id:string;version:string;message:string}
interface Reference{entity_type:string;entity_id:string;entity_version:string;label:string;linked_by:string;linked_at:string}
interface DocumentRow{id:string;version:string;file_version:number;filename:string;media_type:string;size_bytes:number;checksum:string;status:string;linked_document_version:string;linked_by:string;linked_at:string}
interface DocumentPage{data:DocumentRow[];total:number;page:number;per_page:number}
interface ReferenceType{id:'processes'|'quotes'|'purchase_orders'|'invoices';label:string;module:string;path:string;permission:string}
interface UploadStart{id:string;number:number;status:string;upload_url:string;checksum_base64:string}

const referenceTypes:ReferenceType[]=[
 {id:'processes',label:'Proceso',module:'processes',path:'/processes',permission:'procesos.view'},
 {id:'quotes',label:'Cotización',module:'quotes',path:'/quotes',permission:'cotizaciones.view'},
 {id:'purchase_orders',label:'Orden de compra',module:'purchase-orders',path:'/purchase-orders',permission:'ordenes.view'},
 {id:'invoices',label:'Factura',module:'invoices',path:'/invoices',permission:'facturas.view'},
];

@Component({
 selector:'bastion-expense-links',
 imports:[FormsModule,RouterLink],
 template:`<section aria-label="Referencias y comprobantes del gasto"><h3>Referencias y comprobantes</h3>
 @if(error()){<p role="alert">{{error()}}</p>}@if(message()){<p role="status">{{message()}}</p>}
 <section aria-label="Referencias documentales del gasto"><h4>Referencias documentales</h4>
 <p>Estas referencias son evidencia documental; no modifican costos, saldos, impuestos ni estados de otros módulos.</p>
 <ul>@for(r of references();track r.entity_type){<li><span>{{typeLabel(r.entity_type)}}: {{r.label}} · ID {{r.entity_id}} · versión capturada {{r.entity_version}}</span> @if(canOpen(r.entity_type)){<a [routerLink]="sourcePath(r.entity_type)" [queryParams]="{id:r.entity_id}">Abrir {{typeLabel(r.entity_type)}}</a>}</li>}@empty{<li>Sin referencias documentales.</li>}</ul>
 @if(canWriteReferences()){
 <form (ngSubmit)="saveReferences()"><fieldset [disabled]="busy()||disabled||changesPending||!!pendingReferences()"><legend>Reemplazar referencias</legend>
 @for(t of types;track t.id){<label>{{t.label}} (ID)<input [name]="'reference-'+t.id" [(ngModel)]="referenceIds[t.id]" inputmode="numeric"></label>@if(referenceIds[t.id]&&!api.moduleEnabled(t.module)){<small>El módulo {{t.label}} está deshabilitado; quite esta referencia para guardar.</small>}}
 <label>Motivo del cambio<textarea name="referenceReason" [(ngModel)]="referenceReason" maxlength="1000" required></textarea></label>
 <button type="submit" [disabled]="!referenceReason.trim()">Guardar referencias</button></fieldset></form>
 }
 </section>
 @if(pendingReferences()){<button type="button" (click)="saveReferences()" [disabled]="busy()||disabled||changesPending">Reintentar referencias pendientes</button>}
 <section aria-label="Comprobantes del gasto"><h4>Comprobantes</h4>
 @if(expense.deleted_at){<p>Los comprobantes están ocultos mientras el gasto permanezca en papelera.</p>}@else{
 @if(api.has('core.documents.read')){<label><input type="checkbox" [(ngModel)]="documentTrash" [disabled]="busy()||disabled" (change)="documentPage=1;loadDocuments()">Papelera de comprobantes</label>
 <button type="button" (click)="loadDocuments()" [disabled]="busy()||disabled">Actualizar comprobantes</button>
 <p>{{documentTotal()}} comprobantes visibles · Página {{documentPage}}</p>
 <ul>@for(row of documents();track row.id){<li>{{row.filename}} · {{statusLabel(row.status)}} · {{row.media_type}} · versión {{row.version}}
 @if(!documentTrash&&row.status==='available'&&api.has('core.documents.read')){<button type="button" (click)="deliver(row,'preview')">Vista previa</button><button type="button" (click)="deliver(row,'download')">Descargar</button>}@if(canUpload()){<button type="button" (click)="changeDocumentState(row)" [disabled]="busy()||disabled||changesPending">{{documentTrash?'Restaurar':'Eliminar'}} comprobante {{row.filename}}</button>}</li>}@empty{<li>No hay comprobantes visibles.</li>}</ul>
 <button type="button" (click)="documentPage=documentPage-1;loadDocuments()" [disabled]="busy()||documentPage<=1">Comprobantes anteriores</button><button type="button" (click)="documentPage=documentPage+1;loadDocuments()" [disabled]="busy()||documentPage*25>=documentTotal()">Más comprobantes</button>
 }@else{<p>No tiene permiso para consultar comprobantes.</p>}
 @if(canUpload()){
 <label>Archivo comprobante<input type="file" accept=".pdf,.png,.jpg,.jpeg,.txt" (change)="selectFile($event)" [disabled]="busy()||disabled||changesPending||!!pendingDocument()"></label>
 @if(file();as selected){<p>Seleccionado: {{selected.name}} · {{selected.size}} bytes</p>}
 <button type="button" (click)="uploadAndLink()" [disabled]="busy()||disabled||changesPending||!file()">Subir y vincular comprobante</button>
 @if(pendingDocument()){<button type="button" (click)="retryPendingLink()" [disabled]="busy()||disabled||changesPending">Reintentar vínculo del comprobante subido</button>}
 <label>Documento existente (ID)<input [(ngModel)]="existingDocumentId" [disabled]="busy()||disabled||changesPending||!!pendingDocument()"></label><label>Versión del documento existente<input [(ngModel)]="existingDocumentVersion" [disabled]="busy()||disabled||changesPending||!!pendingDocument()"></label><button type="button" (click)="linkExisting()" [disabled]="busy()||disabled||changesPending||!existingDocumentId">Vincular comprobante existente</button>
 <p>Los archivos nuevos quedan en cuarentena hasta que el escáner los apruebe.</p>
 }
 }
 </section>
 @if(changesPending){<p>Guarde o recargue el gasto antes de modificar referencias o comprobantes.</p>}
 </section>`
})
export class ExpenseLinks implements OnChanges{
 @Input({required:true})expense!:ExpenseLinkTarget;
 @Input()changesPending=false;@Input()disabled=false;
 @Output()changed=new EventEmitter<ExpenseLinkChange>();@Output()busyChange=new EventEmitter<boolean>();
 api=inject(Client);types=referenceTypes;references=signal<Reference[]>([]);documents=signal<DocumentRow[]>([]);documentTotal=signal(0);busy=signal(false);confirmed=signal(false);error=signal('');message=signal('');file=signal<File|null>(null);pendingDocument=signal<{id:string;version:string;intent:string;body:{version:string;document_id:string;document_version:string};key:string}|null>(null);pendingReferences=signal<{id:string;body:unknown;key:string;path:string}|null>(null);referenceReason='';referenceIds:Record<string,string>={processes:'',quotes:'',purchase_orders:'',invoices:''};documentPage=1;documentTrash=false;existingDocumentId='';existingDocumentVersion='1';
 private expenseKey='';private identity='';private reloadRequested=false;private pendingKeys=new Map<string,{body:string;key:string}>();private uploadState:{file:File;id:string;stage:'created'|'uploaded'|'verified'}|null=null;
 ngOnChanges(){if(!this.expense)return;const key=this.expense.id+':'+this.expense.version+':'+String(!!this.expense.deleted_at);if(this.identity!==this.expense.id){this.identity=this.expense.id;this.references.set([]);this.documents.set([]);this.documentTotal.set(0);this.documentPage=1;this.documentTrash=false;this.referenceIds={processes:'',quotes:'',purchase_orders:'',invoices:''};this.file.set(null);this.pendingKeys.clear();this.pendingDocument.set(null);this.pendingReferences.set(null);this.uploadState=null;this.existingDocumentId=''}if(key!==this.expenseKey){this.expenseKey=key;this.confirmed.set(false);this.error.set('');this.message.set('');this.referenceReason='';this.reloadRequested=true}if(this.reloadRequested&&!this.busy()&&!this.disabled)void this.load()}
 private setBusy(v:boolean){this.busy.set(v);this.busyChange.emit(v);if(!v&&this.reloadRequested&&!this.disabled)void this.load()}
 typeLabel(kind:string){return this.types.find(x=>x.id===kind)?.label||kind}sourcePath(kind:string){return this.types.find(x=>x.id===kind)?.path||''}canOpen(kind:string){const t=this.types.find(x=>x.id===kind);return !!t&&this.api.moduleEnabled(t.module)&&this.api.has(t.permission)}
 statusLabel(status:string){return ({pending:'Pendiente de carga',quarantined:'En cuarentena',available:'Disponible',rejected:'Rechazado'} as Record<string,string>)[status]||status}
 canWriteReferences(){return !this.disabled&&!this.confirmed()&&!this.expense.deleted_at&&!this.changesPending&&this.api.has('gastos.references.write')&&this.api.has('gastos.view')}
 canUpload(){return !this.disabled&&!this.confirmed()&&!this.expense.deleted_at&&!this.changesPending&&this.api.has('gastos.documents.write')&&this.api.has('core.documents.write')&&this.api.has('gastos.view')}
 private key(path:string,body:unknown){const serialized=JSON.stringify(body),old=this.pendingKeys.get(path);if(old?.body===serialized)return old.key;const key=crypto.randomUUID();this.pendingKeys.set(path,{body:serialized,key});return key}
 private definitive(e:unknown){return e instanceof HttpErrorResponse&&e.status>=400&&e.status<500&&e.status!==408}
 async load(){if(this.busy()||this.disabled){this.reloadRequested=true;return}this.reloadRequested=false;this.setBusy(true);this.error.set('');try{await this.loadReferences(false);if(!this.expense.deleted_at&&this.api.has('core.documents.read'))await this.loadDocuments(false)}catch(e){this.error.set(this.api.message(e))}finally{this.setBusy(false)}}
 async loadReferences(manageBusy=true){if(manageBusy){if(this.busy()||this.disabled)return;this.setBusy(true);this.error.set('')}const key=this.expenseKey,id=this.expense.id;try{const rows=await this.api.get<Reference[]>(`/expenses/${id}/references`);if(key!==this.expenseKey||id!==this.expense.id)return;this.references.set(rows);for(const t of this.types)this.referenceIds[t.id]=rows.find(r=>r.entity_type===t.id)?.entity_id||''}catch(e){if(manageBusy)this.error.set(this.api.message(e));else throw e}finally{if(manageBusy)this.setBusy(false)}}
 async loadDocuments(manageBusy=true){if(this.expense.deleted_at||!this.api.has('core.documents.read')){this.documents.set([]);this.documentTotal.set(0);return};if(manageBusy){if(this.busy()||this.disabled)return;this.setBusy(true);this.error.set('')}const key=this.expenseKey,id=this.expense.id;try{const page=await this.api.get<DocumentPage>(`/expenses/${id}/documents?page=${this.documentPage}&per_page=25`+(this.documentTrash?'&deleted=true':''));if(key!==this.expenseKey||id!==this.expense.id)return;this.documents.set(page.data);this.documentTotal.set(page.total);this.documentPage=page.page}catch(e){if(manageBusy)this.error.set(this.api.message(e));else throw e}finally{if(manageBusy)this.setBusy(false)}}
 private async resolveReference(t:ReferenceType,id:string){if(!this.api.moduleEnabled(t.module))throw new Error(`El módulo ${t.label} está deshabilitado.`);const row=await this.api.get<{version:string}>(`${t.path}/${encodeURIComponent(id)}`);if(!row.version)throw new Error(`No se pudo resolver la versión de ${t.label}.`);return {entity_type:t.id,entity_id:id,entity_version:row.version}}
 async saveReferences(){if(this.busy()||!this.canWriteReferences()||(!this.pendingReferences()&&!this.referenceReason.trim()))return;const id=this.expense.id;this.setBusy(true);this.error.set('');this.message.set('');try{let pending=this.pendingReferences();if(!pending){const references:{entity_type:ReferenceType['id'];entity_id:string;entity_version:string}[]=[];for(const t of this.types){const value=this.referenceIds[t.id].trim();if(value)references.push(await this.resolveReference(t,value))}if(this.expense.id!==id)return;const body={version:this.expense.version,reason:this.referenceReason.trim(),references},path=`/expenses/${id}/references`;pending={id,body,path,key:this.key(path,body)};this.pendingReferences.set(pending)}const r=await this.api.write<{id:string;version:string}>('POST',pending.path,pending.body,pending.key);if(this.expense.id!==id)return;this.pendingKeys.delete(pending.path);this.pendingReferences.set(null);this.confirmed.set(true);this.message.set('Referencias actualizadas.');this.changed.emit({...r,message:'Referencias del gasto actualizadas.'})}catch(e){if(this.definitive(e))this.pendingReferences.set(null);this.error.set(this.api.message(e))}finally{this.setBusy(false)}}
 selectFile(event:Event){if(this.busy()||this.disabled||this.pendingDocument())return;const input=event.target as HTMLInputElement;this.file.set(input.files?.item(0)||null);this.uploadState=null;this.error.set('');this.message.set('')}
 private media(file:File){const allowed=new Set(['application/pdf','image/png','image/jpeg','text/plain']);if(allowed.has(file.type))return file.type;const ext=file.name.toLowerCase().split('.').pop();return ({pdf:'application/pdf',png:'image/png',jpg:'image/jpeg',jpeg:'image/jpeg',txt:'text/plain'} as Record<string,string>)[ext||'']||''}
 private async digest(file:File){const hash=new Uint8Array(await crypto.subtle.digest('SHA-256',await file.arrayBuffer()));const hex=Array.from(hash,b=>b.toString(16).padStart(2,'0')).join('');let binary='';for(const b of hash)binary+=String.fromCharCode(b);return {hex,base64:btoa(binary)}}
 async uploadAndLink(){if(this.pendingDocument()){await this.retryPendingLink();return}const selected=this.file(),id=this.expense.id;if(!selected||this.busy()||!this.canUpload())return;if(selected.size<1||selected.size>20*1024*1024){this.error.set('El comprobante debe pesar entre 1 byte y 20 MB.');return}const media=this.media(selected);if(!media){this.error.set('Tipo de archivo no permitido.');return}this.setBusy(true);this.error.set('');this.message.set('');try{const checksum=await this.digest(selected);if(id!==this.expense.id)return;const createBody={filename:selected.name,media_type:media,size_bytes:selected.size,checksum:checksum.hex,version:'0'},op='/documents:'+id+':'+checksum.hex;
 if(!this.uploadState||this.uploadState.stage==='created'){const upload=await this.api.write<UploadStart>('POST','/documents',createBody,this.key(op,createBody));this.uploadState={file:selected,id:upload.id,stage:'created'};if(id!==this.expense.id)return;const put=await fetch(upload.upload_url,{method:'PUT',credentials:'omit',headers:{'Content-Type':media,'x-amz-checksum-sha256':upload.checksum_base64||checksum.base64},body:selected});if(!put.ok)throw new Error('La carga del comprobante a almacenamiento privado falló.');this.uploadState.stage='uploaded'}
 if(id!==this.expense.id)return;if(this.uploadState.stage==='uploaded'){await this.api.write('POST',`/documents/${this.uploadState.id}/finalize`,{});this.uploadState.stage='verified'};if(id!==this.expense.id)return;this.prepareLink(this.uploadState.id,'1');await this.linkPending(`/expenses/${id}/documents`);if(id===this.expense.id){this.pendingKeys.delete(op);this.file.set(null);this.uploadState=null}
 }catch(e){this.error.set(this.api.message(e))}finally{this.setBusy(false)}}
 private prepareLink(document:string,version:string){const body={version:this.expense.version,document_id:document,document_version:version},path=`/expenses/${this.expense.id}/documents`,intent=path+':'+document;this.pendingDocument.set({id:document,version,intent,body,key:this.key(intent,body)})}
 async linkExisting(){if(this.busy()||!this.canUpload()||!this.existingDocumentId.trim()||this.pendingDocument())return;this.prepareLink(this.existingDocumentId.trim(),this.existingDocumentVersion);await this.retryPendingLink()}
 async retryPendingLink(){if(this.busy()||!this.canUpload()||!this.pendingDocument())return;this.setBusy(true);this.error.set('');try{await this.linkPending(`/expenses/${this.expense.id}/documents`)}catch(e){this.error.set(this.api.message(e))}finally{this.setBusy(false)}}
 private async linkPending(path:string){const pending=this.pendingDocument(),id=this.expense.id;if(!pending)return;try{const r=await this.api.write<{id:string;version:string}>('POST',path,pending.body,pending.key);if(id!==this.expense.id)return;this.pendingKeys.delete(pending.intent);this.pendingDocument.set(null);this.confirmed.set(true);this.message.set('Comprobante vinculado; permanecerá en cuarentena hasta el escaneo.');this.changed.emit({...r,message:'Comprobante del gasto vinculado.'})}catch(e){if(this.definitive(e)){this.existingDocumentId=pending.id;this.existingDocumentVersion=pending.version;this.pendingDocument.set(null)}throw e}}
 async changeDocumentState(row:DocumentRow){if(this.busy()||!this.canUpload())return;const id=this.expense.id,path='/documents/'+row.id+(this.documentTrash?'/restore':''),body={version:row.version};this.setBusy(true);this.error.set('');try{await this.api.write(this.documentTrash?'POST':'DELETE',path,body,this.key(path,body));this.pendingKeys.delete(path);if(id===this.expense.id)await this.loadDocuments(false)}catch(e){this.error.set(this.api.message(e))}finally{this.setBusy(false)}}
 async deliver(row:DocumentRow,action:'preview'|'download'){if(this.busy()||this.disabled||this.documentTrash||row.status!=='available'||!this.api.has('core.documents.read'))return;const id=this.expense.id;this.setBusy(true);this.error.set('');try{const r=await this.api.get<{url:string}>(`/documents/${row.id}/${action}`);if(id===this.expense.id)window.open(r.url,'_blank','noopener,noreferrer')}catch(e){this.error.set(this.api.message(e))}finally{this.setBusy(false)}}
}
