import { Component, HostListener, inject } from "@angular/core";
import { RouterOutlet } from "@angular/router";
import { Client } from "./core/api/client";
@Component({
  selector: "app-root",
  imports: [RouterOutlet],
  template: `@if (api.startupError()) {
      <section class="auth card">
        <h1>Bastion</h1>
        <p role="alert">{{ api.startupError() }}</p>
        <button (click)="retry()">Reintentar conexión</button>
      </section>
    } @else {
      <router-outlet />
    }`,
})
export class App {
  api = inject(Client);
  @HostListener("document:keydown")
  @HostListener("document:pointerdown")
  activity() {
    this.api.humanActivity = true;
  }
  retry() {
    location.reload();
  }
}
