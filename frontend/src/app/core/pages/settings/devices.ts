import { Component, inject, signal } from "@angular/core";
import { DatePipe } from "@angular/common";
import { Router } from "@angular/router";
import { Client } from "../../api/client";
import { Reauthentication } from "../../security/reauthentication";
export interface TrustedDevice {
  id: string;
  name: string;
  created_at: string;
  last_seen_at: string;
  expires_at: string;
  current: boolean;
}
@Component({
  selector: "bastion-devices",
  imports: [DatePipe, Reauthentication],
  template: ` <section aria-labelledby="recognized-devices-title">
    <h3 id="recognized-devices-title">Equipos reconocidos</h3>
    <p>
      Cerrar una pestaña o reiniciar la PC conserva la verificación en este
      navegador. Cerrar sesión manualmente, cambiar de IP o vencer los 30 días
      requiere OTP. Cada navegador se registra por separado.
    </p>
    <button type="button" (click)="load(1)" [disabled]="busy()">
      Consultar equipos reconocidos
    </button>
    <p role="status">{{ message() }}</p>
    @if (loaded()) {
      @if (devices().length) {
        <div class="bs-table-wrap">
          <table class="bs-table">
            <thead>
              <tr>
                <th>Equipo</th>
                <th>Último uso</th>
                <th>Verificado hasta</th>
                <th>Acción</th>
              </tr>
            </thead>
            <tbody>
              @for (device of devices(); track device.id) {
                <tr>
                  <td>
                    {{ device.name }} <small>{{ device.id.slice(0, 8) }}</small>
                    @if (device.current) {
                      <strong> · Este navegador</strong>
                    }
                  </td>
                  <td>{{ device.last_seen_at | date: "dd/MM/yyyy HH:mm" }}</td>
                  <td>{{ device.expires_at | date: "dd/MM/yyyy HH:mm" }}</td>
                  <td>
                    <button
                      type="button"
                      class="secondary"
                      [disabled]="busy()"
                      [attr.aria-label]="
                        'Revocar ' + device.name + ' ' + device.id.slice(0, 8)
                      "
                      (click)="revoke(device)"
                    >
                      {{
                        device.current ? "Revocar y salir" : "Revocar equipo"
                      }}
                    </button>
                  </td>
                </tr>
              }
            </tbody>
          </table>
        </div>
        <p>
          Confirme su contraseña antes de revocar un equipo. Sus sesiones
          también se cerrarán.
        </p>
        <bastion-reauthentication />
      } @else {
        <p>No hay equipos reconocidos vigentes.</p>
      }
      <p>{{ total() }} equipos · Página {{ page() }}</p>
      <button
        type="button"
        class="secondary"
        (click)="load(page() - 1)"
        [disabled]="busy() || page() === 1"
      >
        Equipos anteriores
      </button>
      <button
        type="button"
        class="secondary"
        (click)="load(page() + 1)"
        [disabled]="busy() || page() * 25 >= total()"
      >
        Equipos siguientes
      </button>
    }
  </section>`,
})
export class Devices {
  private api = inject(Client);
  private router = inject(Router);
  devices = signal<TrustedDevice[]>([]);
  busy = signal(false);
  loaded = signal(false);
  message = signal("");
  total = signal(0);
  page = signal(1);
  async load(page = 1) {
    if (this.busy() || page < 1) return;
    this.busy.set(true);
    this.message.set("");
    try {
      const r = await this.api.get<{ data: TrustedDevice[]; total: number }>(
        `/auth/devices?page=${page}&per_page=25`,
      );
      this.devices.set(r.data);
      this.total.set(r.total);
      this.page.set(page);
      this.loaded.set(true);
    } catch (e) {
      this.message.set(this.api.message(e));
    } finally {
      this.busy.set(false);
    }
  }
  async revoke(device: TrustedDevice) {
    if (this.busy()) return;
    this.busy.set(true);
    this.message.set("");
    try {
      const result = await this.api.write<{ current: boolean }>(
        "DELETE",
        `/auth/devices/${device.id}`,
      );
      if (result.current) {
        this.devices.set([]);
        this.api.profile.set(null);
        this.api.mfa.set("anonymous");
        this.api.csrf = "";
        await this.router.navigateByUrl("/login");
      } else {
        this.busy.set(false);
        await this.load(
          this.devices().length === 1 && this.page() > 1
            ? this.page() - 1
            : this.page(),
        );
        this.message.set("Equipo revocado. El próximo acceso requerirá OTP.");
      }
    } catch (e) {
      this.message.set(this.api.message(e));
    } finally {
      this.busy.set(false);
    }
  }
}
