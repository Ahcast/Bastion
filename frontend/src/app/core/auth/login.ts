import { Component, inject, signal } from "@angular/core";
import { FormsModule } from "@angular/forms";
import { Router } from "@angular/router";
import { Client } from "../api/client";
@Component({
  selector: "bastion-login",
  imports: [FormsModule],
  template: ` <section class="auth card">
    <h1>Bastion</h1>
    <p>Acceso a la plataforma de Castillo IT</p>
    @if (error()) {
      <p role="alert" class="error">{{ error() }}</p>
    }
    @if (api.mfa() === "anonymous") {
      <form (ngSubmit)="login()">
        <label
          >Correo<input
            name="email"
            type="email"
            autocomplete="username"
            [(ngModel)]="email"
            required /></label
        ><label
          >Contraseña<input
            name="password"
            type="password"
            autocomplete="current-password"
            [(ngModel)]="password"
            required /></label
        ><button [disabled]="busy()">Ingresar</button>
      </form>
    } @else if (api.mfa() !== "complete") {
      <h2>Verificación en dos pasos</h2>
      @if (api.mfa() === "enroll") {
        <p>
          Registre Bastion en su aplicación de autenticación. La clave se
          muestra únicamente para este enrolamiento.
        </p>
        <button type="button" (click)="enroll()" [disabled]="busy()">
          Generar clave TOTP
        </button>
        @if (uri()) {
          <label
            >URI para su autenticador<textarea
              readonly
              [value]="uri()"
            ></textarea>
          </label>
        }
      }
      <form (ngSubmit)="factor()">
        <label
          >Código de autenticación o recuperación<input
            name="code"
            autocomplete="one-time-code"
            [(ngModel)]="code"
            required /></label
        ><button [disabled]="busy()">Verificar</button>
      </form>
      <button class="secondary" (click)="logout()">Salir</button>
    } @else {
      @if (codes().length) {
        <h2>Guarde estos códigos de recuperación</h2>
        <p>
          Cada código puede utilizarse una sola vez. Esta lista no volverá a
          mostrarse.
        </p>
        <pre>{{
          codes().join(
            "
"
          )
        }}</pre>
      }
      <button (click)="continue()">Continuar</button>
    }
  </section>`,
})
export class Login {
  api = inject(Client);
  private router = inject(Router);
  email = "";
  password = "";
  code = "";
  busy = signal(false);
  error = signal("");
  uri = signal("");
  codes = signal<string[]>([]);
  async run(action: () => Promise<void>) {
    this.busy.set(true);
    this.error.set("");
    try {
      await action();
    } catch (e) {
      this.error.set(this.api.message(e));
    } finally {
      this.busy.set(false);
    }
  }
  login() {
    return this.run(async () => {
      await this.api.login(this.email, this.password);
      this.password = "";
    });
  }
  enroll() {
    return this.run(async () => {
      const r = await this.api.write<{ otpauth_uri: string }>(
        "POST",
        "/auth/mfa/enroll",
      );
      this.uri.set(r.otpauth_uri);
    });
  }
  factor() {
    return this.run(async () => {
      this.codes.set(
        await this.api.factor(this.code, this.api.mfa() === "enroll"),
      );
      this.code = "";
      this.uri.set("");
      if (!this.codes().length) this.continue();
    });
  }
  logout() {
    return this.run(async () => {
      await this.api.logout();
      this.uri.set("");
    });
  }
  continue() {
    this.codes.set([]);
    void this.router.navigateByUrl("/");
  }
}
