import { Component, inject, signal } from "@angular/core";
import { FormsModule } from "@angular/forms";
import { Client } from "../../api/client";
@Component({
  selector: "bastion-personal",
  imports: [FormsModule],
  template: `<section class="card">
    <h2>Mi cuenta</h2>
    <p role="status">{{ message() }}</p>
    <h3>Cambiar contraseña</h3>
    <p>
      Confirme primero su identidad. El cambio cerrará todas sus sesiones y
      conservará MFA.
    </p>
    <form (ngSubmit)="passwordChange()">
      <label
        >Nueva contraseña<input
          name="new-password"
          type="password"
          autocomplete="new-password"
          minlength="15"
          [(ngModel)]="password"
          required /></label
      ><button [disabled]="busy()">Cambiar y cerrar sesiones</button>
    </form>
    <h3>Recuperación</h3>
    <button (click)="recovery()" [disabled]="busy()">
      Generar nuevos códigos de recuperación
    </button>
    @if (codes().length) {
      <p>
        Los anteriores quedan invalidados. Guarde esta lista; no volverá a
        mostrarse.
      </p>
      <pre>{{
        codes().join(
          "
"
        )
      }}</pre>
      <button (click)="codes.set([])">Ocultar</button>
    }
    <h3>Preferencias</h3>
    <form (ngSubmit)="preferences()">
      <label
        >Tamaño de listas<select name="page-size" [(ngModel)]="pageSize">
          <option [ngValue]="25">25</option>
          <option [ngValue]="50">50</option>
          <option [ngValue]="100">100</option>
        </select></label
      ><label
        >Apariencia<select name="theme" [(ngModel)]="theme">
          <option value="system">Sistema</option>
          <option value="light">Clara</option>
          <option value="dark">Oscura</option>
        </select></label
      ><button [disabled]="busy()">Guardar preferencias</button>
    </form>
  </section>`,
})
export class Personal {
  api = inject(Client);
  password = "";
  theme = "system";
  pageSize = 25;
  message = signal("");
  busy = signal(false);
  codes = signal<string[]>([]);
  constructor() {
    void this.load();
  }
  async load() {
    try {
      const p = await this.api.get<{ theme: string; page_size: number }>(
        "/auth/preferences",
      );
      this.theme = p.theme;
      this.pageSize = p.page_size;
      this.applyTheme();
    } catch (e) {
      this.message.set(this.api.message(e));
    }
  }
  async run(f: () => Promise<void>) {
    this.busy.set(true);
    try {
      await f();
      this.message.set("Operación completada.");
    } catch (e) {
      this.message.set(this.api.message(e));
    } finally {
      this.busy.set(false);
    }
  }
  passwordChange() {
    return this.run(async () => {
      await this.api.write("POST", "/auth/password", {
        password: this.password,
      });
      this.password = "";
      this.api.profile.set(null);
      this.api.mfa.set("anonymous");
      location.assign("/login");
    });
  }
  recovery() {
    return this.run(async () => {
      const r = await this.api.write<{ recovery_codes: string[] }>(
        "POST",
        "/auth/recovery-codes",
      );
      this.codes.set(r.recovery_codes);
    });
  }
  preferences() {
    return this.run(async () => {
      await this.api.write("PATCH", "/auth/preferences", {
        theme: this.theme,
        page_size: this.pageSize,
      });
      this.applyTheme();
    });
  }
  applyTheme() {
    document.documentElement.dataset["theme"] = this.theme;
  }
}
