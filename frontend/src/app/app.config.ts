import {
  ApplicationConfig,
  LOCALE_ID,
  inject,
  provideAppInitializer,
  provideBrowserGlobalErrorListeners,
} from "@angular/core";
import { provideRouter } from "@angular/router";
import { provideHttpClient, withInterceptors } from "@angular/common/http";
import { registerLocaleData } from "@angular/common";
import spanishHonduras from "@angular/common/locales/es-HN";
import { routes } from "./app.routes";
import { Client } from "./core/api/client";
import { apiInterceptor } from "./core/api/interceptor";
registerLocaleData(spanishHonduras);
export const appConfig: ApplicationConfig = {
  providers: [
    {provide:LOCALE_ID,useValue:"es-HN"},
    provideBrowserGlobalErrorListeners(),
    provideRouter(routes),
    provideHttpClient(withInterceptors([apiInterceptor])),
    provideAppInitializer(() => inject(Client).initialize()),
  ],
};
