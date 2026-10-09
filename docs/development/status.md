# Estado del proyecto

Actualizado el 9 de octubre de 2026. Core 0.1.0 implementado localmente; producción no desplegada.

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
| Módulos comerciales | Pendientes; su existencia documental no equivale a código implementado |
| Despliegue | Templates systemd/nginx/Bluehost preparados; no instalados ni desplegados |
| Infraestructura real | MySQL TLS/túnel/hosting, DNS/HTTPS, S3/IAM, SMTP y memoria conjunta con crawler pendientes |
| Git | Solo main; limpieza reactiva vigente; prevención GitHub requiere plan Pro |

## Verificación local de esta unidad

- gofmt, go vet y build server/worker/admin satisfactorios.
- 16 tests Go con race detector, incluidos 11 casos de integración en MySQL 5.7 real. Doce solicitudes concurrentes ejecutan un único efecto; mismatch de payload, rollback, permisos/IDOR, leases, MFA/recovery reutilizados, revocación, cuarentena y correo incierto verificados.
- npm ci, build Angular y 4 pruebas del cliente/interceptor satisfactorios.
- Un flujo Playwright contra Angular y API reales en dos orígenes: login→enrolamiento TOTP→recovery→configuración persistida tras recarga→logout. Sin mocks HTTP ni envíos reales.
- 17 pruebas del crawler Python satisfactorias. Enlaces internos verificados.
- CI Core configurado para MySQL 5.7, Go/Angular y navegador; resultado remoto se comprueba tras publicación.

## Alcance pendiente antes de producción

Acceso Bluehost cifrado/restringido, soporte/parches Percona, cuentas DB de mínimo privilegio, backup/restore, clave de cifrado/custodia, corpus curado de contraseñas comprometidas, DNS/HTTPS/CORS/CSP, bucket privado/cifrado/CORS S3/IAM, ejecutor de análisis de archivos y evidencia, SMTP/IMAP autorizado, OAuth delegado Work, medición 2 GB/crawler/jobs y protección perimetral real. Los adapters compilados y sus tests no acreditan esos servicios.

No habilitar integraciones productivas ni migrar BD real mediante tests. No existen aún receptor crawler, tools/pipeline AI, workflows comerciales ni módulos de ventas. El Core rechaza accesos a archivos vinculados a entidades cuya política de módulo no se haya implementado.

## Continuidad

Siguiente unidad: datos base/Procesos y receptor crawler según plan. Registrar fase, archivos/migraciones/contratos, comandos y resultados, bloqueos y próximo paso. Estados: pendiente / en curso / implementado localmente / integrado en entorno validado / desplegado autorizado.
