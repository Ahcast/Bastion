# Estado del proyecto

Actualizado el 9 de octubre de 2026. Core 0.1.0 publicado y validado en CI; Clientes 0.1.0 publicado y validado en pruebas locales y CI; unidad 1 cerrada. Proveedores 0.1.0 publicado y validado en pruebas locales y CI; unidad 2 cerrada. Producción no desplegada.

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
| Crawler Python | Conservado sin cambios; 17 pruebas offline satisfactorias; receptor Processes implementado; CI pendiente |
| Clientes | Ficha/catálogo Go y Angular, contactos/direcciones/notas, historial, snapshots/resolución exacta, permisos y documentos implementados; pruebas locales y CI aprobados |
| Proveedores | Catálogo, contactos/etiquetas/productos, condiciones/evaluación manual, evidencia de precios, historial/notas/documentos y REST/MCP implementados; pruebas locales y CI aprobados |
| Procesos | Ingestión transaccional, observaciones/versiones, consultas, marcas/notas, CSV y REST/MCP/Angular implementados; pruebas y CI en cierre |
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

No habilitar integraciones productivas ni migrar BD real mediante tests. Receptor crawler implementado; tools/pipeline AI, workflows comerciales y documentos de ventas pendientes. El Core admite archivos vinculados a Clientes y Proveedores mediante sus políticas y rechaza entidades/módulos no registrados. La vista 360° de Clientes no inventa procesos/cotizaciones/órdenes/facturas/pagos pendientes.

## Unidad 1 de módulos — Clientes

Código: backend/internal/modules/customers, internal/composition, migración 0002_customers.sql, rutas/servicios Core de composición, Angular src/app/modules/customers, contrato/OpenAPI y runbook de Clientes. Cambios en Core limitados a catálogo asignable de permisos, actor MCP, política compartida de archivos y eventos durables; sin imports Core→negocio ni permisos automáticos a roles técnicos. Crawler conservado byte por byte.

- 27 pruebas Go con race detector satisfactorias: las 16 anteriores y 11 nuevas de Clientes (3 unitarias/DTO y 8 de integración MySQL 5.7).
- Ocho actores concurrentes intentan crear nombres/RTN equivalentes: un alta y siete conflictos, sin fusión. Doce solicitudes con la misma clave producen un único cliente/contacto/dirección/evento. Fallo SQL de auditoría revierte toda la operación y permite retry.
- Versiones, revocación inmediata, scopes all/self/team, IDOR, reautenticación, filtros literales/paginación, notas/historial, snapshot independiente e inactividad verificados. IDs superiores a 2^53 conservan precisión REST/MCP. Archivos requieren ambas políticas y cuarentena; no prueba S3 real.
- SDK oficial MCP verifica search_customers/get_customer, identidad contextual y revocación; no habilita OAuth Work ni escritura de agentes.
- npm ci, build y 8 pruebas Angular satisfactorios (4 Core y 4 Clientes). Ficha mantiene edición y clave ante fallo, no envía campos protegidos, respeta lector y paginación del backend. UI usa locale es-HN.
- Dos flujos de navegador contra MySQL/API/Angular reales satisfactorios: Core y Clientes. Clientes verifica alta/contacto/nota/edición persistida/duplicado/inactivación y filtros.
- 17 pruebas crawler satisfactorias; gofmt, vet, builds y documentación verificados. Código final [ee296bd](https://github.com/Ahcast/Bastion/commit/ee296bd8a433701c028f625d45f7ba919d8a3950): [Core checks 37994052282](https://github.com/Ahcast/Bastion/actions/runs/37994052282) pasó backend con race detector/MySQL 5.7/builds, Angular y navegador real. También pasaron [documentación](https://github.com/Ahcast/Bastion/actions/runs/37994052191) y [rama única](https://github.com/Ahcast/Bastion/actions/runs/37994052247). Incluye DTO de resumen separado de ficha, conexión de lectura team/revocación y rechazo de create self hacia equipos ajenos. La incidencia inicial de descarga del contenedor y su solución están registradas en decisions.md. Todas las pruebas de aceptación de esta unidad cerradas; conexiones productivas externas mantienen los límites indicados arriba.

## Unidad 2 de módulos — Proveedores

Código: backend/internal/modules/suppliers, composición, migración 0003_suppliers.sql; Angular src/app/modules/suppliers; contrato/OpenAPI/runbook. No cambios del crawler ni migraciones anteriores; no dependencias nuevas. Roles comerciales siguen siendo explícitos. Datos históricos pueden conservar supplier_id null y nombre libre; snapshots/ResolveExact autorizados disponibles para futuros OC/Gastos.

- 43 pruebas Go/race/MySQL 5.7 satisfactorias: las 27 anteriores más 16 de Proveedores (5 unitarias/DTO y 11 de integración). Migración limpia/repetida, duplicados entre ocho actores, doce intenciones idempotentes y ocho versiones concurrentes verificadas.
- Rollback de agregado/evidencia/auditoría/hecho/intención ante fallo SQL; scopes/IDOR/equipos/revocación/reauth y BIGINT como cadena. Archivo vinculado exige ambas políticas y respeta cuarentena. Desactivar módulo bloquea transporte y retira su política.
- Fuente/fecha/actor, precios y MOQ exactos, evidencias sin precio, filtro literal por referencia, paginación e historial redactado verificados. Web/catalog nunca se declara cotización oficial. SDK MCP verifica get/search/resolve/price-history y revocación, sin escritura de agentes ni OAuth Work.
- npm ci/build y 14 pruebas Angular satisfactorios (8 anteriores, 6 Proveedores). DTO sin campos protegidos, claves/ediciones persistentes ante fallo, notas/precios conservan cambios pendientes, lector sin escritura; POST conserva ID/version confirmados si falla recarga. Fecha de fuente en Honduras convertida a UTC; presentación DatePipe fija UTC−06:00.
- Navegador real: Core, Clientes y Proveedores; catálogo/contacto/producto/MOQ, nota, precio exacto, persistencia, duplicado, país e inactivación. Última regresión de los tres flujos con zona horaria corregida satisfactoria, también aprobada en CI.
- 17 pruebas crawler offline satisfactorias; gofmt/vet/build y documentación verificados. Primer intento local sobre datadir checkpointado falló al recrear tablas InnoDB; directorio temporal nuevo y pruebas completas aprobadas. No se atribuye ese fallo al código productivo ni se utiliza una BD productiva para las pruebas.

Código final [8e9add2](https://github.com/Ahcast/Bastion/commit/8e9add2b2a66341d78c4b3480fac6adbdc6a95f1): [Core checks 38010479366](https://github.com/Ahcast/Bastion/actions/runs/38010479366) aprobó backend (vet, formato, race detector/MySQL 5.7 y builds), frontend (build y 14 pruebas) y los tres flujos de navegador reales. También aprobaron [documentación](https://github.com/Ahcast/Bastion/actions/runs/38010479382) y [rama única](https://github.com/Ahcast/Bastion/actions/runs/38010479365). Todas las pruebas de aceptación de esta unidad cerradas; conexiones externas mantienen los límites indicados arriba.

RFQ/aprobación/envío, cotizaciones recibidas oficiales, métricas calculadas e investigación automática conservan sus requisitos, pendientes de AI/Workflow y sus propietarios. No se activó correo ni se consultó HonduCompras/S3/Bluehost real. Proveedores no declara esas conexiones finalizadas por aprobar su catálogo manual.

## Unidad 3 — Procesos/receptor crawler

Código: migraciones 0004_processes.sql/0005_process_changes.sql, internal/modules/processes y composición, scopes técnicos explícitos en Core; Angular y contratos/runbook. Código Python, CLI, esperas/reintentos y ACK conservados. Solo datos sintéticos y MySQL 5.7 aislado; no consultas a HonduCompras ni S3/Bluehost productivos.

Fusión por campo/observed_at, evidencia atrasada durable, documentos parciales sin eliminación y hashes confirmados conservados. Identidad única, versiones/diffs y hechos transaccionales; ACK después de commit y replay autorizado. Cierres de snapshot verifican contadores de lotes y admiten nuevas entregas del mismo run. Marcas/notas personales no son campos de fuente. Consultas SQL paginadas/IDOR, exportación neutralizada con límite explícito, tres tools MCP de lectura. No se encolan jobs AI sin consumidor.

Pruebas locales: Angular build y 17 tests aprobados; 51 tests Go/race/MySQL y 17 crawler offline aprobados. Navegador y CI pendientes; no declarar unidad cerrada hasta registrar evidencia. Conversión a cotización, estados/asignación comercial, colaboración y AI mantienen sus dependencias; no hay endpoints simulados ni investigación/envíos activados. S3 raw no se valida mediante la recepción API.

## Continuidad

Orden: Clientes → Proveedores → Procesos/receptor crawler → Cotizaciones → OC de clientes → Facturas/pagos → Gastos → Reportes → Colaboración; Workflow/AI según dependencias. Clientes y Proveedores cerrados con evidencia local y CI satisfactoria. Unidad en curso: Procesos/receptor crawler, implementada y en verificación; no iniciar Cotizaciones hasta cerrar pruebas y CI. Leer contrato/spec de Procesos y contrato único IMPORT_CONTRACT del crawler antes de su implementación. Mantener all/self/team, snapshots, transacciones y permisos mínimos en cada unidad. No se han desplegado servicios ni migrado datos productivos.
