import { Component, OnInit, inject, signal } from "@angular/core";
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

@Component({
  imports: [FormsModule, Reauthentication],
  template: `
    <section aria-labelledby="work-title">
      <h1 id="work-title">Mi trabajo</h1>
      <p>Tareas de Colaboración. No modifica estados de los módulos comerciales.</p>
      <nav aria-label="Colaboración">
        <a href="/collaboration">Biblioteca</a> ·
        <a href="/collaboration/integrations">Integraciones y aprobaciones</a>
      </nav>
      @if (error()) { <p role="alert">{{ error() }}</p> }
      @if (message()) { <p role="status">{{ message() }}</p> }

      <section aria-label="Filtros de trabajo">
        <label>Vista
          <select [(ngModel)]="view" (change)="load()">
            <option value="mine">Mi trabajo</option>
            <option value="today">Hoy</option>
            <option value="overdue">Vencidas</option>
          </select>
        </label>
        <label>Estado
          <select [(ngModel)]="status" (change)="load()">
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
          <h2 id="new-task">Nueva tarea</h2>
          <label>Título<input [(ngModel)]="title" maxlength="180" /></label>
          <label>Nota<textarea [(ngModel)]="note" maxlength="4000"></textarea></label>
          <label>Asignar a
            <select aria-label="Asignar a" [(ngModel)]="assigneeID">
              <option value="">Seleccione</option>
              @for (u of assignees(); track u.id) { <option [value]="u.id">{{ u.name }}</option> }
            </select>
          </label>
          <label>Vence<input type="date" [(ngModel)]="dueDate" /></label>
          <fieldset>
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
          <button type="button" (click)="create()" [disabled]="busy() || !title.trim() || !assigneeID">Crear tarea</button>
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
                @if (task.status === 'open') { <button type="button" (click)="setStatus(task, 'in_progress')">Iniciar</button> }
                <button type="button" (click)="setStatus(task, 'done')">Completar</button>
                <button type="button" (click)="setStatus(task, 'cancelled')">Cancelar</button>
              }
              @if (task.creator_id === api.profile()?.user?.id && api.has('colaboracion.task') && (task.status === 'done' || task.status === 'cancelled')) {
                <button type="button" (click)="reopen(task)">Reabrir</button>
              }
            </li>
          } @empty { <li>No hay tareas para esta vista.</li> }
        </ul>
      </section>
      <bastion-reauthentication />
    </section>
  `,
})
export class CollaborationWork implements OnInit {
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

  async ngOnInit() {
    await this.load();
    if (this.api.has("colaboracion.task")) await this.loadAssignees();
  }
  async load() {
    this.busy.set(true); this.error.set("");
    try {
      const q = new URLSearchParams({ view: this.view, status: this.status, page: "1", per_page: "100" });
      const page = await this.api.get<TaskPage>("/collaboration/tasks?" + q);
      this.tasks.set(page.data); this.total.set(page.total);
    } catch (e) { this.error.set(this.api.message(e)); }
    finally { this.busy.set(false); }
  }
  async loadAssignees() {
    if (!this.api.has("colaboracion.task")) return;
    try {
      const q = new URLSearchParams({ resource_type: this.resourceType, resource_id: this.resourceID, search: "" });
      const r = await this.api.get<{data: Assignee[]}>("/collaboration/task-assignees?" + q);
      this.assignees.set(r.data);
      if (this.assigneeID && !r.data.some((x) => x.id === this.assigneeID)) this.assigneeID = "";
    } catch (e) { this.assignees.set([]); this.error.set(this.api.message(e)); }
  }
  async create() {
    this.busy.set(true); this.error.set(""); this.message.set("");
    try {
      await this.api.write("POST", "/collaboration/tasks", {
        assignee_id: this.assigneeID, resource_type: this.resourceType,
        resource_id: this.resourceID, title: this.title.trim(), note: this.note.trim(), due_date: this.dueDate,
      }, crypto.randomUUID());
      this.title = this.note = this.dueDate = ""; this.resourceType = this.resourceID = "";
      this.message.set("Tarea creada."); await this.load(); await this.loadAssignees();
    } catch (e) { this.error.set(this.api.message(e)); }
    finally { this.busy.set(false); }
  }
  async setStatus(task: Task, status: Task["status"]) {
    this.error.set("");
    try {
      await this.api.write("POST", `/collaboration/tasks/${task.id}/status`, { version: task.version, status }, crypto.randomUUID());
      await this.load();
    } catch (e) { this.error.set(this.api.message(e)); }
  }
  async reopen(task: Task) {
    this.error.set("");
    try {
      await this.api.write("POST", `/collaboration/tasks/${task.id}/reopen`, { version: task.version }, crypto.randomUUID());
      await this.load();
    } catch (e) { this.error.set(this.api.message(e)); }
  }
  label(status: Task["status"]) {
    return ({ open: "Abierta", in_progress: "En progreso", done: "Completada", cancelled: "Cancelada" } as const)[status];
  }
}
