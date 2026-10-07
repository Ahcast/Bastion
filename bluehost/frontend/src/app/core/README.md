# Core frontend

Servicios singleton de sesión, cliente API, guards, interceptors y configuración. Incluye el shell de la aplicación y las pantallas administrativas del Core.

- [Inicio](pages/dashboard/README.md).
- [Configuración](pages/settings/README.md): empresa, usuarios, roles, permisos, MFA, sesiones, SMTP, integraciones y auditoría según permiso.
- [Shell](shell/README.md): navegación y estructura visual.

Los módulos viven en `../modules`; componentes reutilizables en `../shared`. Core no importa implementaciones de módulos. La composición de rutas se realiza en la raíz de la aplicación al inicializar Angular; los guards no sustituyen la autorización backend.
