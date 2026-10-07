# Estructura de Core y módulos

Fecha: 7 de octubre de 2026. Cambio estructural solicitado; no representa software desplegado.

## Organización vigente

| Ruta | Responsabilidad |
| --- | --- |
| `bluehost/backend/app/Core` | Identidad, acceso, configuración, SMTP, API, MCP y capacidades comunes. |
| `bluehost/backend/app/Modules` | Customers, Suppliers, Processes, Quotes, PurchaseOrders, Invoices, Expenses, Reports, Collaboration, Workflow y AI. |
| `bluehost/frontend/src/app/core` | Servicios comunes, shell y pantallas de Inicio/Configuración. |
| `bluehost/frontend/src/app/modules` | Pantallas y lógica de presentación por módulo. |
| `bluehost/frontend/src/app/shared` | Componentes y utilidades reutilizables sin lógica de negocio. |
| `aws` | Crawler, workers técnicos y documentación de S3. |

## Organización consolidada en el maestro v0.6

- Core vive en `bluehost/backend/app/Core`, separado de módulos.
- El frontend organiza negocio en `modules` y pantallas comunes en `core/pages`.
- `dashboard` y `settings` pasan a `core/pages`; los demás features pasan a `modules`.
- No se agregan módulos de negocio ni se cambia el menú aprobado.
- Esta página detalla las mismas rutas del maestro vigente; estructuras anteriores se consultan en Git.

## Dependencias

Core no importa módulos concretos. Los módulos dependen de contratos de Core. Las integraciones entre módulos pasan por servicios públicos o eventos, con dependencias declaradas. REST y MCP son adaptadores hacia las mismas operaciones. Workflow conserva aprobaciones y AI conserva orquestación de modelos; no se fusionan dentro de Core.

Frontend: la composición raíz registra módulos y sus rutas; módulos consumen Core/shared, y shared no depende de Core ni de módulos. El dashboard obtiene agregados autorizados mediante contratos, sin importar implementaciones privadas.

## Infraestructura confirmada por el usuario

- Dominio: `https://bastion.castilloit.net`.
- PHP seleccionable entre 8.1 y 8.5; objetivo propuesto 8.5, pendiente CLI/extensiones.
- SSH y FTP disponibles; despliegue previsto con SSH/SFTP.
- Cron con frecuencia mínima de un minuto.
- Document root actual: `~/public_html/website_091cdca8`.
- Base de datos: Percona Server for MySQL 5.7.44-48. Consultas y migraciones deberán probarse contra MySQL 5.7; soporte/parches por confirmar con Bluehost.

Solo activos públicos se despliegan en document root. El backend privado, secretos y logs deben quedar fuera de `public_html`.

## Tecnologías propuestas en la revisión

Laravel 13, PHP 8.5, Sanctum/Fortify, Spatie Permission y Policies; REST JSON/OpenAPI 3.1 con Scramble; laravel/mcp y Passport para delegación OAuth; colas database y cron. Versiones exactas, extensiones, protocolo MCP y dependencias se validarán antes de instalar. No se introducen dependencias ejecutables en este cambio.

## Seguridad común

Denegación por defecto, MFA obligatorio, Policies por registro, autorización de conexiones MCP y herramientas, auditoría, idempotencia y secretos protegidos. Configuración SMTP, usuarios, RBAC, sesiones, API y MCP pertenecen al Core. Las herramientas no pueden saltarse aprobaciones ni otorgarse permisos mediante contenido externo.

Los valores de sesión, límites y proveedor WAF discutidos son propuestas pendientes de configuración y prueba, no controles ya implementados.

Ver [contrato modular](../contracts/modules.md).

## Lineamientos consolidados

Ver [core.md](core.md) para el alcance completo del Core, infraestructura confirmada, tecnologías API/MCP, seguridad, operación y decisiones pendientes de esta sesión.
