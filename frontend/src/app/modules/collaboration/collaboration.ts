import { Component, inject, OnInit, signal } from "@angular/core";
import {RouterLink,RouterLinkActive} from "@angular/router";
import { FormsModule } from "@angular/forms";
import { Client } from "../../core/api/client";
import { Reauthentication } from "../../core/security/reauthentication";

interface Folder {
  id: string;
  scope: "company" | "person";
  person_id?: string;
  team_id?: string;
  parent_id?: string;
  name: string;
  version: string;
  file_count: number;
  deleted_at?: string;
}
export interface Entry {
  id: string;
  document_id: string;
  folder_id?: string;
  scope: "company" | "person";
  person_id?: string;
  team_id?: string;
  title: string;
  notes?: string;
  uploaded_by: string;
  version: string;
  document_version: string;
  file_version: number;
  filename: string;
  media_type: string;
  size_bytes: number;
  status: string;
  updated_at: string;
  deleted: boolean;
}
interface Page<T> {
  data: T[];
  total: number;
  page: number;
  per_page: number;
}
interface Team {
  id: string;
  name: string;
}
interface Comment {
  id: string;
  author_id: string;
  author_name: string;
  body: string;
  mention_user_ids: string[];
  version: string;
  created_at: string;
}
interface ActivityItem {
  id: string;
  actor_name: string;
  action: "comment_added" | "comment_removed";
  created_at: string;
}
interface MentionableUser {
  id: string;
  name: string;
}
interface CollaborationNotification {
  id: string;
  resource_type: string;
  resource_id: string;
  comment_id: string;
  created_at: string;
  read_at?: string;
}
interface Pending {
  body: string;
  key: string;
}
interface UploadState {
  file: File;
  scope: "company" | "person";
  folderID: string;
  teamID: string;
  id: string;
  uploadUrl: string;
  checksum: string;
  version: string;
  stage: "requesting" | "created" | "uploaded" | "verified";
  bindPayload?: {
    document_id: string;
    document_version: string;
    scope: "company" | "person";
    person_id: string;
    folder_id: string;
    title: string;
    notes: string;
  };
}
interface VersionUploadState {
  file: File;
  entryID: string;
  documentID: string;
  documentVersion: string;
  uploadUrl: string;
  checksum: string;
  stage: "requesting" | "created" | "uploaded";
}

const MEDIA: Record<string, string> = {
  pdf: "application/pdf",
  doc: "application/msword",
  docx: "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
  xls: "application/vnd.ms-excel",
  xlsx: "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
  csv: "text/csv",
  txt: "text/plain",
  jpg: "image/jpeg",
  jpeg: "image/jpeg",
  png: "image/png",
};
const ACCEPT = ".pdf,.doc,.docx,.xls,.xlsx,.csv,.txt,.jpg,.jpeg,.png";

@Component({
  imports: [FormsModule,RouterLink,RouterLinkActive,Reauthentication],
  template: `
    <section aria-labelledby="collab-title" class="workspace">
      <header class="bs-page-head"><div><span class="bs-label">Bastion · Colaboración</span><h1 id="collab-title">Colaboración</h1><p>Archivos, conversaciones y vínculos del equipo.</p></div></header><nav class="bs-section-menu" aria-label="Colaboración"><a routerLink="/collaboration" routerLinkActive="active" [routerLinkActiveOptions]="{exact:true}" ariaCurrentWhenActive="page">Biblioteca</a><a routerLink="/collaboration/work" routerLinkActive="active" ariaCurrentWhenActive="page">Mi trabajo</a><a routerLink="/collaboration/integrations" routerLinkActive="active" ariaCurrentWhenActive="page">Integraciones y aprobaciones</a></nav>
      <p>
        Biblioteca privada de archivos. Los documentos siguen protegidos por
        Core.Files y no salen de cuarentena hasta completar su análisis.
      </p>
      @if (error()) {
        <p role="alert">{{ error() }}</p>
      }
      @if (message()) {
        <p role="status">{{ message() }}</p>
      }

      <section class="filters bs-frame" aria-label="Filtros de biblioteca">
        <label
          >Biblioteca
          <select
            [(ngModel)]="scope"
            (change)="changeScope()"
            [disabled]="busy() || pendingUpload()"
          >
            <option value="company">Empresa</option>
            <option value="person">Mis archivos personales</option>
          </select>
        </label>
        <label
          >Buscar<input
            [(ngModel)]="search"
            maxlength="100"
            (keyup.enter)="page = 1; load()"
        /></label>
        <button type="button" (click)="page = 1; load()" [disabled]="busy()">
          Buscar
        </button>
        <label class="check"
          ><input
            type="checkbox"
            [(ngModel)]="trash"
            (change)="changeTrash()"
            [disabled]="busy() || pendingUpload()"
          />Papelera</label
        >
      </section>

      <section class="folders bs-frame" aria-label="Carpetas">
        <div class="folder-heading">
          <div>
            <h2>Carpetas</h2>
            <p>{{ scopeLabel() }} · {{ currentFolderName }}</p>
          </div>
          @if (currentFolder) {
            <button type="button" (click)="goRoot()" [disabled]="busy()">
              Volver a la raíz
            </button>
          }
        </div>
        @if (!trash) {
          @if (canOrganize()) {
            <div class="new-folder">
              <label
                >Nueva carpeta<input
                  [(ngModel)]="newFolderName"
                  maxlength="120"
              /></label>
              @if (scope === "company") {
                <label
                  >Acceso de equipo<select [(ngModel)]="newFolderTeam">
                    <option value="">Empresa</option>
                    @for (t of teams(); track t.id) {
                      <option [value]="t.id">{{ t.name }}</option>
                    }
                  </select></label
                >
              }
              <button
                type="button"
                (click)="createFolder()"
                [disabled]="busy() || !newFolderName.trim()"
              >
                Crear carpeta
              </button>
            </div>
          }
        }
        <ul>
          @for (folder of folders(); track folder.id) {
            <li>
              @if (!trash) {
                @if (editingFolder() === folder.id) {
                  <label
                    >Nombre de carpeta<input
                      [(ngModel)]="folderName"
                      maxlength="120"
                  /></label>
                  <button
                    type="button"
                    (click)="saveFolder(folder)"
                    [disabled]="busy() || !folderName.trim()"
                  >
                    Guardar nombre
                  </button>
                  <button
                    type="button"
                    (click)="editingFolder.set('')"
                    [disabled]="busy()"
                  >
                    Cancelar
                  </button>
                } @else {
                  <button
                    class="folder-link"
                    type="button"
                    (click)="openFolder(folder)"
                  >
                    {{ folder.name }} <span>({{ folder.file_count }})</span>
                  </button>
                  @if (canOrganize()) {
                    <button type="button" (click)="startFolderRename(folder)">
                      Renombrar carpeta
                    </button>
                  }
                }
                @if (canDelete()) {
                  <button
                    type="button"
                    (click)="trashFolder(folder)"
                    [disabled]="busy()"
                  >
                    Enviar carpeta a papelera
                  </button>
                }
              } @else {
                <span>{{ folder.name }}</span>
                @if (canDelete()) {
                  <button
                    type="button"
                    (click)="restoreFolder(folder)"
                    [disabled]="busy()"
                  >
                    Restaurar carpeta
                  </button>
                }
              }
            </li>
          } @empty {
            <li>No hay carpetas en esta ubicación.</li>
          }
        </ul>
      </section>

      @if (!trash && canUpload()) {
        <section class="upload bs-frame" aria-label="Subir archivo">
          <h2>Subir archivo a esta ubicación</h2>
          <label
            >Título<input
              [(ngModel)]="title"
              maxlength="180"
              [disabled]="busy() || pendingUpload()"
          /></label>
          <label
            >Notas<textarea
              [(ngModel)]="notes"
              maxlength="4000"
              rows="2"
              [disabled]="busy() || pendingUpload()"
            ></textarea>
          </label>
          <label
            >Archivo<input
              type="file"
              [attr.accept]="accept"
              (change)="selectFile($event)"
              [disabled]="busy() || pendingUpload()"
          /></label>
          <button
            type="button"
            (click)="upload()"
            [disabled]="busy() || !file || (!pendingUpload() && !title.trim())"
          >
            {{
              pendingUpload()
                ? "Reintentar carga pendiente"
                : "Cargar y registrar en biblioteca"
            }}
          </button>
        </section>
      }

      @if (
        canUpload() || canEditEntry() || canManageFiles() || canDelete() || canComment() || canModerate()
      ) {
        <details class="reauth">
          <summary>
            Confirmar identidad para vincular, editar, eliminar o moderar contenido
          </summary>
          <bastion-reauthentication />
        </details>
      }

      <section aria-label="Notificaciones de menciones">
        <h2>Menciones</h2>
        <button
          type="button"
          (click)="loadNotifications()"
          [disabled]="busy()"
        >
          Actualizar menciones
        </button>
        <ul>
          @for (notification of notifications(); track notification.id) {
            <li>
              Mención en un recurso · {{ notification.created_at }}
              <button
                type="button"
                (click)="openNotification(notification)"
                [disabled]="busy()"
              >
                Abrir conversación
              </button>
              @if (!notification.read_at) {
                <button
                  type="button"
                  (click)="readNotification(notification)"
                  [disabled]="busy()"
                >
                  Marcar como leída
                </button>
              }
            </li>
          } @empty {
            <li>No hay menciones disponibles.</li>
          }
        </ul>
        @if (notificationsMore()) {
          <button
            type="button"
            (click)="loadNotifications(false)"
            [disabled]="busy()"
          >
            Cargar más menciones
          </button>
        }
      </section>

      @if (conversationEntry(); as conversation) {
        <section aria-label="Conversación del recurso" class="conversation bs-frame">
          <div class="file-heading">
            <h2>Conversación: {{ conversation.title }}</h2>
            <button type="button" (click)="closeConversation()">
              Cerrar
            </button>
          </div>
          <h3>Comentarios</h3>
          <ul class="bs-timeline">
            @for (comment of comments(); track comment.id) {
              <li class="bs-tl-row">
                <time class="bs-when">{{ comment.created_at }}</time><div><strong>{{ comment.author_name }}</strong><p>{{ comment.body }}</p></div>
                @if (
                  comment.author_id === currentUserID() && canComment() ||
                  canModerate()
                ) {
                  <button
                    type="button"
                    (click)="removeComment(comment)"
                    [disabled]="busy()"
                  >
                    Enviar comentario a papelera
                  </button>
                }
              </li>
            } @empty {
              <li>No hay comentarios en este recurso.</li>
            }
          </ul>
          @if (canComment()) {
            <label
              >Comentario<textarea
                [(ngModel)]="commentBody"
                maxlength="4000"
                rows="3"
                [disabled]="busy()"
              ></textarea
            ></label>
            <label
              >Mencionar usuarios
              <input
                [(ngModel)]="mentionSearch"
                maxlength="100"
                placeholder="Buscar por nombre"
                [disabled]="busy()"
              />
            </label>
            <button
              type="button"
              (click)="findMentionableUsers()"
              [disabled]="busy()"
            >
              Buscar personas
            </button>
            <label
              >Destinatarios<select
                multiple
                [(ngModel)]="mentionUserIDs"
                [disabled]="busy()"
              >
                @for (user of mentionableUsers(); track user.id) {
                  <option [value]="user.id">{{ user.name }}</option>
                }
              </select></label
            >
            <button
              type="button"
              (click)="postComment()"
              [disabled]="busy() || !commentBody.trim()"
            >
              Publicar comentario
            </button>
          }
          <h3>Actividad</h3>
          <ul>
            @for (item of activity(); track item.id) {
              <li>
                {{ item.actor_name }}
                {{ item.action === "comment_added" ? "agregó un comentario" : "envió un comentario a papelera" }}
                · {{ item.created_at }}
              </li>
            } @empty {
              <li>No hay actividad disponible.</li>
            }
          </ul>
        </section>
      }

      <section class="bs-frame bs-library-list" aria-label="Archivos de la biblioteca">
        <div class="file-heading">
          <h2>{{ trash ? "Archivos en papelera" : "Archivos" }}</h2>
          <span>{{ total() }} resultados</span>
        </div>
        @for (entry of entries(); track entry.id) {
          <article class="entry bs-item">
            @if (editing() === entry.id) {
              <label
                >Título<input [(ngModel)]="editTitle" maxlength="180"
              /></label>
              <label
                >Notas<textarea
                  [(ngModel)]="editNotes"
                  maxlength="4000"
                  rows="2"
                ></textarea>
              </label>
              <label
                >Carpeta<select [(ngModel)]="editFolderID">
                  <option value="">Sin carpeta</option>
                  @for (folder of folders(); track folder.id) {
                    @if (
                      folder.scope === entry.scope &&
                      (folder.person_id || "") === (entry.person_id || "") &&
                      (folder.team_id || "") === (entry.team_id || "")
                    ) {
                      <option [value]="folder.id">{{ folder.name }}</option>
                    }
                  }
                </select></label
              >
              <button
                type="button"
                (click)="saveEntry(entry)"
                [disabled]="busy()"
              >
                Guardar cambios
              </button>
              <button
                type="button"
                (click)="editing.set('')"
                [disabled]="busy()"
              >
                Cancelar
              </button>
            } @else {
              <div class="entry-title">
                <strong>{{ entry.title }}</strong
                ><span>{{ entry.filename }}</span>
              </div>
              @if (entry.notes) {
                <p>{{ entry.notes }}</p>
              }
              <p class="meta">
                {{ size(entry.size_bytes) }} · {{ status(entry.status) }} ·
                versión de archivo {{ entry.file_version }}
                @if (entry.team_id) {
                  · equipo
                }
              </p>
              <div class="actions">
                @if (
                  !trash &&
                  entry.status === "available" &&
                  previewable(entry.media_type)
                ) {
                  <button type="button" (click)="deliver(entry, 'preview')">
                    Vista previa
                  </button>
                }
                @if (!trash && entry.status === "available") {
                  <button type="button" (click)="deliver(entry, 'download')">
                    Descargar
                  </button>
                }
                @if (!trash && canEditEntry()) {
                  <button type="button" (click)="startEdit(entry)">
                    Editar ficha
                  </button>
                }
                @if (!trash && canManageFiles()) {
                  <button
                    type="button"
                    (click)="trashEntry(entry)"
                    [disabled]="busy()"
                  >
                    Enviar a papelera
                  </button>
                }
                @if (trash && canManageFiles()) {
                  <button
                    type="button"
                    (click)="restoreEntry(entry)"
                    [disabled]="busy()"
                  >
                    Restaurar
                  </button>
                }
                @if (!trash) {
                  <button type="button" (click)="showVersions(entry)">
                    Historial de versiones
                  </button>
                  <button
                    type="button"
                    (click)="openConversation(entry)"
                    [disabled]="busy()"
                  >
                    Conversación y actividad
                  </button>
                }
                @if (!trash && canUpload() && entry.status === "available") {
                  @if (versionTarget() === entry.id) {
                    <label
                      >Archivo de nueva versión<input
                        type="file"
                        [attr.accept]="accept"
                        (change)="selectVersionFile($event, entry)"
                        [disabled]="busy() || versionUploadPending()"
                    /></label>
                    <button
                      type="button"
                      (click)="uploadVersion(entry)"
                      [disabled]="busy() || !versionFile"
                    >
                      {{
                        versionUploadPending()
                          ? "Reintentar versión"
                          : "Cargar nueva versión"
                      }}
                    </button>
                  } @else {
                    <button
                      type="button"
                      (click)="startVersionUpload(entry)"
                      [disabled]="busy() || versionUploadPending()"
                    >
                      Subir nueva versión
                    </button>
                  }
                }
              </div>
              @if (versionsFor() === entry.id) {
                <ul class="versions">
                  @for (v of versions(); track v.file_version) {
                    <li>
                      v{{ v.file_version }} · {{ v.filename }} ·
                      {{ size(v.size_bytes) }} · {{ status(v.status) }}
                    </li>
                  }
                </ul>
              }
            }
          </article>
        } @empty {
          <p>No hay archivos para mostrar en esta ubicación.</p>
        }
        <div class="pagination">
          <button
            type="button"
            (click)="page = page - 1; load()"
            [disabled]="busy() || page <= 1"
          >
            Anterior
          </button>
          <span>Página {{ page }}</span>
          <button
            type="button"
            (click)="page = page + 1; load()"
            [disabled]="busy() || page * perPage >= total()"
          >
            Siguiente
          </button>
        </div>
      </section>
    </section>
  `,
  styles: [
    `
      .workspace {
        display: grid;
        gap: 1rem;
        width:100%;
      }
      .filters,
      .new-folder,
      .folder-heading,
      .file-heading,
      .actions,
      .pagination {
        display: flex;
        flex-wrap: wrap;
        gap: 0.75rem;
        align-items: end;
      }
      .folders,
      .upload,
      .entry {
        border: 1px solid var(--line);
        border-radius:0;
        padding: 1rem;
      }
      .folders h2,
      .upload h2,
      .file-heading h2 {
        margin: 0.1rem 0 0.5rem;
      }
      .folder-heading,
      .file-heading {
        justify-content: space-between;
        align-items: center;
      }
      .folders ul,
      .versions {
        display: grid;
        gap: 0.5rem;
        padding-left: 1.25rem;
      }
      .folders li {
        padding: 0.25rem 0;
      }
      .folder-link {
        border: 0;
        padding: 0;
        background: transparent;
        color: var(--ink);
        font-weight: 600;
        text-decoration: underline;
        cursor: pointer;
      }
      label {
        display: grid;
        gap: 0.3rem;
        min-width: 12rem;
        flex: 1;
      }
      .check {
        display: flex;
        align-items: center;
        min-width: auto;
      }
      .check input {
        width: auto;
      }
      .upload {
        display: grid;
        gap: 0.7rem;
      }
      .entry {
        margin: 0.5rem 0;
        display: grid;
        gap: 0.5rem;
      }
      .entry-title {
        display: flex;
        flex-wrap: wrap;
        gap: 0.5rem 1rem;
        align-items: baseline;
      }
      .meta,
      .folder-heading p {
        color: var(--ink-subtle);
        margin: 0.2rem 0;
      }
      .reauth {
        border: 1px solid var(--line);
        padding: 0.75rem;
      }
      .pagination {
        justify-content: center;
        align-items: center;
      }
    `,
  ],
})
export class Collaboration implements OnInit {
  api = inject(Client);
  entries = signal<Entry[]>([]);
  folders = signal<Folder[]>([]);
  teams = signal<Team[]>([]);
  versions = signal<
    Array<{
      file_version: number;
      filename: string;
      media_type: string;
      size_bytes: number;
      status: string;
    }>
  >([]);
  conversationEntry = signal<Entry | null>(null);
  comments = signal<Comment[]>([]);
  activity = signal<ActivityItem[]>([]);
  mentionableUsers = signal<MentionableUser[]>([]);
  notifications = signal<CollaborationNotification[]>([]);
  notificationAfterID = signal("0");
  notificationsMore = signal(false);
  total = signal(0);
  busy = signal(false);
  error = signal("");
  message = signal("");
  editing = signal("");
  editingFolder = signal("");
  versionsFor = signal("");
  versionTarget = signal("");
  scope: "company" | "person" = "company";
  search = "";
  page = 1;
  readonly perPage = 25;
  trash = false;
  currentFolder = "";
  currentFolderName = "Raíz";
  currentFolderTeam = "";
  newFolderName = "";
  newFolderTeam = "";
  title = "";
  notes = "";
  editTitle = "";
  editNotes = "";
  editFolderID = "";
  folderName = "";
  commentBody = "";
  mentionSearch = "";
  mentionUserIDs: string[] = [];
  file: File | null = null;
  versionFile: File | null = null;
  readonly accept = ACCEPT;
  private pending = new Map<string, Pending>();
  private uploadState: UploadState | null = null;
  private versionUploadState: VersionUploadState | null = null;
  private loadToken = 0;

  async ngOnInit() {
    const userID = this.api.profile()?.user.id;
    if (!userID) return;
    await this.loadTeams();
    await this.load();
    await this.loadNotifications();
  }

  canUpload() {
    return (
      this.api.has("colaboracion.upload") &&
      this.api.has("colaboracion.organize") &&
      this.api.has("core.documents.write")
    );
  }
  canOrganize() {
    return this.api.has("colaboracion.organize");
  }
  canEditEntry() {
    return (
      this.api.has("colaboracion.organize") &&
      this.api.has("core.documents.write")
    );
  }
  canDelete() {
    return this.api.has("colaboracion.delete");
  }
  canComment() {
    return this.api.has("colaboracion.comment");
  }
  canModerate() {
    return this.api.has("colaboracion.moderate");
  }
  currentUserID() {
    return this.api.profile()?.user.id || "";
  }
  canManageFiles() {
    return (
      this.api.has("colaboracion.delete") &&
      this.api.has("core.documents.write")
    );
  }
  pendingUpload() {
    return this.uploadState !== null;
  }
  scopeLabel() {
    return this.scope === "person" ? "Mis archivos" : "Biblioteca de empresa";
  }

  private query(path: string, params: Record<string, string>) {
    const q = new URLSearchParams(params);
    return this.api.get<{
      data: Entry[];
      total: number;
      page: number;
      per_page: number;
    }>(path + "?" + q.toString());
  }

  async load() {
    const token = ++this.loadToken;
    this.busy.set(true);
    this.error.set("");
    const personID =
      this.scope === "person" ? this.api.profile()?.user.id || "" : "";
    const params: Record<string, string> = {
      scope: this.scope,
      search: this.search,
      page: String(this.page),
      per_page: String(this.perPage),
      deleted: String(this.trash),
    };
    // The root-level trash view spans all folders. Sending folder_id=root here
    // would hide deleted entries that were stored inside a folder.
    if (!this.trash || this.currentFolder) {
      params["folder_id"] = this.currentFolder || "root";
    }
    if (personID) params["person_id"] = personID;
    try {
      const [page, folders] = await Promise.all([
        this.query("/collaboration", params),
        this.queryFolders(personID),
      ]);
      if (token !== this.loadToken) return;
      this.entries.set(page.data);
      this.total.set(page.total);
      this.folders.set(folders);
    } catch (e) {
      if (token === this.loadToken) {
        this.error.set(this.api.message(e));
        this.entries.set([]);
        this.total.set(0);
        this.folders.set([]);
      }
    } finally {
      if (token === this.loadToken) this.busy.set(false);
    }
  }

  private async queryFolders(personID: string) {
    const q = new URLSearchParams({
      scope: this.scope,
      parent_id: this.currentFolder || "root",
      deleted: String(this.trash),
    });
    if (personID) q.set("person_id", personID);
    return (
      await this.api.get<{ data: Folder[] }>(
        "/collaboration/folders?" + q.toString(),
      )
    ).data;
  }

  async loadTeams() {
    try {
      this.teams.set(
        (await this.api.get<{ data: Team[] }>("/collaboration/teams")).data,
      );
    } catch (e) {
      this.error.set(this.api.message(e));
    }
  }

  async loadNotifications(reset = true) {
    const after = reset ? "0" : this.notificationAfterID();
    try {
      const page = await this.api.get<{
        data: CollaborationNotification[];
        next_after_id: string;
        has_more: boolean;
      }>(`/collaboration/notifications?after_id=${after}&limit=50`);
      this.notifications.set(
        reset ? page.data : [...this.notifications(), ...page.data],
      );
      this.notificationAfterID.set(page.next_after_id || after);
      this.notificationsMore.set(page.has_more && page.next_after_id !== after);
    } catch (e) {
      this.error.set(this.api.message(e));
      if (reset) this.notifications.set([]);
      this.notificationsMore.set(false);
    }
  }

  async openConversation(entry: Entry) {
    if (this.trash || entry.deleted) return;
    this.conversationEntry.set(entry);
    this.comments.set([]);
    this.activity.set([]);
    this.mentionableUsers.set([]);
    this.commentBody = "";
    this.mentionUserIDs = [];
    this.error.set("");
    await this.loadConversation(entry);
  }

  closeConversation() {
    this.conversationEntry.set(null);
    this.comments.set([]);
    this.activity.set([]);
    this.mentionableUsers.set([]);
    this.commentBody = "";
    this.mentionUserIDs = [];
  }

  private async loadConversation(entry: Entry) {
    const path = `/collaboration/resources/collaboration/${entry.id}`;
    try {
      const [comments, activity, users] = await Promise.all([
        this.api.get<{ data: Comment[] }>(`${path}/comments?page=1&per_page=50`),
        this.api.get<{ data: ActivityItem[] }>(
          `${path}/activity?page=1&per_page=50`,
        ),
        this.canComment()
          ? this.api.get<{ data: MentionableUser[] }>(
              `${path}/mentionable-users`,
            )
          : Promise.resolve({ data: [] as MentionableUser[] }),
      ]);
      if (this.conversationEntry()?.id !== entry.id) return;
      this.comments.set(comments.data);
      this.activity.set(activity.data);
      this.mentionableUsers.set(users.data);
    } catch (e) {
      if (this.conversationEntry()?.id === entry.id) {
        this.conversationEntry.set(null);
        this.comments.set([]);
        this.activity.set([]);
        this.mentionableUsers.set([]);
      }
      this.error.set(this.api.message(e));
    }
  }

  async findMentionableUsers() {
    const entry = this.conversationEntry();
    if (!entry || !this.canComment()) return;
    try {
      const query = new URLSearchParams({ search: this.mentionSearch });
      this.mentionableUsers.set(
        (
          await this.api.get<{ data: MentionableUser[] }>(
            `/collaboration/resources/collaboration/${entry.id}/mentionable-users?${query}`,
          )
        ).data,
      );
    } catch (e) {
      this.error.set(this.api.message(e));
    }
  }

  async postComment() {
    const entry = this.conversationEntry();
    if (!entry || !this.canComment() || !this.commentBody.trim()) return;
    await this.mutate(async () => {
      const path = `/collaboration/resources/collaboration/${entry.id}/comments`;
      const body = {
        body: this.commentBody.trim(),
        mention_user_ids: [...new Set(this.mentionUserIDs)].sort(),
      };
      await this.api.write("POST", path, body, this.key(path, body));
      this.clearKey(path);
      this.commentBody = "";
      this.mentionUserIDs = [];
      this.message.set("Comentario publicado.");
      await this.loadConversation(entry);
      await this.loadNotifications();
    });
  }

  async removeComment(comment: Comment) {
    const entry = this.conversationEntry();
    if (!entry) return;
    await this.mutate(async () => {
      const path = `/collaboration/comments/${comment.id}`;
      const body = { version: comment.version };
      await this.api.write("DELETE", path, body, this.key(path, body));
      this.clearKey(path);
      this.message.set("Comentario enviado a papelera.");
      await this.loadConversation(entry);
      await this.loadNotifications();
    });
  }

  async openNotification(notification: CollaborationNotification) {
    if (notification.resource_type !== "collaboration") {
      this.error.set("El recurso ya no está disponible.");
      return;
    }
    try {
      const entry = await this.api.get<Entry>(
        `/collaboration/${notification.resource_id}`,
      );
      this.scope = entry.scope;
      this.currentFolder = entry.folder_id || "";
      this.currentFolderTeam = entry.team_id || "";
      this.currentFolderName = entry.folder_id ? "Carpeta seleccionada" : "Raíz";
      this.trash = false;
      this.page = 1;
      await this.load();
      await this.openConversation(entry);
    } catch (e) {
      this.error.set(this.api.message(e));
    }
  }

  async readNotification(notification: CollaborationNotification) {
    await this.mutate(async () => {
      const path = `/collaboration/notifications/${notification.id}/read`;
      const body = {};
      await this.api.write("POST", path, body, this.key(path, body));
      this.clearKey(path);
      await this.loadNotifications();
    });
  }

  async changeScope() {
    if (this.uploadState) {
      this.scope = this.uploadState.scope;
      this.error.set(
        "Complete la carga pendiente antes de cambiar de biblioteca.",
      );
      return;
    }
    this.currentFolder = "";
    this.closeConversation();
    this.currentFolderName = "Raíz";
    this.currentFolderTeam = "";
    this.newFolderTeam = "";
    this.page = 1;
    this.trash = false;
    this.newFolderTeam = "";
    await this.load();
  }
  async changeTrash() {
    if (this.uploadState) {
      this.trash = false;
      this.error.set("Complete la carga pendiente antes de abrir la papelera.");
      return;
    }
    this.currentFolder = "";
    this.closeConversation();
    this.currentFolderName = "Raíz";
    this.currentFolderTeam = "";
    this.newFolderTeam = "";
    this.page = 1;
    this.editing.set("");
    await this.load();
  }
  async goRoot() {
    if (this.uploadState) {
      this.error.set(
        "Complete la carga pendiente antes de cambiar de carpeta.",
      );
      return;
    }
    this.currentFolder = "";
    this.closeConversation();
    this.currentFolderName = "Raíz";
    this.currentFolderTeam = "";
    this.newFolderTeam = "";
    this.page = 1;
    await this.load();
  }
  async openFolder(folder: Folder) {
    if (this.uploadState) {
      this.error.set(
        "Complete la carga pendiente antes de cambiar de carpeta.",
      );
      return;
    }
    this.currentFolder = folder.id;
    this.closeConversation();
    this.currentFolderName = folder.name;
    this.currentFolderTeam = folder.team_id || "";
    this.newFolderTeam = this.currentFolderTeam;
    this.page = 1;
    await this.load();
  }

  private key(operation: string, body: unknown) {
    const encoded = JSON.stringify(body),
      previous = this.pending.get(operation);
    if (previous?.body === encoded) return previous.key;
    const key = crypto.randomUUID();
    this.pending.set(operation, { body: encoded, key });
    return key;
  }
  private clearKey(operation: string) {
    this.pending.delete(operation);
  }

  async createFolder() {
    const parentID = this.currentFolder;
    const personID =
      this.scope === "person" ? this.api.profile()?.user.id || "" : "";
    const body = {
      scope: this.scope,
      person_id: personID,
      team_id: this.scope === "company" ? this.newFolderTeam : "",
      parent_id: parentID,
      name: this.newFolderName.trim(),
    };
    await this.mutate(async () => {
      await this.api.write(
        "POST",
        "/collaboration/folders",
        body,
        this.key("folder:create:" + parentID + ":" + body.name, body),
      );
      this.clearKey("folder:create:" + parentID + ":" + body.name);
      this.newFolderName = "";
      this.newFolderTeam = this.currentFolderTeam;
      this.message.set("Carpeta creada.");
      await this.load();
    });
  }

  async trashFolder(folder: Folder) {
    await this.mutate(async () => {
      const path = "/collaboration/folders/" + folder.id,
        body = { version: folder.version };
      await this.api.write("DELETE", path, body, this.key(path, body));
      this.clearKey(path);
      this.message.set("Carpeta enviada a papelera.");
      await this.load();
    });
  }
  async restoreFolder(folder: Folder) {
    await this.mutate(async () => {
      const path = "/collaboration/folders/" + folder.id + "/restore",
        body = { version: folder.version };
      await this.api.write("POST", path, body, this.key(path, body));
      this.clearKey(path);
      this.message.set("Carpeta restaurada.");
      await this.load();
    });
  }

  startFolderRename(folder: Folder) {
    this.editingFolder.set(folder.id);
    this.folderName = folder.name;
  }
  async saveFolder(folder: Folder) {
    await this.mutate(async () => {
      const path = "/collaboration/folders/" + folder.id;
      const body = { version: folder.version, name: this.folderName.trim() };
      await this.api.write("PATCH", path, body, this.key(path, body));
      this.clearKey(path);
      if (this.currentFolder === folder.id) this.currentFolderName = body.name;
      this.editingFolder.set("");
      this.message.set("Carpeta renombrada.");
      await this.load();
    });
  }

  startVersionUpload(entry: Entry) {
    this.versionTarget.set(entry.id);
    this.versionFile = null;
    this.error.set("");
  }
  versionUploadPending() {
    return this.versionUploadState !== null;
  }
  selectVersionFile(event: Event, entry: Entry) {
    const file = (event.target as HTMLInputElement).files?.[0] || null;
    if (
      this.versionUploadState &&
      (file !== this.versionUploadState.file ||
        entry.id !== this.versionUploadState.entryID)
    ) {
      this.error.set(
        "Complete la versión pendiente antes de seleccionar otro archivo.",
      );
      (event.target as HTMLInputElement).value = "";
      return;
    }
    this.versionTarget.set(entry.id);
    this.versionFile = file;
  }
  async uploadVersion(entry: Entry) {
    const file = this.versionFile;
    if (!file || this.busy()) return;
    this.busy.set(true);
    this.error.set("");
    this.message.set("");
    try {
      if (file.size < 1 || file.size > 20 * 1024 * 1024)
        throw new Error("El archivo debe tener entre 1 byte y 20 MB.");
      const ext = file.name.split(".").pop()?.toLowerCase() || "";
      const media = MEDIA[ext];
      if (!media) throw new Error("Tipo de archivo no permitido.");
      const data = await file.arrayBuffer();
      const hash = await crypto.subtle.digest("SHA-256", data);
      const checksum = Array.from(new Uint8Array(hash))
        .map((value) => value.toString(16).padStart(2, "0"))
        .join("");
      if (!this.versionUploadState) {
        this.versionUploadState = {
          file,
          entryID: entry.id,
          documentID: entry.document_id,
          documentVersion: entry.document_version,
          uploadUrl: "",
          checksum: "",
          stage: "requesting",
        };
      }
      const state = this.versionUploadState;
      if (
        !state ||
        state.entryID !== entry.id ||
        state.documentID !== entry.document_id ||
        state.file !== file
      )
        throw new Error("La versión pendiente pertenece a otro archivo.");
      const body = {
        filename: file.name,
        media_type: media,
        size_bytes: file.size,
        checksum,
        version: state.documentVersion,
      };
      if (state.stage === "requesting") {
        const path = "/documents/" + entry.document_id;
        const created = await this.api.write<{
          id: string;
          upload_url: string;
          checksum_base64: string;
        }>(
          "POST",
          path,
          body,
          this.key("collaboration:version:" + entry.id, body),
        );
        state.uploadUrl = created.upload_url;
        state.checksum = created.checksum_base64;
        state.stage = "created";
      }
      if (state.stage === "created") {
        const uploaded = await fetch(state.uploadUrl, {
          method: "PUT",
          credentials: "omit",
          headers: {
            "Content-Type": media,
            "x-amz-checksum-sha256": state.checksum,
          },
          body: data,
        });
        if (!uploaded.ok)
          throw new Error("No se confirmó la carga privada de la versión.");
        state.stage = "uploaded";
      }
      if (state.stage === "uploaded") {
        await this.api.write(
          "POST",
          "/documents/" + state.documentID + "/finalize",
        );
      }
      this.clearKey("collaboration:version:" + entry.id);
      this.versionUploadState = null;
      this.versionFile = null;
      this.versionTarget.set("");
      this.message.set(
        "Nueva versión cargada y enviada a análisis de seguridad.",
      );
      await this.load();
    } catch (e) {
      this.error.set(this.api.message(e));
    } finally {
      this.busy.set(false);
    }
  }

  selectFile(event: Event) {
    const file = (event.target as HTMLInputElement).files?.[0] || null;
    if (this.uploadState && file !== this.uploadState.file) {
      this.error.set(
        "Complete la carga pendiente antes de seleccionar otro archivo.",
      );
      (event.target as HTMLInputElement).value = "";
      return;
    }
    this.file = file;
  }

  async upload() {
    const file = this.file;
    if (!file || this.busy()) return;
    const scope = this.scope,
      folderID = this.currentFolder,
      title = this.title.trim(),
      notes = this.notes.trim();
    this.busy.set(true);
    this.error.set("");
    this.message.set("");
    try {
      if (file.size < 1 || file.size > 20 * 1024 * 1024)
        throw new Error("El archivo debe tener entre 1 byte y 20 MB.");
      const ext = file.name.split(".").pop()?.toLowerCase() || "",
        media = MEDIA[ext];
      if (!media) throw new Error("Tipo de archivo no permitido.");
      const data = await file.arrayBuffer(),
        hash = await crypto.subtle.digest("SHA-256", data),
        checksum = Array.from(new Uint8Array(hash))
          .map((v) => v.toString(16).padStart(2, "0"))
          .join("");
      const teamID = this.currentFolderTeam;
      const createBody = {
        filename: file.name,
        media_type: media,
        size_bytes: file.size,
        checksum,
        version: "0",
        ...(teamID ? { team_id: teamID } : {}),
      };
      if (!this.uploadState) {
        this.uploadState = {
          file,
          scope,
          folderID,
          teamID,
          id: "",
          uploadUrl: "",
          checksum: "",
          version: "",
          stage: "requesting",
        };
      }
      const state = this.uploadState;
      if (!state || state.file !== file) {
        throw new Error("La carga pendiente pertenece a otro archivo.");
      }
      if (
        state.scope !== scope ||
        state.folderID !== folderID ||
        state.teamID !== teamID
      )
        throw new Error(
          "La carga pendiente pertenece a otra ubicación. Complete la carga anterior o seleccione el mismo destino.",
        );
      if (state.stage === "requesting") {
        const op = "collaboration:upload:" + state.scope + ":" + state.folderID;
        const created = await this.api.write<{
          id: string;
          upload_url: string;
          checksum_base64: string;
          version: string;
        }>("POST", "/documents", createBody, this.key(op, createBody));
        state.id = created.id;
        state.uploadUrl = created.upload_url;
        state.checksum = created.checksum_base64;
        state.version = created.version;
        state.stage = "created";
      }
      if (state.stage === "created") {
        const result = await fetch(state.uploadUrl, {
          method: "PUT",
          credentials: "omit",
          headers: {
            "Content-Type": media,
            "x-amz-checksum-sha256": state.checksum,
          },
          body: data,
        });
        if (!result.ok)
          throw new Error("No se confirmó la carga privada del archivo.");
        state.stage = "uploaded";
      }
      if (state.stage === "uploaded") {
        await this.api.write("POST", "/documents/" + state.id + "/finalize");
        state.stage = "verified";
      }
      const personID =
        scope === "person" ? this.api.profile()?.user.id || "" : "";
      state.bindPayload ??= {
        document_id: state.id,
        document_version: state.version,
        scope: state.scope,
        person_id: state.scope === "person" ? personID : "",
        folder_id: state.folderID,
        title,
        notes,
      };
      const bindBody = state.bindPayload;
      const bindPath = "/collaboration";
      await this.api.write(
        "POST",
        bindPath,
        bindBody,
        this.key("collaboration:bind:" + state.id, bindBody),
      );
      this.clearKey("collaboration:bind:" + state.id);
      this.uploadState = null;
      this.file = null;
      this.title = "";
      this.notes = "";
      this.message.set(
        "Archivo cargado y registrado. Queda sujeto al análisis de seguridad de Core.Files.",
      );
      await this.load();
    } catch (e) {
      this.error.set(this.api.message(e));
    } finally {
      this.busy.set(false);
    }
  }

  startEdit(entry: Entry) {
    this.editing.set(entry.id);
    this.editTitle = entry.title;
    this.editNotes = entry.notes || "";
    this.editFolderID = entry.folder_id || "";
  }
  async saveEntry(entry: Entry) {
    await this.mutate(async () => {
      const path = "/collaboration/" + entry.id,
        body = {
          version: entry.version,
          folder_id: this.editFolderID,
          title: this.editTitle.trim(),
          notes: this.editNotes.trim(),
        };
      await this.api.write("PATCH", path, body, this.key(path, body));
      this.clearKey(path);
      this.editing.set("");
      this.message.set("Ficha actualizada.");
      await this.load();
    });
  }
  async trashEntry(entry: Entry) {
    await this.mutate(async () => {
      const path = "/collaboration/" + entry.id,
        body = { version: entry.document_version };
      await this.api.write("DELETE", path, body, this.key(path, body));
      this.clearKey(path);
      this.message.set("Archivo enviado a papelera.");
      if (this.conversationEntry()?.id === entry.id) this.closeConversation();
      await this.load();
    });
  }
  async restoreEntry(entry: Entry) {
    await this.mutate(async () => {
      const path = "/collaboration/" + entry.id + "/restore",
        body = { version: entry.document_version };
      await this.api.write("POST", path, body, this.key(path, body));
      this.clearKey(path);
      this.message.set("Archivo restaurado.");
      await this.load();
    });
  }
  private async mutate(action: () => Promise<void>) {
    if (this.busy()) return;
    this.busy.set(true);
    this.error.set("");
    this.message.set("");
    try {
      await action();
    } catch (e) {
      this.error.set(this.api.message(e));
    } finally {
      this.busy.set(false);
    }
  }

  async deliver(entry: Entry, action: "preview" | "download") {
    try {
      const result = await this.api.get<{ url: string }>(
        `/collaboration/${entry.id}/${action}`,
      );
      const url = new URL(result.url);
      if (url.protocol !== "https:")
        throw new Error("Enlace de archivo inválido.");
      window.open(url.href, "_blank", "noopener,noreferrer");
    } catch (e) {
      this.error.set(this.api.message(e));
    }
  }
  async showVersions(entry: Entry) {
    if (this.versionsFor() === entry.id) {
      this.versionsFor.set("");
      return;
    }
    try {
      this.versions.set(
        await this.api.get<
          Array<{
            file_version: number;
            filename: string;
            media_type: string;
            size_bytes: number;
            status: string;
          }>
        >(`/collaboration/${entry.id}/versions`),
      );
      this.versionsFor.set(entry.id);
    } catch (e) {
      this.error.set(this.api.message(e));
    }
  }
  previewable(media: string) {
    return [
      "application/pdf",
      "image/png",
      "image/jpeg",
      "text/plain",
    ].includes(media);
  }
  status(value: string) {
    return (
      (
        {
          pending: "Carga pendiente",
          quarantined: "En cuarentena",
          available: "Disponible",
          rejected: "Rechazado",
        } as Record<string, string>
      )[value] || value
    );
  }
  size(value: number) {
    if (value < 1024) return `${value} B`;
    if (value < 1024 * 1024) return `${(value / 1024).toFixed(1)} KB`;
    return `${(value / 1024 / 1024).toFixed(1)} MB`;
  }
}
