import { Component, inject, signal } from "@angular/core";
import { FormsModule } from "@angular/forms";
import { Client } from "../api/client";
import { RouterLink } from "@angular/router";
@Component({
  imports: [FormsModule, RouterLink],
  template: `<section class="auth card">
    <h1>Activar cuenta o restablecer contraseña</h1>
    <p>
      Solicite al administrador un código vigente. El restablecimiento de
      contraseña conserva MFA.
    </p>
    <form (ngSubmit)="submit()">
      <label
        >Código de invitación<input
          name="token"
          [(ngModel)]="token"
          required /></label
      ><label>Nombre (alta nueva)<input name="name" [(ngModel)]="name" /></label
      ><label
        >Nueva contraseña (15 caracteres mínimo)<input
          name="password"
          type="password"
          autocomplete="new-password"
          minlength="15"
          [(ngModel)]="password"
          required /></label
      ><button [disabled]="busy()">Guardar</button>
    </form>
    <p role="status">{{ message() }}</p>
    <a routerLink="/login">Iniciar sesión</a>
  </section>`,
})
export class Accept {
  api = inject(Client);
  token = "";
  name = "";
  password = "";
  message = signal("");
  busy = signal(false);
  async submit() {
    this.busy.set(true);
    try {
      await this.api.write("POST", "/auth/accept", {
        token: this.token,
        name: this.name,
        password: this.password,
      });
      this.token = "";
      this.password = "";
      this.message.set("Datos guardados. Inicie sesión y complete MFA.");
    } catch (e) {
      this.message.set(this.api.message(e));
    } finally {
      this.busy.set(false);
    }
  }
}
