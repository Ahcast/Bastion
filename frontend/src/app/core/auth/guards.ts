import { inject } from "@angular/core";
import { CanActivateFn, Router } from "@angular/router";
import { Client } from "../api/client";
export const authenticated: CanActivateFn = () =>
  inject(Client).mfa() === "complete" ||
  inject(Router).createUrlTree(["/login"]);
export const permitted: CanActivateFn = (route) =>
  (inject(Client).has(route.data["permission"]) && (!route.data["module"] || inject(Client).moduleEnabled(route.data["module"]))) ||
  inject(Router).createUrlTree(["/"]);
