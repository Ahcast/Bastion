import { Component, inject, signal } from "@angular/core";
import {
  RouterLink,
  RouterLinkActive,
  RouterOutlet,
  Router,
} from "@angular/router";
import { Client } from "../api/client";
@Component({
  imports: [RouterLink, RouterLinkActive, RouterOutlet],
  template: `<header>
      <a class="brand" routerLink="/">Bastion</a
      ><span>{{ api.profile()?.user?.name }}</span
      ><button (click)="logout()">Cerrar sesión</button>
    </header>
    <div class="layout">
      <nav aria-label="Menú principal">
        <a
          routerLink="/"
          routerLinkActive="active"
          [routerLinkActiveOptions]="{ exact: true }"
          >Inicio</a
        >
        @if(api.has("clientes.view") && api.moduleEnabled("customers")){<a routerLink="/customers" routerLinkActive="active">Clientes</a>}
        @if(api.has("proveedores.view") && api.moduleEnabled("suppliers")){<a routerLink="/suppliers" routerLinkActive="active">Proveedores</a>}
        @if(api.has("cotizaciones.view") && api.moduleEnabled("quotes")){<a routerLink="/quotes" routerLinkActive="active">Cotizaciones</a>}
        @if(api.has("ordenes.view") && api.moduleEnabled("purchase-orders")){<a routerLink="/purchase-orders" routerLinkActive="active">Órdenes de Compra</a>}
        @if(api.has("facturas.view") && api.moduleEnabled("invoices")){<a routerLink="/invoices" routerLinkActive="active">Facturas</a>}
        @if(api.has("procesos.view") && api.moduleEnabled("processes")){<a routerLink="/processes" routerLinkActive="active">Procesos</a>}
        <a routerLink="/settings" routerLinkActive="active">Configuración</a>
      </nav>
      <main>
        @if (api.has("core.notifications.read")) {
          <details>
            <summary>Notificaciones</summary>
            <button (click)="loadNotifications()">
              Actualizar notificaciones
            </button>
            <ul>
              @for (n of notifications(); track n["id"]) {
                <li>
                  {{ n["message"] }}
                  @if (!n["read_at"]) {
                    <button (click)="readNotification(n['id'])">
                      Marcar leída
                    </button>
                  }
                </li>
              } @empty {
                <li>No hay notificaciones cargadas.</li>
              }
            </ul>
          </details>
        }
        <p role="alert">{{ error() }}</p>
        <router-outlet />
      </main>
    </div>`,
})
export class Shell {
  api = inject(Client);
  private router = inject(Router);
  error = signal("");
  notifications = signal<Record<string, string>[]>([]);
  async loadNotifications() {
    try {
      this.notifications.set(
        (
          await this.api.get<{ data: Record<string, string>[] }>(
            "/notifications",
          )
        ).data,
      );
    } catch (e) {
      this.error.set(this.api.message(e));
    }
  }
  async readNotification(id: string) {
    try {
      await this.api.write("PATCH", "/notifications/" + id + "/read");
      await this.loadNotifications();
    } catch (e) {
      this.error.set(this.api.message(e));
    }
  }
  async logout() {
    try {
      await this.api.logout();
      void this.router.navigateByUrl("/login");
    } catch (e) {
      this.error.set(this.api.message(e));
    }
  }
}
