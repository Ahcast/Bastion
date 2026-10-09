import { HttpInterceptorFn } from "@angular/common/http";
import { inject } from "@angular/core";
import { Client } from "./client";
import { catchError, throwError } from "rxjs";
import { Router } from "@angular/router";
export const apiInterceptor: HttpInterceptorFn = (req, next) => {
  const api = inject(Client);
  const router = inject(Router);
  if (!api.apiUrl || !req.url.startsWith(api.apiUrl + "/api/v1/"))
    return next(req);
  const headers: Record<string, string> = {};
  if (!["GET", "HEAD", "OPTIONS"].includes(req.method))
    headers["X-CSRF-Token"] = api.csrf;
  if (api.humanActivity) {
    headers["X-Bastion-Activity"] = "true";
    api.humanActivity = false;
  }
  return next(req.clone({ withCredentials: true, setHeaders: headers })).pipe(
    catchError((error) => {
      if (error.status === 401 && api.profile()) {
        api.profile.set(null);
        api.mfa.set("anonymous");
        void router.navigateByUrl("/login");
      }
      return throwError(() => error);
    }),
  );
};
