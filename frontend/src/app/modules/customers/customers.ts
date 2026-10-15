import {NgTemplateOutlet} from "@angular/common";
import {WorkspaceSummary} from "../../shared/ui/workspace-summary";
import { Component, inject, signal, OnInit } from "@angular/core";
import { FormsModule } from "@angular/forms";
import { DatePipe } from "@angular/common";
import { Client } from "../../core/api/client";

export interface Contact { name:string;role:string;phone:string;email:string }
export interface Address { label:string;address:string }
export interface Customer { id:string;name:string;rtn:string;contact_name:string;phone:string;email:string;address:string;institution_type:string;is_active:boolean;team_id:string;version:string;contacts:Contact[];addresses:Address[] }
type CustomerSummary=Pick<Customer,"id"|"name"|"rtn"|"email"|"is_active"|"version">;
interface Page<T> {data:T[];total:number;page:number;per_page:number}
interface Note {id:string;body:string;user_name:string;created_at:string}
interface History {id:string;action:string;actor_name:string;created_at:string;details:{version?:string}}
interface Document {id:string;filename:string;status:string;version:string}
const empty=():Customer=>({id:"",name:"",rtn:"",contact_name:"",phone:"",email:"",address:"",institution_type:"other",is_active:true,team_id:"",version:"0",contacts:[],addresses:[]});

@Component({imports:[NgTemplateOutlet,WorkspaceSummary,FormsModule,DatePipe],templateUrl:"./customers.html"})
export class Customers implements OnInit {
 api=inject(Client);rows=signal<CustomerSummary[]>([]);total=signal(0);loading=signal(false);busy=signal(false);error=signal("");message=signal("");editor=signal<Customer|null>(null);
 teams=signal<{id:string;name:string}[]>([]);notes=signal<Note[]>([]);history=signal<History[]>([]);documents=signal<Document[]>([]);personalFiles=signal<Document[]>([]);
 search="";rtn="";active="true";page=1;perPage=25;note="";notePage=1;noteTotal=signal(0);historyPage=1;historyTotal=signal(0);documentPage=1;documentTotal=signal(0);personalPage=1;selectedDocument="";
 password="";factor="";private pending=new Map<string,{body:string,key:string}>();
 async ngOnInit(){await this.load();try{this.teams.set((await this.api.get<{data:{id:string;name:string}[]}>("/customers/teams")).data)}catch(e){this.error.set(this.api.message(e))}}
 async load(reset=false){if(reset)this.page=1;this.loading.set(true);this.error.set("");try{const q=new URLSearchParams({search:this.search,rtn:this.rtn,page:String(this.page),per_page:String(this.perPage)});if(this.active!=="")q.set("is_active",this.active);const p=await this.api.get<Page<CustomerSummary>>("/customers?"+q);this.rows.set(p.data);this.total.set(p.total)}catch(e){this.error.set(this.api.message(e));this.rows.set([]);this.total.set(0)}finally{this.loading.set(false)}}
 create(){this.editor.set(empty());this.clearDetails();this.error.set("");this.message.set("")}
 clearDetails(){this.notes.set([]);this.history.set([]);this.documents.set([]);this.note="";this.notePage=1;this.historyPage=1;this.documentPage=1;this.selectedDocument="";this.personalFiles.set([]);this.personalPage=1;this.noteTotal.set(0);this.historyTotal.set(0);this.documentTotal.set(0)}
 async open(id:string){this.busy.set(true);this.error.set("");try{this.editor.set(await this.api.get<Customer>("/customers/"+id));this.clearDetails();await this.loadNotes();await this.loadHistory()}catch(e){this.editor.set(null);this.error.set(this.api.message(e))}finally{this.busy.set(false)}}
 key(operation:string,body:unknown){const encoded=JSON.stringify(body),old=this.pending.get(operation);if(old?.body===encoded)return old.key;const key=crypto.randomUUID();this.pending.set(operation,{body:encoded,key});return key}
 async save(){const c=this.editor();if(!c||this.busy())return;this.busy.set(true);this.error.set("");this.message.set("");const id=c.id,body={name:c.name,rtn:c.rtn,contact_name:c.contact_name,phone:c.phone,email:c.email,address:c.address,institution_type:c.institution_type,is_active:c.is_active,team_id:c.team_id,version:c.version,contacts:c.contacts,addresses:c.addresses};const path="/customers"+(id?"/"+id:"");try{const result=await this.api.write<{id:string;version:string}>(id?"PATCH":"POST",path,body,this.key(path,body));this.pending.delete(path);this.editor.set(await this.api.get<Customer>("/customers/"+result.id));await this.load();await this.loadHistory();this.message.set("Cliente guardado.")}catch(e){this.error.set(this.api.message(e))}finally{this.busy.set(false)}}
 contact(){this.editor()?.contacts.push({name:"",role:"",phone:"",email:""})}
 address(){this.editor()?.addresses.push({label:"",address:""})}
 async reauthenticate(){if(this.busy())return;this.busy.set(true);this.error.set("");try{await this.api.write("POST","/auth/reauth",{password:this.password});this.message.set("Identidad confirmada durante cinco minutos.")}catch(e){this.error.set(this.api.message(e))}finally{this.password="";this.factor="";this.busy.set(false)}}
 async loadNotes(){const id=this.editor()?.id;if(!id)return;try{const p=await this.api.get<Page<Note>>("/customers/"+id+"/notes?page="+this.notePage);this.notes.set(p.data);this.noteTotal.set(p.total)}catch(e){this.notes.set([]);this.error.set(this.api.message(e))}}
 async addNote(){const c=this.editor();if(!c?.id||this.busy())return;this.busy.set(true);this.error.set("");const path="/customers/"+c.id+"/notes",body={body:this.note,version:c.version};try{await this.api.write("POST",path,body,this.key(path,body));this.pending.delete(path);this.note="";this.editor.set(await this.api.get<Customer>("/customers/"+c.id));this.notePage=1;await this.loadNotes();await this.loadHistory();this.message.set("Nota registrada.")}catch(e){this.error.set(this.api.message(e))}finally{this.busy.set(false)}}
 async loadHistory(){const id=this.editor()?.id;if(!id)return;try{const p=await this.api.get<Page<History>>("/customers/"+id+"/history?page="+this.historyPage);this.history.set(p.data);this.historyTotal.set(p.total)}catch(e){this.history.set([]);this.error.set(this.api.message(e))}}
 historyLabel(action:string){return ({"customers.created":"Cliente creado","customers.updated":"Ficha actualizada","customers.noted":"Nota registrada","customers.document_linked":"Documento vinculado","mcp.get_customer":"Ficha consultada mediante integración"} as Record<string,string>)[action]||"Actividad de consulta"}
 async loadDocuments(){const id=this.editor()?.id;if(!id)return;try{const p=await this.api.get<Page<Document>>("/customers/"+id+"/documents?page="+this.documentPage);this.documents.set(p.data);this.documentTotal.set(p.total)}catch(e){this.documents.set([]);this.error.set(this.api.message(e))}}
 async loadPersonalFiles(){try{this.personalFiles.set((await this.api.get<{data:Document[]}>("/documents?page="+this.personalPage)).data)}catch(e){this.error.set(this.api.message(e))}}
 async linkDocument(){const c=this.editor(),doc=this.personalFiles().find(d=>d.id===this.selectedDocument);if(!c?.id||!doc||this.busy())return;this.busy.set(true);this.error.set("");const path="/customers/"+c.id+"/documents",body={document_id:doc.id,version:doc.version};try{await this.api.write("POST",path,body,this.key(path,body));this.pending.delete(path);this.selectedDocument="";await this.loadDocuments();await this.loadPersonalFiles();await this.loadHistory();this.message.set("Documento vinculado.")}catch(e){this.error.set(this.api.message(e))}finally{this.busy.set(false)}}
 async download(id:string){try{const value=await this.api.get<{url:string}>("/documents/"+id+"/download");const url=new URL(value.url);if(url.protocol!=="https:")throw new Error("Enlace de descarga inválido.");window.open(url.href,"_blank","noopener,noreferrer")}catch(e){this.error.set(this.api.message(e))}}
 state(status:string){return ({pending:"Carga pendiente",quarantined:"En cuarentena",available:"Disponible",rejected:"Rechazado"} as Record<string,string>)[status]||"No disponible"}

 inlineId(){return this.editor()?.id||''}
 inlineInPage(){return this.rows().some(row=>row.id===this.inlineId())}
 toggleInline(id:string,event:Event){const target=event.target as HTMLElement;if(target.closest('button,a,input,select,textarea'))return;if(event.type==='keydown'){if(event.target!==event.currentTarget)return;event.preventDefault()}if(this.busy())return;if(this.inlineId()===id){this.editor.set(null)}else{void this.open(id)}}
}
