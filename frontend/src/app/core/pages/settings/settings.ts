import { Component, inject, signal } from "@angular/core";
import { FormsModule } from "@angular/forms";
import { Personal } from "./personal";
import { FilePanel } from "./files";
import { Client, User, Grant } from "../../api/client";
interface Role {
  id: string;
  name: string;
  version: string;
  grants: Grant[];
}
interface Team {
  id: string;
  name: string;
  version: string;
  members: string[];
}
interface Setting {
  name: string;
  version: string;
  value: Record<string, unknown>;
  has_secret: boolean;
}
@Component({
  imports: [FormsModule, Personal, FilePanel],
  template: `
    <header class="bs-page-head"><div><span class="bs-label">Administración</span><h1>Configuración</h1></div></header><nav class="bs-section-menu" aria-label="Secciones de configuración"><a href="#personal">Personal</a>@if(api.has("core.users.read")){<a href="#usuarios">Usuarios</a>}@if(api.has("core.roles.read")){<a href="#roles">Roles</a>}@if(api.has("core.settings.read")){<a href="#empresa">Empresa y SMTP</a>}@if(api.has("core.settings.read")){<a href="#servicios">Servicios</a>}@if(api.has("core.audit.read")){<a href="#auditoria">Auditoría</a>}@if(api.has("core.documents.read")||api.has("core.documents.write")){<a href="#archivos">Archivos</a>}</nav>
    <section id="personal"><bastion-personal/></section>
    <p role="status" class="message">{{ message() }}</p>
    <section class="bs-frame">
      <h2>Confirmar identidad</h2>
      <p>
        Las operaciones sensibles requieren confirmación reciente de su contraseña.
      </p>
      <form (ngSubmit)="reauth()">
        <label
          >Contraseña<input
            name="reauth-password"
            type="password"
            autocomplete="current-password"
            [(ngModel)]="password"
            required /></label
        ><button [disabled]="busy()">Confirmar</button>
      </form>
    </section>
    @if (api.has("core.documents.read") || api.has("core.documents.write")) {
      <section id="archivos"><bastion-files/></section>
    }
    @if (api.has("core.users.read")) {
      <section class="bs-frame">
        <h2 id="usuarios">Usuarios</h2>
        <button (click)="loadUsers()">Actualizar</button>
        <div class="bs-table-wrap">
          <table class="bs-table">
            <thead>
              <tr>
                <th>Usuario</th>
                <th>Correo</th>
                <th>Estado</th>
                <th>MFA</th>
                <th>Acciones</th>
              </tr>
            </thead>
            <tbody>
              @for (u of users(); track u.id) {
                <tr>
                  <td>{{ u.name }}</td>
                  <td>{{ u.email }}</td>
                  <td>{{ u.active ? "Activo" : "Inactivo" }}</td>
                  <td>{{ u.mfa_enabled ? "Registrado" : "Pendiente" }}</td>
                  <td>
                    @if (api.has("core.users.manage")) {
                      <button
                        (click)="selectUser(u)"
                        [disabled]="u.id === api.profile()?.user?.id"
                      >
                        Editar</button
                      ><button (click)="revoke(u.id)">Revocar sesiones</button>
                    }
                  </td>
                </tr>
              }
            </tbody>
          </table>
        </div>
        @if (selectedUser) {
          <form (ngSubmit)="saveUser()">
            <h3>Editar {{ selectedUser.email }}</h3>
            <label
              >Nombre<input
                name="user-name"
                [(ngModel)]="selectedUser.name"
                required /></label
            ><label
              ><input
                name="user-active"
                type="checkbox"
                [(ngModel)]="selectedUser.active"
              />
              Cuenta activa</label
            ><label
              >Roles<select
                name="user-roles"
                multiple
                [(ngModel)]="userRoleIDs"
              >
                @for (role of roles(); track role.id) {
                  @if (role.id !== "1") {
                    <option [value]="role.id">{{ role.name }}</option>
                  }
                }
              </select></label
            ><button [disabled]="busy()">Guardar y revocar sesiones</button>
          </form>
        }
      </section>
    }
    @if (api.has("core.users.manage")) {
      <section class="bs-frame">
        <h2>Invitación y recuperación</h2>
        <form (ngSubmit)="invite()">
          <label
            >Correo<input
              name="invite-email"
              type="email"
              [(ngModel)]="inviteEmail"
              required /></label
          ><label
            >Operación<select name="invite-kind" [(ngModel)]="inviteKind">
              <option value="invite">Alta por invitación</option>
              <option value="reset">Restablecer contraseña</option>
            </select></label
          ><button [disabled]="busy()">Generar código</button>
        </form>
        @if (invitation()) {
          <p>
            Entregue este código por un canal autorizado. No se ha enviado
            correo.
          </p>
          <pre>{{ invitation() }}</pre>
          <button (click)="invitation.set('')">Ocultar código</button>
        }
        <h3>Restablecer MFA tras verificar identidad</h3>
        <form (ngSubmit)="resetMFA()">
          <label
            >Usuario<select name="mfa-user" [(ngModel)]="mfaUser" required>
              @for (u of users(); track u.id) {
                @if (u.id !== api.profile()?.user?.id) {
                  <option [value]="u.id">{{ u.name }} · {{ u.email }}</option>
                }
              }
            </select></label
          ><label
            >Referencia de verificación administrativa<input
              name="mfa-reference"
              minlength="10"
              [(ngModel)]="mfaReference"
              required /></label
          ><button [disabled]="busy()">
            Restablecer MFA y revocar accesos
          </button>
        </form>
      </section>
    }
    @if (api.has("core.roles.read")) {
      <section class="bs-frame">
        <h2 id="roles">Roles y permisos</h2>
        <button (click)="loadRoles()">Actualizar</button>
        <ul>
          @for (role of roles(); track role.id) {
            <li>
              {{ role.name }}
              <button (click)="selectRole(role)" [disabled]="role.id === '1'">
                Editar
              </button>
            </li>
          }
        </ul>
        @if (api.has("core.roles.manage")) {
          <form (ngSubmit)="saveRole()">
            <label
              >Nombre<input
                name="role-name"
                [(ngModel)]="roleName"
                required /></label
            ><label
              >Permisos (un permiso:alcance por línea)<textarea
                name="role-grants"
                [(ngModel)]="roleGrants"
                placeholder="core.documents.read:self"
              ></textarea>
            </label>
            <p>
              Alcances: all, self, team. Los permisos administrativos requieren
              all.
            </p>
            <details>
              <summary>Permisos disponibles</summary>
              <pre>{{
                permissions().join(
                  "
"
                )
              }}</pre>
            </details>
            <button [disabled]="busy()">Guardar rol</button
            ><button type="button" (click)="clearRole()">Nuevo</button>
          </form>
        }
      </section>
    }
    @if (api.has("core.teams.read")) {
      <section class="bs-frame">
        <h2>Equipos</h2>
        <button (click)="loadTeams()">Actualizar</button>
        <ul>
          @for (t of teams(); track t.id) {
            <li>
              {{ t.name }} <button (click)="selectTeam(t)">Editar</button>
            </li>
          }
        </ul>
        @if (api.has("core.teams.manage")) {
          <form (ngSubmit)="saveTeam()">
            <label
              >Nombre<input
                name="team-name"
                [(ngModel)]="teamName"
                required /></label
            ><label
              >Miembros<select
                name="team-members"
                multiple
                [(ngModel)]="teamMemberIDs"
              >
                @for (u of users(); track u.id) {
                  <option [value]="u.id">{{ u.name }} · {{ u.email }}</option>
                }
              </select></label
            ><button [disabled]="busy()">Guardar equipo</button
            ><button type="button" (click)="clearTeam()">Nuevo</button>
          </form>
        }
      </section>
    }
    @if (api.has("core.settings.read")) {
      <section class="bs-frame">
        <h2 id="empresa">Empresa y SMTP</h2>
        <button (click)="loadSettings()">Actualizar</button>
        @if (api.has("core.settings.manage")) {
          <form (ngSubmit)="saveCompany()">
            <label
              >Nombre de empresa<input
                name="company-name"
                [(ngModel)]="companyName"
                required /></label
            ><label>RTN<input name="company-tax" [(ngModel)]="taxID" /></label
            ><label
              >Dirección<textarea
                name="company-address"
                [(ngModel)]="address"
              ></textarea></label
            ><button [disabled]="busy()">Guardar empresa</button>
          </form>
          <form (ngSubmit)="saveSMTP()">
            <label
              >Servidor autorizado<input
                name="smtp-host"
                [(ngModel)]="smtpHost"
                required /></label
            ><label
              >Puerto<select name="smtp-port" [(ngModel)]="smtpPort">
                <option [ngValue]="587">587 · STARTTLS</option>
                <option [ngValue]="465">465 · TLS</option>
              </select></label
            ><label
              >Usuario<input
                name="smtp-user"
                [(ngModel)]="smtpUsername"
                required /></label
            ><label
              >Remitente<input
                name="smtp-from"
                type="email"
                [(ngModel)]="smtpFrom"
                required /></label
            ><label
              >Contraseña SMTP (vacío conserva la actual)<input
                name="smtp-password"
                type="password"
                autocomplete="new-password"
                [(ngModel)]="smtpPassword" /></label
            ><button [disabled]="busy()">Guardar SMTP</button>
          </form>
        }
        @if (api.has("core.email.test")) {
          <button (click)="testEmail()" [disabled]="busy()">
            Enviar prueba a mi correo
          </button>
        }
      </section>
    }
    @if (api.has("core.settings.manage")) {
      <section class="bs-frame">
        <h2 id="servicios">Integraciones</h2>
        <p>
          Los límites del entorno siguen vigentes. Estas opciones permiten
          desactivar conexiones; no crean credenciales ni recursos.
        </p>
        <form (ngSubmit)="saveIntegrations()">
          <label
            ><input
              type="checkbox"
              name="integration-mcp"
              [(ngModel)]="integrationMCP"
            />
            MCP</label
          ><label
            ><input
              type="checkbox"
              name="integration-files"
              [(ngModel)]="integrationFiles"
            />
            Archivos S3</label
          ><label
            ><input
              type="checkbox"
              name="integration-email"
              [(ngModel)]="integrationEmail"
            />
            Correo</label
          ><button [disabled]="busy()">Guardar límites de integración</button>
        </form>
      </section>
    }
    @if (api.has("core.services.manage")) {
      <section class="bs-frame">
        <h2>Conexiones técnicas MCP</h2>
        <p>Acceso limitado a lectura; OAuth externo pendiente de validar.</p>
        <form (ngSubmit)="createService()">
          <label
            >Nombre<input
              name="service-name"
              [(ngModel)]="serviceName"
              required /></label
          ><label
            >Permisos (separados por coma)<input
              name="service-scopes"
              [(ngModel)]="serviceScopes" /></label
          ><label
            >Vence<input
              name="service-expiry"
              type="date"
              [(ngModel)]="serviceExpiry"
              required /></label
          ><button [disabled]="busy()">Crear conexión</button>
        </form>
        @if (serviceToken()) {
          <pre>{{ serviceToken() }}</pre>
          <button (click)="serviceToken.set('')">Ocultar token</button>
        }
        <button (click)="loadServices()">Actualizar conexiones</button>
        <ul>
          @for (c of services(); track c["id"]) {
            <li>
              {{ c["id"] }} · {{ c["name"] }} ·
              {{ c["active"] ? "Activa" : "Revocada" }}
              <button (click)="revokeService(c['id'])">Revocar</button>
            </li>
          }
        </ul>
      </section>
    }
    @if (api.has("core.audit.read")) {
      <section class="bs-frame">
        <h2 id="auditoria">Auditoría</h2>
        <button (click)="loadAudit()">Actualizar</button>
        <div class="bs-table-wrap">
          <table class="bs-table">
            <thead>
              <tr>
                <th>Fecha</th>
                <th>Actor</th>
                <th>Acción</th>
                <th>Resultado</th>
              </tr>
            </thead>
            <tbody>
              @for (e of audit(); track e["id"]) {
                <tr>
                  <td>{{ e["created_at"] }}</td>
                  <td>{{ e["actor_id"] }}</td>
                  <td>{{ e["action"] }}</td>
                  <td>{{ state(e["result"]) }}</td>
                </tr>
              }
            </tbody>
          </table>
        </div>
        <button (click)="auditPage = auditPage + 1; loadAudit()">
          Página siguiente
        </button>
      </section>
    }
    @if (api.has("core.jobs.read")) {
      <section class="bs-frame">
        <h2>Trabajos</h2>
        <button (click)="loadJobs()">Actualizar</button>
        <ul>
          @for (j of jobs(); track j["id"]) {
            <li>
              {{ j["kind"] }} · {{ state(j["status"]) }} ·
              {{ j["attempts"] }} intentos
              @if (j["status"] === "failed" && api.has("core.jobs.retry")) {
                <button (click)="retryJob(j['id'])">Reintentar</button>
              }
            </li>
          }
        </ul>
      </section>
    }
    @if (api.has("core.email.read")) {
      <section class="bs-frame">
        <h2>Estado del correo</h2>
        <button (click)="loadEmails()">Actualizar</button>
        <ul>
          @for (e of emails(); track e["id"]) {
            <li>
              {{ e["id"] }} · {{ e["recipient"] }} · {{ state(e["status"]) }}
            </li>
          }
        </ul>
        <form (ngSubmit)="reconcile()">
          <label
            >ID de correo incierto<input
              name="reconcile-id"
              [(ngModel)]="emailID"
              required /></label
          ><label
            >Resultado verificado<select
              name="reconcile-state"
              [(ngModel)]="emailStatus"
            >
              <option value="accepted">Aceptado por SMTP</option>
              <option value="failed">No enviado</option>
            </select></label
          ><label
            >Referencia de evidencia<input
              name="reconcile-reference"
              minlength="10"
              [(ngModel)]="emailReference"
              required /></label
          ><button [disabled]="busy()">Registrar conciliación</button>
        </form>
      </section>
    }
  `,
})
export class Settings {
  api = inject(Client);
  busy = signal(false);
  message = signal("");
  password = "";
  code = "";
  users = signal<User[]>([]);
  selectedUser: User | null = null;
  userRoleIDs: string[] = [];
  inviteEmail = "";
  inviteKind = "invite";
  invitation = signal("");
  mfaUser = "";
  mfaReference = "";
  roles = signal<Role[]>([]);
  permissions = signal<string[]>([]);
  roleID = "";
  roleName = "";
  roleVersion = "0";
  roleGrants = "";
  teams = signal<Team[]>([]);
  teamID = "";
  teamVersion = "0";
  teamName = "";
  teamMemberIDs: string[] = [];
  settings = signal<Setting[]>([]);
  companyVersion = "0";
  companyName = "";
  taxID = "";
  address = "";
  smtpVersion = "0";
  smtpHost = "";
  smtpPort = 587;
  smtpUsername = "";
  smtpFrom = "";
  smtpPassword = "";
  integrationVersion = "0";
  integrationMCP = false;
  integrationFiles = false;
  integrationEmail = false;
  serviceName = "";
  serviceScopes = "core.modules.read";
  serviceExpiry = "";
  serviceToken = signal("");
  services = signal<Record<string, string>[]>([]);
  audit = signal<Record<string, string>[]>([]);
  auditPage = 1;
  jobs = signal<Record<string, string>[]>([]);
  emails = signal<Record<string, string>[]>([]);
  emailID = "";
  emailStatus = "accepted";
  emailReference = "";
  constructor() {
    void this.initialize();
  }
  async initialize() {
    try {
      if (this.api.has("core.users.read"))
        this.users.set(
          (await this.api.get<{ data: User[] }>("/users?per_page=100")).data,
        );
      if (this.api.has("core.roles.read")) {
        const r = await this.api.get<{ data: Role[]; permissions: string[] }>(
          "/roles",
        );
        this.roles.set(r.data);
        this.permissions.set(r.permissions);
      }
    } catch (e) {
      this.message.set(this.api.message(e));
    }
  }
  async run(f: () => Promise<void>) {
    this.busy.set(true);
    this.message.set("");
    try {
      await f();
      this.message.set("Operación completada.");
    } catch (e) {
      this.message.set(this.api.message(e));
    } finally {
      this.busy.set(false);
    }
  }
  reauth() {
    return this.run(async () => {
      await this.api.write("POST", "/auth/reauth", {
        password: this.password,

      });
      this.password = "";
      this.code = "";
    });
  }
  loadUsers() {
    return this.run(async () =>
      this.users.set(
        (await this.api.get<{ data: User[] }>("/users?per_page=100")).data,
      ),
    );
  }
  selectUser(u: User) {
    this.selectedUser = { ...u };
    this.userRoleIDs = [...(u.roles || [])];
  }
  saveUser() {
    return this.run(async () => {
      const u = this.selectedUser!;
      await this.api.write("PATCH", "/users/" + u.id, {
        name: u.name,
        active: u.active,
        roles: this.userRoleIDs,
        version: u.version,
      });
      this.selectedUser = null;
      this.users.set(
        (await this.api.get<{ data: User[] }>("/users?per_page=100")).data,
      );
    });
  }
  revoke(id: string) {
    return this.run(async () => {
      await this.api.write("DELETE", "/users/" + id + "/sessions");
    });
  }
  invite() {
    return this.run(async () => {
      const r = await this.api.write<{ token: string }>(
        "POST",
        "/invitations",
        { email: this.inviteEmail, kind: this.inviteKind },
      );
      this.invitation.set(r.token);
    });
  }
  resetMFA() {
    return this.run(async () => {
      await this.api.write("POST", "/users/" + this.mfaUser + "/reset-mfa", {
        verification_reference: this.mfaReference,
      });
    });
  }
  loadRoles() {
    return this.run(async () => {
      const r = await this.api.get<{ data: Role[]; permissions: string[] }>(
        "/roles",
      );
      this.roles.set(r.data);
      this.permissions.set(r.permissions);
    });
  }
  selectRole(r: Role) {
    this.roleID = r.id;
    this.roleVersion = r.version;
    this.roleName = r.name;
    this.roleGrants = r.grants
      .map((g) => g.permission + ":" + g.scope)
      .join("\n");
  }
  clearRole() {
    this.roleID = "";
    this.roleVersion = "0";
    this.roleName = "";
    this.roleGrants = "";
  }
  saveRole() {
    return this.run(async () => {
      const grants = this.roleGrants
        .split("\n")
        .filter((x) => x.trim())
        .map((x) => {
          const [permission, scope] = x.trim().split(":");
          return { permission, scope };
        });
      await this.api.write(
        this.roleID ? "PATCH" : "POST",
        "/roles" + (this.roleID ? "/" + this.roleID : ""),
        { name: this.roleName, version: this.roleVersion, grants },
      );
      this.clearRole();
    });
  }
  loadTeams() {
    return this.run(async () =>
      this.teams.set((await this.api.get<{ data: Team[] }>("/teams")).data),
    );
  }
  selectTeam(t: Team) {
    this.teamID = t.id;
    this.teamName = t.name;
    this.teamVersion = t.version;
    this.teamMemberIDs = [...t.members];
  }
  clearTeam() {
    this.teamID = "";
    this.teamName = "";
    this.teamVersion = "0";
    this.teamMemberIDs = [];
  }
  saveTeam() {
    return this.run(async () => {
      await this.api.write(
        this.teamID ? "PATCH" : "POST",
        "/teams" + (this.teamID ? "/" + this.teamID : ""),
        {
          name: this.teamName,
          version: this.teamVersion,
          members: this.teamMemberIDs,
        },
      );
      this.clearTeam();
    });
  }
  loadSettings() {
    return this.run(async () => {
      const r = await this.api.get<{ data: Setting[] }>("/settings");
      this.settings.set(r.data);
      for (const s of r.data) {
        if (s.name === "integrations") {
          this.integrationVersion = s.version;
          this.integrationMCP = Boolean(s.value["mcp"]);
          this.integrationFiles = Boolean(s.value["files"]);
          this.integrationEmail = Boolean(s.value["email"]);
        }
        if (s.name === "company") {
          this.companyVersion = s.version;
          this.companyName = String(s.value["name"] || "");
          this.taxID = String(s.value["tax_id"] || "");
          this.address = String(s.value["address"] || "");
        }
        if (s.name === "smtp") {
          this.smtpVersion = s.version;
          this.smtpHost = String(s.value["host"] || "");
          this.smtpPort = Number(s.value["port"]);
          this.smtpUsername = String(s.value["username"] || "");
          this.smtpFrom = String(s.value["from"] || "");
        }
      }
    });
  }
  saveCompany() {
    return this.run(async () => {
      await this.api.write("PATCH", "/settings/company", {
        version: this.companyVersion,
        value: {
          name: this.companyName,
          tax_id: this.taxID,
          address: this.address,
        },
        secret: "",
      });
      this.companyVersion = String(BigInt(this.companyVersion) + 1n);
    });
  }
  saveSMTP() {
    return this.run(async () => {
      await this.api.write("PATCH", "/settings/smtp", {
        version: this.smtpVersion,
        value: {
          host: this.smtpHost,
          port: this.smtpPort,
          username: this.smtpUsername,
          from: this.smtpFrom,
          tls: this.smtpPort === 465 ? "implicit" : "starttls",
        },
        secret: this.smtpPassword,
      });
      this.smtpPassword = "";
      this.smtpVersion = String(BigInt(this.smtpVersion) + 1n);
    });
  }
  saveIntegrations() {
    return this.run(async () => {
      await this.api.write("PATCH", "/settings/integrations", {
        version: this.integrationVersion,
        value: {
          mcp: this.integrationMCP,
          files: this.integrationFiles,
          email: this.integrationEmail,
        },
        secret: "",
      });
      this.integrationVersion = String(BigInt(this.integrationVersion) + 1n);
    });
  }
  testEmail() {
    return this.run(async () => {
      await this.api.write("POST", "/emails/test", {}, crypto.randomUUID());
    });
  }
  createService() {
    return this.run(async () => {
      const r = await this.api.write<{ token: string }>("POST", "/services", {
        name: this.serviceName,
        scopes: this.csv(this.serviceScopes),
        expires_at: new Date(this.serviceExpiry + "T23:59:59Z").toISOString(),
      });
      this.serviceToken.set(r.token);
    });
  }
  loadServices() {
    return this.run(async () =>
      this.services.set(
        (await this.api.get<{ data: Record<string, string>[] }>("/services"))
          .data,
      ),
    );
  }
  revokeService(id: string) {
    return this.run(async () => {
      await this.api.write("DELETE", "/services/" + id);
    });
  }
  loadAudit() {
    return this.run(async () =>
      this.audit.set(
        (
          await this.api.get<{ data: Record<string, string>[] }>(
            "/audit?page=" + this.auditPage,
          )
        ).data,
      ),
    );
  }
  loadJobs() {
    return this.run(async () =>
      this.jobs.set(
        (await this.api.get<{ data: Record<string, string>[] }>("/jobs")).data,
      ),
    );
  }
  retryJob(id: string) {
    return this.run(async () => {
      await this.api.write("POST", "/jobs/" + id + "/retry");
    });
  }
  loadEmails() {
    return this.run(async () =>
      this.emails.set(
        (await this.api.get<{ data: Record<string, string>[] }>("/emails"))
          .data,
      ),
    );
  }
  reconcile() {
    return this.run(async () => {
      await this.api.write("POST", "/emails/" + this.emailID + "/reconcile", {
        status: this.emailStatus,
        evidence_reference: this.emailReference,
      });
    });
  }
  state(s: string) {
    return (
      (
        {
          pending: "Pendiente",
          running: "En ejecución",
          completed: "Completado",
          failed: "Fallido",
          uncertain: "Resultado incierto",
          sending: "Enviando",
          accepted: "Aceptado por SMTP",
          success: "Correcto",
          denied: "Denegado",
          partial: "MFA pendiente",
        } as Record<string, string>
      )[s] || s
    );
  }
  private csv(v: string) {
    return v
      .split(",")
      .map((x) => x.trim())
      .filter(Boolean);
  }
}
