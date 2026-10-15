import { Component, inject, signal } from "@angular/core";
import { FormsModule } from "@angular/forms";
import { Client } from "../api/client";
@Component({
  selector: "bastion-reauthentication",
  imports: [FormsModule],
  template: `<section class="card">
    <h3>Confirmación de identidad</h3>
    <p>
      Confirme su contraseña antes de realizar una acción sensible. Su
      verificación OTP sigue vigente en este navegador y esta IP.
    </p>
    <form (ngSubmit)="confirm()">
      <label
        >Contraseña para confirmar<input
          type="password"
          name="confirmation-password"
          autocomplete="current-password"
          [(ngModel)]="password"
          required /></label
      ><button [disabled]="busy()">Confirmar identidad</button>
    </form>
    <p aria-live="polite">{{ message() }}</p>
  </section>`,
})
export class Reauthentication {
  api = inject(Client);
  password = "";
  busy = signal(false);
  message = signal("");
  async confirm() {
    if (this.busy()) return;
    this.busy.set(true);
    this.message.set("");
    try {
      await this.api.write("POST", "/auth/reauth", { password: this.password });
      this.message.set(
        "Identidad confirmada. Puede continuar con la operación.",
      );
    } catch (e) {
      this.message.set(this.api.message(e));
    } finally {
      this.password = "";
      this.busy.set(false);
    }
  }
}
