# Estado del proyecto

Actualizado el 10 de octubre de 2026. Core 0.1.0 publicado y validado en CI; Clientes 0.1.0 publicado y validado en pruebas locales y CI; unidad 1 cerrada. Proveedores 0.1.0 publicado y validado en pruebas locales y CI; unidad 2 cerrada. Catálogo de fuente y receptor de Procesos publicados y validados en pruebas locales y CI; unidad 3 de base cerrada. Producción no desplegada.

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
| Crawler Python | Conservado sin cambios; 17 pruebas offline satisfactorias; receptor Processes validado en CI |
| Clientes | Ficha/catálogo Go y Angular, contactos/direcciones/notas, historial, snapshots/resolución exacta, permisos y documentos implementados; pruebas locales y CI aprobados |
| Proveedores | Catálogo, contactos/etiquetas/productos, condiciones/evaluación manual, evidencia de precios, historial/notas/documentos y REST/MCP implementados; pruebas locales y CI aprobados |
| Procesos | Ingestión transaccional, observaciones/versiones, consultas, marcas/notas, CSV y REST/MCP/Angular implementados; catálogo/ingestión validados en pruebas y CI; funciones cruzadas pendientes |
| Cotizaciones | Borradores/cálculo, conversión, PDF de borrador/adjuntos, estados/revisiones y aprobación humana implementados y validados en CI; emisión/correo/bulk pendientes |
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

Pruebas locales: Angular build y 17 tests aprobados; 53 tests Go/race/MySQL aprobados sin integración omitida (43 anteriores + 10 Procesos: 5 unitarias y 5 de integración); 17 crawler offline aprobados. CI aprobó los cuatro flujos reales de navegador: Core, Clientes, Proveedores y Procesos. El lote de 30 registros y más de 4 MiB se confirmó sin cambios globales de max_allowed_packet. Historial incluye diff anterior/posterior transaccional. Conversión a cotización, estados/asignación comercial, colaboración y AI mantienen sus dependencias; no hay endpoints simulados ni investigación/envíos activados. S3 raw no se valida mediante la recepción API.

Código final [bab3790](https://github.com/Ahcast/Bastion/commit/bab37902f1ef250a386ae1f5f8ff11df8fd1c7d4): [Core checks 38015160340](https://github.com/Ahcast/Bastion/actions/runs/38015160340) aprobó backend (formato/vet/race/MySQL5.7/builds), frontend (build y 17 pruebas) y navegador (cuatro flujos reales). También aprobaron [documentación](https://github.com/Ahcast/Bastion/actions/runs/38015160330) y [rama única](https://github.com/Ahcast/Bastion/actions/runs/38015160324). Aceptación del catálogo/ingestión cerrada; no equivale a haber implementado conversión/flujo comercial/AI ni a validar infraestructura productiva.

## Continuidad

Orden: Clientes → Proveedores → Procesos/receptor crawler → Cotizaciones → OC de clientes → Facturas/pagos → Gastos → Reportes → Colaboración; Workflow/AI según dependencias. Clientes y Proveedores cerrados con evidencia local y CI satisfactoria. Unidad de base Procesos/receptor crawler cerrada con pruebas y CI satisfactorios. Cotizaciones en desarrollo: borradores/cálculo y conversión desde Procesos implementados; PDF/adjuntos cerrados; estados/revisiones y aprobación humana cerrados con pruebas y CI; continuar emisión/correo/bulk y pruebas del módulo antes de avanzar a OC. Leer contrato/runbook de Quotes. Mantener all/self/team, snapshots, transacciones y permisos mínimos en cada unidad. No se han desplegado servicios ni migrado datos productivos.

## Unidad 4a — Cotizaciones: borradores y cálculo

Migración 0006, internal/modules/quotes, snapshots públicos transaccionales de Customers/Processes, composición y Angular. shopspring/decimal v1.4.0 fijado. Calculador canónico exacto, descuento absoluto por unidad, porcentajes, distribución proporcional con residuo determinista, bases 15/18/exento/exonerado, costos/margen internos. Cabecera/items relacionales, versiones inmutables, contador bloqueado, idempotencia y auditoría/hechos en TX. REST/MCP consulta común con cadenas BIGINT/version; no escritura de máquinas ni grants implícitos al root.

Conversiones desde Procesos: solo entidad/expediente, cliente resuelto autorizado o snapshot manual, varias intenciones, sin items/documentos/notas/especificaciones. Edición conserva snapshot/origen. UI incluye cálculo backend, filtros URL, editor, versiones, clones/papelera y enlaces/lista paginada entre módulos. Flag retira rutas/tools y conserva datos. No hay migración productiva, correo enviado ni acceso S3/HonduCompras/Bluehost real.

Pruebas locales: 22 Angular y build aprobados; 64 Go/race/MySQL aprobadas sin integración omitida (53 anteriores + 11 nuevas: seis unitarias/DTO y cinco de integración). Formato, vet, builds y documentación aprobados. CI aprobó backend (MySQL5.7/race/builds), frontend (build/22 pruebas) y cinco flujos reales de navegador: Core, Clientes, Proveedores, Procesos y Cotizaciones. El nuevo flujo verifica creación desde proceso sin copiar ítems, cálculo, guardado/recarga, clonación y trazabilidad bidireccional. Integración real verifica ocho números concurrentes únicos, doce retries de intención, rollback del contador/aggregate/historial/auditoría/evento/intención, versiones, scopes all/self/team, reauth/revocación, snapshots, origen, múltiples conversiones y MCP SDK >2^53. Crawler/Python y migraciones 0001..0005 conservados.

Aceptación completa de Cotizaciones pendiente: PDF/S3 público sin información interna, adjuntos/política de archivos, SMTP inmediato/programado y ledger, emisión/aprobaciones/revisiones, bulk y FX fechado; conversiones OC/Factura al incorporar sus servicios propietarios. Permanecer en Cotizaciones, no iniciar OC. Esta unidad no acredita esos requisitos ni un despliegue.

Código de esta unidad [10b8baf](https://github.com/Ahcast/Bastion/commit/10b8bafe45bd2369259bcaa915ebb3c282ce4f92): [Core checks 38017148904](https://github.com/Ahcast/Bastion/actions/runs/38017148904), [documentación 38017148874](https://github.com/Ahcast/Bastion/actions/runs/38017148874) y [rama única 38017148896](https://github.com/Ahcast/Bastion/actions/runs/38017148896) satisfactorios. Unidad 4a cerrada con evidencia; Cotizaciones completo sigue pendiente. Próxima unidad dentro de Quotes: PDF/adjuntos y emisión segura, antes de correo/programación, estados/aprobaciones/bulk y aceptación integral. No se ha iniciado OC.

## Unidad 4b — Cotizaciones: PDF de borrador y adjuntos

Migración 0007, proyección pública/PDF Go A4 Unicode, snapshot SQL durable y subida S3 inmutable condicional, recuperación de fallo y registro Core transaccional. Sin costos, margen ni notas privadas. Documentos generados no se reemplazan; uploads mantienen cuarentena/scanner. Adjuntos requieren políticas de Quotes y Core; vincular versiona cotización, papelera/restauración versiona archivo, revocación live y módulo deshabilitado bloquean acceso. UI incluye generación/listado/descarga, vista previa autorizada, upload/reintento/vínculo y papelera paginada. PDF marcado BORRADOR, no aprobación ni envío.

Validación local: 27 pruebas Angular y build; 71 pruebas Go/race/MySQL5.7 (64 anteriores + 3 PDF, 3 integración de archivos y 1 presign seguro), formato/vet/build. PDF corto y seis páginas largas renderizados con Poppler e inspeccionados sin solapamientos/recortes. Verificadas privacidad, determinismo, glifos rechazados, concurrencia, versiones, rollback después de subida y replay, inmutabilidad, cuarentena y ambas políticas. Prueba browser extendida a vínculo/cuarentena/papelera/restauración. CI final aprobado: backend, frontend y cinco flujos reales de navegador. Papelera exige reauth; error de reautenticación se conserva para mostrar la instrucción al usuario. Recargas internas después de mutaciones no reescriben URL ni compiten con enlaces a Procesos; prueba de regresión incluida.

Cotizaciones completo continúa pendiente: emisión/estados/aprobaciones/revisiones, SMTP inmediato/programado y ledger, bulk/FX y conversiones OC/Factura por sus propietarios. No avanzar a OC. S3/SMTP/Bluehost productivos no activados ni validados; no despliegue/migración productiva.

Código final de esta unidad [9c85ac9](https://github.com/Ahcast/Bastion/commit/9c85ac91ad2323c3ac758be722ae9f5c8ae6471c): [Core checks 38021574460](https://github.com/Ahcast/Bastion/actions/runs/38021574460), [documentación 38021574512](https://github.com/Ahcast/Bastion/actions/runs/38021574512) y [rama única 38021574458](https://github.com/Ahcast/Bastion/actions/runs/38021574458) satisfactorios. Unidad 4b cerrada; Cotizaciones completo sigue pendiente. CI intermedio detectó navegación de filtros competidora y un fallo del registro diagnóstico de pruebas tras abandonar página; ambos corregidos y recorrido completo revalidado. No se ignoran ni se atribuyen esos fallos a servicios productivos. Crawler y migraciones 0001..0006 conservados byte a byte.

## Unidad 4c.1 — Cotizaciones: estados preliminares y revisiones

Migración 0008 registra cotizaciones.transition sin grants automáticos. Workflow evalúa grafo explícito; Quotes aplica draft→costing→review y retornos autorizados, control de versión, idempotencia y snapshot/auditoría/hecho en TX. Edición desde costeo/revisión genera nueva versión en borrador, sin cambiar número ni origen. Adjuntos editables solo borrador/costeo; revisión bloquea escritura documental. Papelera comercial solo borrador. PDF continúa BORRADOR. UI muestra estado real, pasos permitidos y bloqueo ante cambios sin guardar; conserva intención ante timeout y versión/estado confirmados aunque falle la recarga.

Validación local: build/30 pruebas Angular y 74 Go/race/MySQL5.7 aprobadas sin integración omitida; formato/vet/build y documentación aprobados. Las 17 pruebas crawler offline también aprobaron con sus dependencias fijadas. CI final aprobó backend, frontend y los cinco flujos reales de navegador, incluyendo costeo→revisión→edición/reinicio e historial. Pruebas nuevas cubren secuencia, revisiones, replay, saltos/emisión rechazados, ocho actores concurrentes, rollback, scopes/revocación y DTO estricto. Unidad 4c.1 cerrada con CI satisfactorio; no equivale a la aceptación completa de Cotizaciones.

Aprobaciones vinculadas a acción/hash/version/vigencia y reauth, emisión/PDF aprobado, SMTP inmediato/programado y ledger, bulk/FX y conversiones por propietarios siguen pendientes. Definiciones Workflow editables no implementadas por esta base. Permanecer en Cotizaciones antes de OC; ninguna integración productiva activada.

Código final [d80d72c](https://github.com/Ahcast/Bastion/commit/d80d72c8976ebe3e624b4916409f81ea7fd40b18): [Core checks 38029384941](https://github.com/Ahcast/Bastion/actions/runs/38029384941), [documentación 38029385007](https://github.com/Ahcast/Bastion/actions/runs/38029385007) y [rama única 38029384934](https://github.com/Ahcast/Bastion/actions/runs/38029384934) satisfactorios. CI intermedio detectó un selector de prueba ambiguo entre dos alertas; corregido para verificar el texto exacto y revalidado el recorrido completo. Un fixture local sin ítems conservaba un descuento general inválido; corregido antes de aprobar las 74 pruebas. Crawler y migraciones 0001..0007 conservados byte a byte. Próxima unidad: aprobación Workflow vinculada a acción/versión/hash/vigencia y emisión segura dentro de Cotizaciones.

## Unidad 4c.2 — Cotizaciones: aprobación humana vinculada

Revisión del avance local sin publicar: completados ledger Workflow, servicios Quotes, migración 0009, composición/flag, evidencia Core y UI de solicitud/decisión/revocación/reauth. Solicitud liga acción quote.emit, versión destino, contenido interno/público, compañía/version, plantilla/activos y metadata ordenada de adjuntos activos disponibles. Separación creador/solicitante/aprobador, scopes all/self/team, reauth al decidir/revocar y autoridad vigente del aprobador. Decisión/revocación versiona Quote; edición invalida pending/approved. Consumo único comparte TX con el futuro efecto del propietario, sin endpoint de emisión ni correo en esta unidad.

Revisión corrigió replay de solicitud tras vencer (ACK perdido), y ocultación del error de recarga tras confirmar decisión. Contratos/OpenAPI/runbook y estado de Workflow actualizados; no grants automáticos ni dependencias nuevas. PDFs actuales siguen BORRADOR incluso aprobados internamente. Siguiente unidad: emisión segura/PDF aprobado; luego SMTP/programación/bulk/FX antes de OC.

Validación local: build y 35 pruebas Angular aprobados; 87 pruebas Go/race/MySQL5.7 aprobadas sin skips, incluido el caso adicional de permisos por equipo. Diez nuevas integraciones de aprobación cubren decisiones/rechazo/revocación, contenido/empresa/activos/adjuntos, scopes/IDOR/reauth/revocación, 12 replays concurrentes y ocho decisiones, rollback, consumo único y replay vencido. 17 crawler offline, formato/vet/build y enlaces aprobados. Primeros fixtures corrigieron claves cortas y argumento secreto inválido; fallo local de tablespace al recrear esquema desapareció usando datadir desechable en /tmp, sin modificar migraciones ni parámetros productivos. CI inicial detectó selector prematuro y errores reales 1213/1267; corregidos selector, reserva exclusiva del bucket HTTP y búsqueda Unicode con pruebas específicas, además de diagnóstico sin SQL privado. CI final aprobó backend (vet/formato/race/MySQL5.7/builds), frontend (build/35 pruebas) y seis recorridos de navegador contra API/Angular/MySQL reales, incluido el flujo de aprobación con dos usuarios. La expectativa de negar una decisión inmediatamente después de login MFA se corrigió: ese login ya confirma identidad reciente; el navegador confirma identidad explícitamente y backend verifica rechazo con reauth vencida. No despliegue ni conexiones externas activadas.

Código final de esta unidad [c5005fc](https://github.com/Ahcast/Bastion/commit/c5005fcb649951653cf122fd6fc3ba89106d3184): [Core checks 38033240344](https://github.com/Ahcast/Bastion/actions/runs/38033240344), [documentación 38033240336](https://github.com/Ahcast/Bastion/actions/runs/38033240336) y [rama única 38033240337](https://github.com/Ahcast/Bastion/actions/runs/38033240337) satisfactorios. Unidad 4c.2 cerrada. Cotizaciones completo sigue pendiente; continuar emisión segura/PDF aprobado antes de SMTP/programación/bulk/FX y OC. Crawler y migraciones 0001..0008 conservados.

## Unidad 4c.3 — Cotizaciones: emisión segura en validación

Migración 0010, acción Emit y DTO/OpenAPI, kind de PDF, renderer issued y Angular. Reserva congelada, render/upload fuera de TX, revalidación live y consumo Workflow+documento+auditoría+quote.issued atómicos. Un único artefacto por aprobación; replay histórico autorizado; no sent/SMTP. Composición de navegador aislada permite descarga real de bytes del store sintético sin cambiar cmd/server. No infraestructura productiva activada.

Validación local: 93 Go/race/MySQL5.7 sin skips, vet/formato/build, 38 Angular/build y 17 crawler offline aprobados. PDFs issued de una y cinco páginas renderizados con Poppler e inspeccionados sin recortes/solapamientos. Casos cubren 16 solicitudes concurrentes con misma/diferente intención, rollback tras objeto y consumo, replay histórico, cambios/revocación/vigencia/grants/reauth/Files durante I/O, scopes/DTO/disabled y UI ante timeout/recarga fallida. Corregida carga duplicada de ítems durante verificación del binding. Crawler usa su entorno con dependencias fijadas. CI/navegador final pendientes; no declarar cerrada antes de sus resultados.
