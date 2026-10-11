import {Component,Input,OnChanges,inject,signal} from '@angular/core';
import {FormsModule} from '@angular/forms';
import {DatePipe} from '@angular/common';
import {Client} from '../../core/api/client';

interface EmailQuote {id:string;number:string;version:string;status:string;created_by?:string;deleted_at:string|null;customer:{email:string}}
interface PDFSnapshot {id:string;quote_version:string;status:string;kind:'draft'|'issued'}
interface DocumentRef {id:string;filename:string;status:string;media_type:string}
interface EmailIntent {id:string;quote_version:string;pdf_snapshot_id:string;created_by:string;to:string[];cc:string[];bcc:string[];attachment_ids:string[];subject:string;message:string;scheduled_at:string;approval_id?:string;approval_version:string;approval_status?:string;requested_by?:string;message_id?:string;job_id?:string;status:string;error_code?:string;accepted_at?:string;created_at:string;updated_at:string}

@Component({
 selector:'bastion-quote-email',
 imports:[FormsModule,DatePipe],
 template:`
 <section class="card">
  <h3>Envío comercial</h3>
  <p>El envío requiere un PDF emitido y una autorización separada que queda vinculada a destinatarios, asunto, mensaje, adjuntos y fecha programada. Aprobar no envía automáticamente.</p>
  <button type="button" (click)="refresh()" [disabled]="busy()">Actualizar correo y ledger</button>
  @if(error()){<p role="alert">{{error()}}</p>}
  @if(quote.status==='approved'&&!quote.deleted_at&&api.has('cotizaciones.send')&&api.has('cotizaciones.request_approval')){
   <fieldset [disabled]="busy()"><legend>Nueva autorización de correo</legend>
    <label>PDF emitido<select [(ngModel)]="pdfSnapshotId"><option value="">Seleccione</option>@for(p of issuedPDFs();track p.id){<option [value]="p.id">PDF emitido · versión {{p.quote_version}} · {{p.id}}</option>}</select></label>
    <label>Para<input [value]="to" (input)="to=$any($event.target).value" autocomplete="off" placeholder="cliente@ejemplo.com"></label>
    <label>CC<input [(ngModel)]="cc" autocomplete="off" placeholder="correo1@ejemplo.com, correo2@ejemplo.com"></label>
    <label>BCC<input [(ngModel)]="bcc" autocomplete="off"></label>
    <p>Separe múltiples direcciones con coma, punto y coma o salto de línea.</p>
    <label>Asunto<input [(ngModel)]="subject" maxlength="200"></label>
    <label>Mensaje<textarea [(ngModel)]="message"></textarea></label>
    <label>Programar envío (hora de Honduras)<input type="datetime-local" [(ngModel)]="scheduledAt"></label>
    <label>Vigencia de autorización (hora de Honduras)<input type="datetime-local" [(ngModel)]="expiresAt"></label>
    <label>Motivo de autorización<textarea [(ngModel)]="reason" maxlength="1000"></textarea></label>
    @if(documents().length){<fieldset><legend>Adjuntos adicionales</legend>@for(d of documents();track d.id){@if(d.status==='available'){<label><input type="checkbox" [checked]="attachmentIds.includes(d.id)" (change)="toggleAttachment(d.id)">{{d.filename}}</label>}}</fieldset>}
    <button type="button" (click)="requestAuthorization()" [disabled]="!canRequest()">Solicitar autorización de envío</button>
   </fieldset>
  }
  <h4>Ledger de correo</h4>
  <p>{{total()}} intenciones registradas · Página {{page}}</p>
  <label>Motivo de decisión o revocación<textarea [(ngModel)]="decisionReason" maxlength="1000"></textarea></label>
  @for(x of intents();track x.id){<article class="card">
   <p><strong>{{statusLabel(x.status)}}</strong> · versión {{x.quote_version}} · programado {{x.scheduled_at|date:'dd/MM/yyyy HH:mm':'-0600'}} Honduras</p>
   <p>Para: {{x.to.join(', ')||'—'}} @if(x.cc.length){· CC: {{x.cc.join(', ')}}} @if(x.bcc.length){· BCC: {{x.bcc.join(', ')}}}</p>
   <p>Asunto: {{x.subject}}</p><p style="white-space:pre-wrap">{{x.message}}</p>
   <p>PDF {{x.pdf_snapshot_id}} @if(x.attachment_ids.length){· {{x.attachment_ids.length}} adjunto(s) adicional(es)}</p>
   @if(x.approval_id){<p>Aprobación {{x.approval_id}} · {{x.approval_status||'—'}} · solicitada por {{x.requested_by||'—'}}</p>}
   @if(x.message_id){<p>Message-ID: {{x.message_id}}</p>}
   @if(x.accepted_at){<p>Aceptado por SMTP: {{x.accepted_at|date:'dd/MM/yyyy HH:mm:ss':'-0600'}} Honduras. Esto confirma aceptación del servidor SMTP, no lectura ni entrega final al buzón.</p>}
   @if(x.error_code){<p role="alert">Error: {{errorLabel(x.error_code)}}</p>}
   @if(x.status==='uncertain'){<p role="alert">Resultado SMTP incierto. No se reenvía automáticamente; debe conciliarse antes de cualquier nuevo intento.</p>}
   @if(canDecide(x)){<button type="button" (click)="decide(x,'approved')" [disabled]="busy()||!decisionReason.trim()">Aprobar envío</button><button type="button" (click)="decide(x,'rejected')" [disabled]="busy()||!decisionReason.trim()">Rechazar envío</button>}
   @if(canRevoke(x)){<button type="button" (click)="revoke(x)" [disabled]="busy()||!decisionReason.trim()">Revocar autorización</button>}
   @if(canSend(x)){<button type="button" (click)="send(x)" [disabled]="busy()">Enviar / programar correo autorizado</button>}
  </article>}@empty{<p>No hay intenciones de correo registradas.</p>}
  <button type="button" [disabled]="page<=1||busy()" (click)="page=page-1;loadLedger()">Anterior</button><button type="button" [disabled]="page*25>=total()||busy()" (click)="page=page+1;loadLedger()">Más correos</button>
 </section>`
})
export class QuoteEmail implements OnChanges{
 @Input({required:true}) quote!:EmailQuote;
 api=inject(Client);busy=signal(false);error=signal('');issuedPDFs=signal<PDFSnapshot[]>([]);documents=signal<DocumentRef[]>([]);intents=signal<EmailIntent[]>([]);total=signal(0);page=1;
 pdfSnapshotId='';to='';cc='';bcc='';subject='';message='';scheduledAt='';expiresAt='';reason='';decisionReason='';attachmentIds:string[]=[];private pending=new Map<string,string>();private quoteKey='';
 ngOnChanges(){if(!this.quote||this.quote.id===this.quoteKey)return;this.quoteKey=this.quote.id;this.page=1;this.resetForm();void this.refresh()}
 private localInput(ms:number){return new Date(Date.now()+ms-6*60*60*1000).toISOString().slice(0,16)}
 private hondurasISO(v:string){if(!v)throw new Error('Indique fecha y hora.');const d=new Date(v+'-06:00');if(Number.isNaN(d.getTime()))throw new Error('Fecha u hora inválida.');return d.toISOString()}
 private addresses(v:string){return [...new Set(v.split(/[;,\n]/).map(x=>x.trim()).filter(Boolean))]}
 private key(path:string){const old=this.pending.get(path);if(old)return old;const k=crypto.randomUUID();this.pending.set(path,k);return k}
 resetForm(){this.pdfSnapshotId='';this.to=this.quote.customer.email||'';this.cc='';this.bcc='';this.subject=`Cotización ${this.quote.number}`;this.message='Adjuntamos la cotización aprobada.';this.scheduledAt=this.localInput(0);this.expiresAt=this.localInput(24*60*60*1000);this.reason='';this.decisionReason='';this.attachmentIds=[]}
 async refresh(){if(!this.quote?.id)return;this.busy.set(true);this.error.set('');try{await Promise.all([this.loadPDFs(),this.loadDocuments(),this.loadLedger()])}finally{this.busy.set(false)}}
 async loadPDFs(){try{const r=await this.api.get<{data:PDFSnapshot[]}>(`/quotes/${this.quote.id}/pdfs?per_page=100`);this.issuedPDFs.set(r.data.filter(p=>p.kind==='issued'&&p.status==='available'&&p.quote_version===this.quote.version));if(!this.pdfSnapshotId&&this.issuedPDFs().length)this.pdfSnapshotId=this.issuedPDFs()[0].id}catch(e){this.error.set(this.api.message(e))}}
 async loadDocuments(){if(!this.api.has('core.documents.read'))return;try{const r=await this.api.get<{data:DocumentRef[]}>(`/quotes/${this.quote.id}/documents?per_page=100&deleted=false`);this.documents.set(r.data.filter(d=>d.status==='available'))}catch(e){this.error.set(this.api.message(e))}}
 async loadLedger(){try{const r=await this.api.get<{data:EmailIntent[];total:number}>(`/quotes/${this.quote.id}/email-authorizations?page=${this.page}&per_page=25`);this.intents.set(r.data);this.total.set(r.total)}catch(e){this.error.set(this.api.message(e))}}
 toggleAttachment(id:string){this.attachmentIds=this.attachmentIds.includes(id)?this.attachmentIds.filter(x=>x!==id):[...this.attachmentIds,id]}
 canRequest(){return !this.busy()&&!!this.pdfSnapshotId&&!!this.to.trim()&&!!this.subject.trim()&&!!this.message.trim()&&!!this.scheduledAt&&!!this.expiresAt&&!!this.reason.trim()}
 async requestAuthorization(){if(!this.canRequest())return;const path=`/quotes/${this.quote.id}/email-authorizations`;this.busy.set(true);this.error.set('');try{const body={quote_version:this.quote.version,pdf_snapshot_id:this.pdfSnapshotId,to:this.addresses(this.to),cc:this.addresses(this.cc),bcc:this.addresses(this.bcc),attachment_ids:this.attachmentIds,subject:this.subject,message:this.message,scheduled_at:this.hondurasISO(this.scheduledAt),expires_at:this.hondurasISO(this.expiresAt),reason:this.reason};await this.api.write('POST',path,body,this.key(path+JSON.stringify(body)));this.pending.clear();this.reason='';await this.loadLedger()}catch(e){this.error.set(this.api.message(e))}finally{this.busy.set(false)}}
 canDecide(x:EmailIntent){const user=this.api.profile()?.user.id;return x.status==='pending_approval'&&x.approval_status==='pending'&&!!x.approval_id&&user!==x.requested_by&&this.api.has('cotizaciones.approve')}
 async decide(x:EmailIntent,decision:'approved'|'rejected'){if(!this.canDecide(x)||!this.decisionReason.trim())return;const path=`/quotes/${this.quote.id}/email-authorizations/${x.id}/decision`,body={version:x.approval_version,decision,reason:this.decisionReason};await this.write(path,body)}
 canRevoke(x:EmailIntent){return ['pending_approval','approved'].includes(x.status)&&!!x.approval_id&&this.api.has('cotizaciones.revoke_approval')}
 async revoke(x:EmailIntent){if(!this.canRevoke(x)||!this.decisionReason.trim())return;const path=`/quotes/${this.quote.id}/email-authorizations/${x.id}/revoke`,body={version:x.approval_version,reason:this.decisionReason};await this.write(path,body)}
 canSend(x:EmailIntent){return x.status==='approved'&&x.quote_version===this.quote.version&&this.api.has('cotizaciones.send')}
 async send(x:EmailIntent){if(!this.canSend(x))return;const path=`/quotes/${this.quote.id}/email-authorizations/${x.id}/send`;await this.write(path,{})}
 private async write(path:string,body:unknown){this.busy.set(true);this.error.set('');try{await this.api.write('POST',path,body,this.key(path+JSON.stringify(body)));this.pending.clear();this.decisionReason='';await this.loadLedger()}catch(e){this.error.set(this.api.message(e))}finally{this.busy.set(false)}}
 statusLabel(s:string){return ({pending_approval:'Pendiente de aprobación',approved:'Autorizado',rejected:'Rechazado',revoked:'Revocado',queued:'En cola',preparing:'Preparando',sending:'Enviando',accepted:'Aceptado por SMTP',failed:'Fallido',uncertain:'Resultado incierto'} as Record<string,string>)[s]||s}
 errorLabel(s:string){return ({attachment_integrity_failed:'Falló la integridad de un adjunto',envelope_invalid:'Sobre de correo inválido',authorization_revoked:'Autorización o permiso revocado',pre_send_revalidation_failed:'Falló la revalidación previa al envío',external_result_uncertain:'El resultado externo no pudo determinarse',preparation_rejected:'La preparación del envío fue rechazada'} as Record<string,string>)[s]||s}
}
