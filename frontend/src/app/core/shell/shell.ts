import { Component, computed, inject, signal } from "@angular/core";
import {
  Router,
  RouterLink,
  RouterLinkActive,
  RouterOutlet,
} from "@angular/router";
import { FormsModule } from "@angular/forms";
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
  icon: string;
}

const NAV: NavItem[] = [
  { path: "/customers", label: "Clientes", permission: "clientes.view", module: "customers", icon: "clients" },
  { path: "/suppliers", label: "Proveedores", permission: "proveedores.view", module: "suppliers", icon: "suppliers" },
  { path: "/processes", label: "Procesos", permission: "procesos.view", module: "processes", icon: "tenders" },
  { path: "/quotes", label: "Cotizaciones", permission: "cotizaciones.view", module: "quotes", icon: "quotes" },
  { path: "/purchase-orders", label: "Órdenes de compra", permission: "ordenes.view", module: "purchase-orders", icon: "orders" },
  { path: "/invoices", label: "Facturas", permission: "facturas.view", module: "invoices", icon: "invoices" },
  { path: "/expenses", label: "Gastos", permission: "gastos.view", module: "expenses", icon: "expenses" },
  { path: "/reports", label: "Reportes", permission: "reportes.view", module: "reports", icon: "reports" },
  { path: "/collaboration", label: "Colaboración", permission: "colaboracion.view", module: "collaboration", icon: "team" },
];

@Component({
  imports: [RouterLink, RouterLinkActive, RouterOutlet, Icon, Logo, FormsModule],
  template: `
    <div class="bs-app">
      <header class="bs-topnav">
        <a
          class="bs-topnav-home"
          routerLink="/"
          routerLinkActive="bs-tab-active"
          [routerLinkActiveOptions]="{ exact: true }"
          aria-label="Inicio" ariaCurrentWhenActive="page"
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
              ><bs-icon [name]="item.icon" [size]="16" />{{ item.label }}</a
            >
          }
        </nav>

        <div class="bs-topnav-tools">
          <button type="button" class="bs-iconbtn" aria-label="Buscar módulo" [attr.aria-expanded]="searchOpen()" aria-controls="bs-module-search" (click)="searchOpen.set(!searchOpen())"><bs-icon name="search" /></button>
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
          <button type="button" class="bs-avatar bs-user-trigger" [title]="userName()" aria-label="Menú de usuario" [attr.aria-expanded]="userOpen()" aria-controls="bs-user-menu" (click)="userOpen.set(!userOpen())">{{
            initials()
          }}</button>
          @if(userOpen()){<nav id="bs-user-menu" class="bs-user-menu" aria-label="Cuenta de usuario"><a routerLink="/settings" fragment="personal" (click)="userOpen.set(false)">Mi cuenta</a><button type="button" (click)="logout()">Cerrar sesión</button></nav>}
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

      @if (searchOpen()) {
        <section id="bs-module-search" class="bs-searchbar" aria-label="Buscar módulo">
          <label class="bs-field">Buscar módulo<input class="bs-input" [(ngModel)]="moduleSearch" placeholder="Clientes, procesos, cotizaciones…" (keydown.escape)="searchOpen.set(false)" /></label>
          <nav class="bs-search-results" aria-label="Resultados de módulos">@for(item of searchItems(); track item.path){<a [routerLink]="item.path" (click)="searchOpen.set(false)">{{item.label}} →</a>} @empty {<p>No hay módulos disponibles con ese nombre.</p>}</nav>
        </section>
      }
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
  userOpen=signal(false);
  searchOpen = signal(false);
  moduleSearch = "";
  searchItems() {
    const query = this.moduleSearch.normalize("NFD").replace(/[\u0300-\u036f]/g, "").toLowerCase();
    return this.items().filter(item => item.label.normalize("NFD").replace(/[\u0300-\u036f]/g, "").toLowerCase().includes(query));
  }
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
