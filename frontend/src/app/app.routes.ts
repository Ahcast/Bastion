import { Routes } from "@angular/router";
import { authenticated, permitted } from "./core/auth/guards";
export const routes: Routes = [
  {
    path: "login",
    loadComponent: () => import("./core/auth/login").then((m) => m.Login),
  },
  {
    path: "accept",
    loadComponent: () => import("./core/auth/accept").then((m) => m.Accept),
  },
  {
    path: "",
    canActivate: [authenticated],
    loadComponent: () => import("./core/shell/shell").then((m) => m.Shell),
    children: [
      { path:"customers", canActivate:[permitted],data:{permission:"clientes.view",module:"customers"},loadComponent:()=>import("./modules/customers/customers").then(m=>m.Customers) },
      {
        path: "",
        loadComponent: () =>
          import("./core/pages/dashboard/dashboard").then((m) => m.Dashboard),
      },
      {
        path: "settings",
        loadComponent: () =>
          import("./core/pages/settings/settings").then((m) => m.Settings),
      },
    ],
  },
  { path: "**", redirectTo: "" },
];
