import { Component, computed, inject, signal } from "@angular/core";
import {
  Router,
  RouterLink,
  RouterLinkActive,
  RouterOutlet,
} from "@angular/router";
import { Client } from "../api/client";
import { Icon } from "../ui/icon";
import { Logo } from "../ui/logo";

/**
 * Shell de Bastion · Concreto.
 * Barra superior de carbón (56px): el logo es Inicio, nueve pestañas de
 * texto en el orden aprobado y, a la derecha, alertas, Configuración
 * (engranaje), usuario y cerrar sesión. Sin menú lateral.
 * Mantiene los permisos y módulos del shell original.
 */
interface NavItem {
  path: string;
  label: string;
  permission: string;
  module: string;
}

const NAV: NavItem[] = [
  { path: "/customers", label: "Clientes", permission: "clientes.view", module: "customers" },
  { path: "/suppliers", label: "Proveedores", permission: "proveedores.view", module: "suppliers" },
  { path: "/processes", label: "Procesos", permission: "procesos.view", module: "processes" },
  { path: "/quotes", label: "Cotizaciones", permission: "cotizaciones.view", module: "quotes" },
  // Etiqueta conservada con mayúscula por las pruebas e2e (purchase-orders.spec.ts).
  { path: "/purchase-orders", label: "Órdenes de Compra", permission: "ordenes.view", module: "purchase-orders" },
  { path: "/invoices", label: "Facturas", permission: "facturas.view", module: "invoices" },
  { path: "/expenses", label: "Gastos", permission: "gastos.view", module: "expenses" },
  { path: "/reports", label: "Reportes", permission: "reportes.view", module: "reports" },
  { path: "/collaboration", label: "Colaboración", permission: "colaboracion.view", module: "collaboration" },
];

@Component({
  imports: [RouterLink, RouterLinkActive, RouterOutlet, Icon, Logo],
  template: `
    <div class="bs-app">
      <header class="bs-topnav">
        <a
          class="bs-topnav-home"
          routerLink="/"
          routerLinkActive="bs-tab-active"
          [routerLinkActiveOptions]="{ exact: true }"
          aria-label="Inicio"
          title="Inicio"
          ><bs-logo [size]="30"
        /></a>

        <nav class="bs-topnav-items" aria-label="Menú principal">
          @for (item of items(); track item.path) {
            <a
              class="bs-tab"
              [routerLink]="item.path"
              routerLinkActive="bs-tab-active"
              ariaCurrentWhenActive="page"
              >{{ item.label }}</a
            >
          }
        </nav>

        <div class="bs-topnav-tools">
          @if (api.has("core.notifications.read")) {
            <button
              type="button"
              class="bs-iconbtn"
              aria-label="Notificaciones"
              title="Notificaciones"
              [attr.aria-expanded]="alertsOpen()"
              aria-controls="bs-alerts"
              (click)="toggleAlerts()"
            >
              <bs-icon name="bell" />
              @if (unread() > 0) {
                <span class="bs-dot" aria-hidden="true"></span>
              }
            </button>
          }
          <a
            class="bs-tab bs-tab-icon"
            routerLink="/settings"
            routerLinkActive="bs-tab-active"
            ariaCurrentWhenActive="page"
            aria-label="Configuración"
            title="Configuración"
            ><bs-icon name="settings"
          /></a>
          <span class="bs-avatar" [title]="userName()" aria-hidden="true">{{
            initials()
          }}</span>
          <button
            type="button"
            class="bs-iconbtn"
            aria-label="Cerrar sesión"
            title="Cerrar sesión"
            (click)="logout()"
          >
            <bs-icon name="logout" />
          </button>
        </div>
      </header>

      @if (alertsOpen()) {
        <section id="bs-alerts" class="bs-searchbar" aria-label="Notificaciones">
          <div class="bs-frame">
            <div class="bs-frame-head">
              <h2 class="bs-frame-title">Notificaciones</h2>
              <button type="button" class="bs-btn bs-btn-ghost bs-btn-sm" (click)="loadNotifications()">
                Actualizar notificaciones
              </button>
            </div>
            <div class="bs-list">
              @for (n of notifications(); track n["id"]) {
                <div class="bs-item" [class.bs-item-alert]="!n['read_at']">
                  <span class="bs-item-t">{{ n["message"] }}</span>
                  @if (!n["read_at"]) {
                    <button type="button" class="bs-btn bs-btn-ghost bs-btn-sm" (click)="readNotification(n['id'])">
                      Marcar leída
                    </button>
                  }
                </div>
              } @empty {
                <p class="bs-muted">No hay notificaciones cargadas.</p>
              }
            </div>
          </div>
        </section>
      }

      <main class="bs-page">
        @if (error()) {
          <div class="bs-band bs-band-alert" role="alert">
            <strong>{{ error() }}</strong>
          </div>
        }
        <router-outlet />
      </main>
    </div>
  `,
})
export class Shell {
  api = inject(Client);
  private router = inject(Router);
  error = signal("");
  notifications = signal<Record<string, string>[]>([]);
  alertsOpen = signal(false);

  items = computed(() =>
    NAV.filter((i) => this.api.has(i.permission) && this.api.moduleEnabled(i.module)),
  );
  userName = computed(() => this.api.profile()?.user?.name ?? "");
  initials = computed(() =>
    this.userName()
      .split(/\s+/)
      .filter(Boolean)
      .slice(0, 2)
      .map((p) => p[0]!.toUpperCase())
      .join(""),
  );
  unread = computed(() => this.notifications().filter((n) => !n["read_at"]).length);

  toggleAlerts() {
    const open = !this.alertsOpen();
    this.alertsOpen.set(open);
    if (open) void this.loadNotifications();
  }
  async loadNotifications() {
    try {
      this.notifications.set(
        (await this.api.get<{ data: Record<string, string>[] }>("/notifications")).data,
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
