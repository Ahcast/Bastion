# Estado del proyecto

Actualizado el 9 de octubre de 2026. Core 0.1.0 publicado y validado en CI; Clientes 0.1.0 implementado localmente, pendiente de cierre de CI. Producción no desplegada.

| Área | Estado real |
| --- | --- |
| Backend Go | Core/API y comandos server/admin/worker implementados; builds reproducibles |
| Identidad | Usuarios/invitaciones, Argon2id, sesiones, CSRF entre orígenes, TOTP/recovery, reauth, revocación y denylist configurable |
| Autorización | Roles/permisos all/self/team, equipos, denegación por defecto y reevaluación en operaciones/jobs |
| Administración | Empresa/SMTP/integraciones tipadas/versionadas, auditoría, tokens técnicos y notificaciones |
| Persistencia | Migración Core y runner con lock/checksum/dirty; probado en MySQL Community 5.7.44, no en Percona Bluehost real |
| Ejecución | Idempotencia, outbox, leases/retries/jobs y conciliación SMTP implementados |
| Archivos | Metadatos/versiones, adapter AWS SDK, upload/checksum/cuarentena/descarga y registro de análisis; S3 real y scanner externo no validados |
| MCP | SDK oficial Streamable HTTP, bearer técnico limitado y conformance por SDK cliente; OAuth/Work pendientes |
| Frontend Angular | Core de sesión, shell, Inicio y Configuración implementados; UI española; build y tests |
| Crawler Python | Conservado sin cambios; 17 pruebas offline satisfactorias; receptor Processes aún pendiente |
| Clientes | Ficha/catálogo Go y Angular, contactos/direcciones/notas, historial, snapshots/resolución exacta, permisos y documentos implementados; CI por cerrar |
| Demás módulos comerciales | Pendientes; se implementan de uno en uno tras cerrar pruebas y CI del anterior |
| Despliegue | Templates systemd/nginx/Bluehost preparados; no instalados ni desplegados |
| Infraestructura real | MySQL TLS/túnel/hosting, DNS/HTTPS, S3/IAM, SMTP y memoria conjunta con crawler pendientes |
| Git | Solo main; limpieza reactiva vigente; prevención GitHub requiere plan Pro |

## Verificación del Core (unidad anterior)

- gofmt, go vet y build server/worker/admin satisfactorios.
- 16 tests Go con race detector, incluidos 11 casos de integración en MySQL 5.7 real. Doce solicitudes concurrentes ejecutan un único efecto; mismatch de payload, rollback, permisos/IDOR, leases, MFA/recovery reutilizados, revocación, cuarentena y correo incierto verificados.
- npm ci, build Angular y 4 pruebas del cliente/interceptor satisfactorios.
- Un flujo Playwright contra Angular y API reales en dos orígenes: login→enrolamiento TOTP→recovery→configuración persistida tras recarga→logout. Sin mocks HTTP ni envíos reales.
- 17 pruebas del crawler Python satisfactorias. Enlaces internos verificados.
- CI remoto satisfactorio para el commit [322a1d2](https://github.com/Ahcast/Bastion/commit/322a1d2b3fd16f6f7cb94b3226e56747e969f96a): [Core checks](https://github.com/Ahcast/Bastion/actions/runs/37988596602) pasó backend (gofmt, vet, race detector/MySQL 5.7 y builds), frontend (build y tests) y navegador contra API/Angular/MySQL reales. También pasaron [documentación](https://github.com/Ahcast/Bastion/actions/runs/37988596500) y [rama única](https://github.com/Ahcast/Bastion/actions/runs/37988596531).

## Alcance pendiente antes de producción

Acceso Bluehost cifrado/restringido, soporte/parches Percona, cuentas DB de mínimo privilegio, backup/restore, clave de cifrado/custodia, corpus curado de contraseñas comprometidas, DNS/HTTPS/CORS/CSP, bucket privado/cifrado/CORS S3/IAM, ejecutor de análisis de archivos y evidencia, SMTP/IMAP autorizado, OAuth delegado Work, medición 2 GB/crawler/jobs y protección perimetral real. Los adapters compilados y sus tests no acreditan esos servicios.

No habilitar integraciones productivas ni migrar BD real mediante tests. No existen aún receptor crawler, tools/pipeline AI, workflows comerciales ni documentos de ventas. El Core admite archivos vinculados a Clientes mediante su política y rechaza entidades/módulos no registrados. La vista 360° de Clientes no inventa procesos/cotizaciones/órdenes/facturas/pagos pendientes.

## Unidad 1 de módulos — Clientes

Código: backend/internal/modules/customers, internal/composition, migración 0002_customers.sql, rutas/servicios Core de composición, Angular src/app/modules/customers, contrato/OpenAPI y runbook de Clientes. Cambios en Core limitados a catálogo asignable de permisos, actor MCP, política compartida de archivos y eventos durables; sin imports Core→negocio ni permisos automáticos a roles técnicos. Crawler conservado byte por byte.

- 27 pruebas Go con race detector satisfactorias: las 16 anteriores y 11 nuevas de Clientes (3 unitarias/DTO y 8 de integración MySQL 5.7).
- Ocho actores concurrentes intentan crear nombres/RTN equivalentes: un alta y siete conflictos, sin fusión. Doce solicitudes con la misma clave producen un único cliente/contacto/dirección/evento. Fallo SQL de auditoría revierte toda la operación y permite retry.
- Versiones, revocación inmediata, scopes all/self/team, IDOR, reautenticación, filtros literales/paginación, notas/historial, snapshot independiente e inactividad verificados. IDs superiores a 2^53 conservan precisión REST/MCP. Archivos requieren ambas políticas y cuarentena; no prueba S3 real.
- SDK oficial MCP verifica search_customers/get_customer, identidad contextual y revocación; no habilita OAuth Work ni escritura de agentes.
- npm ci, build y 8 pruebas Angular satisfactorios (4 Core y 4 Clientes). Ficha mantiene edición y clave ante fallo, no envía campos protegidos, respeta lector y paginación del backend. UI usa locale es-HN.
- Dos flujos de navegador contra MySQL/API/Angular reales satisfactorios: Core y Clientes. Clientes verifica alta/contacto/nota/edición persistida/duplicado/inactivación y filtros.
- 17 pruebas crawler satisfactorias; gofmt, vet, builds y documentación verificados. CI remoto pendiente de publicación; no comenzar Proveedores hasta su resultado satisfactorio.

## Continuidad

Orden: Clientes → Proveedores → Procesos/receptor crawler → Cotizaciones → OC de clientes → Facturas/pagos → Gastos → Reportes → Colaboración; Workflow/AI según dependencias. Proveedores no iniciado. Cerrar CI de Clientes y registrar evidencia antes de pasar al siguiente. Mantener all/self/team, snapshots, transacciones y permisos mínimos en cada unidad. No se han desplegado servicios ni migrado datos productivos.
