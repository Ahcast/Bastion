import { inject } from "@angular/core";
import { CanActivateFn, Router } from "@angular/router";
import { Client } from "../api/client";
export const authenticated: CanActivateFn = () =>
  inject(Client).mfa() === "complete" ||
  inject(Router).createUrlTree(["/login"]);
export const permitted: CanActivateFn = (route) =>
  inject(Client).has(route.data["permission"]) ||
  inject(Router).createUrlTree(["/"]);
