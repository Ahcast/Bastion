# Backend Laravel

Referencia de alcance: [maestro vigente](../../docs/Proyecto_Bastion_Documento_Maestro.md). La nueva aplicación aún requiere implementación y validación.

## Estructura y dependencias
Core en `app/Core/`; módulos en `app/Modules/`. Ver [rutas](../../docs/architecture/core-modules.md) y [Core](../../docs/architecture/core.md). Core conserva API/MCP, identidad, MFA/RBAC, configuración SMTP y servicios comunes. Módulos conservan negocio y consumen contratos.

## Entorno objetivo
Laravel 13/PHP 8.5 propuestos; PHP CLI/extensiones y compatibilidad por validar. Percona/MySQL 5.7.44-48 reportado: no depender de SQL exclusivo de MySQL 8. Versiones exactas/lockfiles tras prueba en entorno objetivo.
Dominio bastion.castilloit.net; exponer solo public/activos. Angular se compila fuera del hosting.

## Contratos
REST JSON versionado y MCP son entradas a las mismas Actions/Policies. Sanctum/Fortify/TOTP para SPA; Passport/OAuth con PKCE para MCP según Core, con guards y scopes delimitados.
[Pipeline IA](../../docs/architecture/ai-integration.md): Work consulta pendientes mediante MCP; AI guarda tareas/resultados; no invoca Plus como API. Jobs database/cron acotados. Crawler/backups permanecen en Lightsail.
