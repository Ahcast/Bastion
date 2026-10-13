import { Component, OnDestroy, OnInit, inject, signal } from "@angular/core";
import { RouterLink } from "@angular/router";
import { FormsModule } from "@angular/forms";
import { Client } from "../../core/api/client";
import { Reauthentication } from "../../core/security/reauthentication";

interface Task {
  id: string;
  creator_id: string;
  assignee_id: string;
  assignee_name: string;
  resource_type?: string;
  resource_id?: string;
  resource_label?: string;
  title: string;
  note?: string;
  due_date?: string;
  status: "open" | "in_progress" | "done" | "cancelled";
  version: string;
}
interface TaskPage {
  data: Task[];
  total: number;
  page: number;
  per_page: number;
}
interface Assignee { id: string; name: string }

interface Intent {
  method: 'POST'|'PATCH'; path: string; body: Record<string,string>; key: string; clearForm: boolean;
}

@Component({
  imports: [FormsModule, RouterLink, Reauthentication],
  template: `
    <section aria-labelledby="work-title">
      <h1 id="work-title">Mi trabajo</h1>
      <p>Tareas de Colaboración. No modifica estados de los módulos comerciales.</p>
      <nav aria-label="Colaboración">
        <a routerLink="/collaboration">Biblioteca</a> ·
        <a routerLink="/collaboration/integrations">Integraciones y aprobaciones</a>
      </nav>
      @if (error()) { <p role="alert">{{ error() }}</p> }
      @if (message()) { <p role="status">{{ message() }}</p> }

      @if (pending()) {
        <p role="status">La respuesta no está confirmada. Reintente la misma operación para recuperar el resultado.</p>
        <button type="button" (click)="retry()" [disabled]="busy()">Reintentar operación</button>
      }
      <section aria-label="Filtros de trabajo">
        <label>Vista
          <select [(ngModel)]="view" (change)="page=1;load()">
            <option value="mine">Mi trabajo</option>
            <option value="today">Hoy</option>
            <option value="overdue">Vencidas</option>
          </select>
        </label>
        <label>Estado
          <select [(ngModel)]="status" (change)="page=1;load()">
            <option value="">Todos</option>
            <option value="open">Abierta</option>
            <option value="in_progress">En progreso</option>
            <option value="done">Completada</option>
            <option value="cancelled">Cancelada</option>
          </select>
        </label>
        <button type="button" (click)="load()" [disabled]="busy()">Actualizar</button>
      </section>

      @if (api.has('colaboracion.task')) {
        <section aria-labelledby="new-task">
          <h2 id="new-task">{{ editing() ? 'Editar tarea' : 'Nueva tarea' }}</h2>
          <fieldset [disabled]="blocked()">
          <legend>Datos de tarea</legend>
          <label>Título<input [(ngModel)]="title" maxlength="180" /></label>
          <label>Nota<textarea [(ngModel)]="note" maxlength="4000"></textarea></label>
          <label>Asignar a
            <select aria-label="Asignar a" [(ngModel)]="assigneeID">
              <option value="">Seleccione</option>
              @for (u of assignees(); track u.id) { <option [value]="u.id">{{ u.name }}</option> }
            </select>
          </label>
          <label>Vence<input type="date" [(ngModel)]="dueDate" /></label>
          <fieldset [disabled]="!!editing()">
            <legend>Recurso opcional</legend>
            <label>Tipo
              <select [(ngModel)]="resourceType" (change)="loadAssignees()">
                <option value="">Sin recurso</option>
                <option value="processes">Proceso</option>
                <option value="quotes">Cotización</option>
                <option value="purchase_orders">Orden de compra</option>
                <option value="invoices">Factura</option>
                <option value="collaboration">Archivo de colaboración</option>
              </select>
            </label>
            <label>ID<input [(ngModel)]="resourceID" (change)="loadAssignees()" /></label>
          </fieldset>
          <button type="button" (click)="create()" [disabled]="blocked() || !title.trim() || !assigneeID">{{ editing() ? 'Guardar tarea' : 'Crear tarea' }}</button>
          @if (editing()) { <button type="button" (click)="cancelEdit()">Cancelar edición</button> }
          </fieldset>
        </section>
      }

      <section aria-labelledby="task-list">
        <h2 id="task-list">Tareas ({{ total() }})</h2>
        <ul>
          @for (task of tasks(); track task.id) {
            <li>
              <strong>{{ task.title }}</strong> — {{ label(task.status) }}
              <span> · {{ task.assignee_name }}</span>
              @if (task.due_date) { <span> · vence {{ task.due_date }}</span> }
              @if (task.resource_label) { <span> · {{ task.resource_label }}</span> }
              @if (task.note) { <p>{{ task.note }}</p> }
              @if (task.assignee_id === api.profile()?.user?.id && (task.status === 'open' || task.status === 'in_progress')) {
                @if (task.status === 'open') { <button type="button" (click)="setStatus(task, 'in_progress')" [disabled]="blocked()">Iniciar</button> }
                <button type="button" (click)="setStatus(task, 'done')" [disabled]="blocked()">Completar</button>
                <button type="button" (click)="setStatus(task, 'cancelled')" [disabled]="blocked()">Cancelar</button>
              }
              @if (task.creator_id === api.profile()?.user?.id && api.has('colaboracion.task') && (task.status === 'open' || task.status === 'in_progress')) {
                <button type="button" (click)="edit(task)" [disabled]="blocked()">Editar tarea</button>
              }
              @if (task.creator_id === api.profile()?.user?.id && api.has('colaboracion.task') && (task.status === 'done' || task.status === 'cancelled')) {
                <button type="button" (click)="reopen(task)" [disabled]="blocked()">Reabrir</button>
              }
            </li>
          } @empty { <li>No hay tareas para esta vista.</li> }
        </ul>
        <button type="button" (click)="page=page-1;load()" [disabled]="busy() || page<=1">Anterior</button>
        <button type="button" (click)="page=page+1;load()" [disabled]="busy() || page*100>=total()">Siguiente</button>
      </section>
      <bastion-reauthentication />
    </section>
  `,
})
export class CollaborationWork implements OnInit, OnDestroy {
  api = inject(Client);
  tasks = signal<Task[]>([]);
  assignees = signal<Assignee[]>([]);
  total = signal(0);
  busy = signal(false);
  error = signal("");
  message = signal("");
  view = "mine";
  status = "";
  title = "";
  note = "";
  assigneeID = "";
  dueDate = "";
  resourceType = "";
  resourceID = "";

  editing = signal<Task|null>(null);
  pending = signal<Intent|null>(null);
  confirmed = signal(false);
  page = 1;
  private readSequence = 0;
  private assigneeSequence = 0;
  private disposed = false;
  blocked() { return this.busy() || !!this.pending() || this.confirmed(); }
  ngOnDestroy() { this.disposed = true; this.readSequence++; this.assigneeSequence++; }
  async ngOnInit() {
    await this.load();
    if (this.api.has("colaboracion.task")) await this.loadAssignees();
  }
  async load() {
    const sequence = ++this.readSequence;
    this.busy.set(true); this.error.set("");
    try {
      const q = new URLSearchParams({ view: this.view, status: this.status, page: String(this.page), per_page: "100" });
      const page = await this.api.get<TaskPage>("/collaboration/tasks?" + q);
      if (this.disposed || sequence !== this.readSequence) return;
      this.tasks.set(page.data); this.total.set(page.total); this.confirmed.set(false);
    } catch (e) {
      if (!this.disposed && sequence === this.readSequence) {
        this.tasks.set([]); this.total.set(0); this.error.set(this.api.message(e));
      }
    } finally { if (!this.disposed && sequence === this.readSequence) this.busy.set(false); }
  }
  async loadAssignees() {
    if (!this.api.has("colaboracion.task")) return;
    const sequence = ++this.assigneeSequence;
    this.assignees.set([]);
    try {
      const q = new URLSearchParams({ resource_type: this.resourceType, resource_id: this.resourceID, search: "" });
      const r = await this.api.get<{data: Assignee[]}>("/collaboration/task-assignees?" + q);
      if (this.disposed || sequence !== this.assigneeSequence) return;
      this.assignees.set(r.data);
      if (this.assigneeID && !r.data.some((x) => x.id === this.assigneeID)) this.assigneeID = "";
    } catch (e) { if (!this.disposed && sequence === this.assigneeSequence) this.error.set(this.api.message(e)); }
  }
  async edit(task: Task) {
    if (this.blocked()) return;
    this.editing.set(task);
    this.title = task.title; this.note = task.note || ""; this.assigneeID = task.assignee_id;
    this.dueDate = task.due_date || ""; this.resourceType = task.resource_type || ""; this.resourceID = task.resource_id || "";
    await this.loadAssignees();
  }
  cancelEdit() {
    if (this.blocked()) return;
    this.resetForm(); void this.loadAssignees();
  }
  private resetForm() {
    this.editing.set(null); this.title = this.note = this.dueDate = "";
    this.resourceType = this.resourceID = "";
  }
  async create() {
    if (this.blocked()) return;
    const editing = this.editing();
    const body: Record<string,string> = {
      assignee_id: this.assigneeID, title: this.title.trim(), note: this.note.trim(), due_date: this.dueDate,
      ...(editing ? {version: editing.version} : {resource_type: this.resourceType, resource_id: this.resourceID}),
    };
    await this.perform({method: editing ? 'PATCH' : 'POST', path: '/collaboration/tasks' + (editing ? '/' + editing.id : ''), body, key: crypto.randomUUID(), clearForm: true});
  }
  async setStatus(task: Task, status: Task["status"]) {
    if (this.blocked()) return;
    await this.perform({method: 'POST', path: `/collaboration/tasks/${task.id}/status`, body: {version:task.version,status}, key:crypto.randomUUID(),clearForm:false});
  }
  async reopen(task: Task) {
    if (this.blocked()) return;
    await this.perform({method:'POST',path:`/collaboration/tasks/${task.id}/reopen`,body:{version:task.version},key:crypto.randomUUID(),clearForm:false});
  }
  async retry() {
    const pending = this.pending();
    if (pending && !this.busy()) await this.perform(pending);
  }
  private async perform(intent: Intent) {
    this.pending.set(intent); this.busy.set(true); this.error.set("");
    try {
      await this.api.write(intent.method, intent.path, intent.body, intent.key);
      if (this.disposed) return;
      this.pending.set(null); this.confirmed.set(true);
      if (intent.clearForm) this.resetForm();
      this.message.set("Operación guardada.");
      await this.load();
      if (intent.clearForm) await this.loadAssignees();
    } catch (e) {
      if (!this.disposed) {
        this.error.set(this.api.message(e));
        // A definitive HTTP rejection has no effect to recover. Network errors
        // retain the original intent; an exact retry never creates another task.
        const status = (e as {status?:number})?.status;
        if (status && status >= 400 && status < 500) this.pending.set(null);
      }
    } finally { if (!this.disposed) this.busy.set(false); }
  }
  label(status: Task["status"]) {
    return ({ open: "Abierta", in_progress: "En progreso", done: "Completada", cancelled: "Cancelada" } as const)[status];
  }
}
