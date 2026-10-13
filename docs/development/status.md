# Estado del proyecto

Actualizado el 11 de octubre de 2026. Core 0.1.0 publicado y validado en CI; Clientes 0.1.0 publicado y validado en pruebas locales y CI; unidad 1 cerrada. Proveedores 0.1.0 publicado y validado en pruebas locales y CI; unidad 2 cerrada. Catálogo de fuente y receptor de Procesos publicados y validados en pruebas locales y CI; unidad 3 de base cerrada. Producción no desplegada.

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
| Cotizaciones | Alcance monomoneda HNL/USD cerrado y validado en CI; incluye correo/bulk y estados finales |
| OC de clientes | Unidades 1, 2, 3 y 4 cerradas: agregado, cálculo, estados, entrega, conversión y documentos privados; clonación pendiente |
| Demás módulos comerciales | Pendientes; se implementan de uno en uno tras cerrar pruebas y CI del anterior |
| Despliegue | Templates systemd/nginx/Bluehost preparados; no instalados ni desplegados |
| Infraestructura real | MySQL TLS/túnel/hosting, DNS/HTTPS, S3/IAM, SMTP y memoria conjunta con crawler pendientes |
| Git | Solo main; limpieza reactiva vigente; prevención GitHub requiere plan Pro |

## Cierre vigente — 2026-10-11

Prevalece sobre las notas históricas inferiores. Cotizaciones está cerrado para HNL/USD separados, sin FX automático, certificado en `0a98bf756c4e7a3965b7b625473258509fbaa571`. OC unidades 1, 2, 3 y 4 están cerradas; el módulo permanece activo. Evidencia y próximo paso exacto en `../ESTADO_DESARROLLO.md`.

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

Orden: Clientes → Proveedores → Procesos/receptor crawler → Cotizaciones → OC de clientes → Facturas/pagos → Gastos → Reportes → Colaboración; Workflow/AI según dependencias. Clientes y Proveedores cerrados con evidencia local y CI satisfactoria. Unidad de base Procesos/receptor crawler cerrada con pruebas y CI satisfactorios. Cotizaciones en desarrollo: borradores/cálculo y conversión desde Procesos implementados; PDF/adjuntos cerrados; estados/revisiones, aprobación humana y emisión segura de PDF cerrados con pruebas y CI; continuar correo/bulk y pruebas del módulo antes de avanzar a OC. Leer contrato/runbook de Quotes. Mantener all/self/team, snapshots, transacciones y permisos mínimos en cada unidad. No se han desplegado servicios ni migrado datos productivos.

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

## Unidad 4c.3 — Cotizaciones: emisión segura cerrada

Migración 0010, acción Emit y DTO/OpenAPI, kind de PDF, renderer issued y Angular. Reserva congelada, render/upload fuera de TX, revalidación live y consumo Workflow+documento+auditoría+quote.issued atómicos. Un único artefacto por aprobación; replay histórico autorizado; no sent/SMTP. Composición de navegador aislada permite descarga real de bytes del store sintético sin cambiar cmd/server. No infraestructura productiva activada.

Validación local: 93 Go/race/MySQL5.7 sin skips, vet/formato/build, 38 Angular/build y 17 crawler offline aprobados. PDFs issued de una y cinco páginas renderizados con Poppler e inspeccionados sin recortes/solapamientos. Casos cubren 16 solicitudes concurrentes con misma/diferente intención, rollback tras objeto y consumo, replay histórico, cambios/revocación/vigencia/grants/reauth/Files durante I/O, scopes/DTO/disabled y UI ante timeout/recarga fallida. Corregida carga duplicada de ítems durante verificación del binding. Crawler usa su entorno con dependencias fijadas. CI final aprobó backend (formato/vet/race/MySQL5.7/builds), frontend (build/38 pruebas) y siete recorridos de navegador contra API/Angular/MySQL reales. El nuevo flujo verifica aprobación humana, emisión, descarga de bytes PDF, recarga y conservación de evidencia tras nueva revisión. El almacenamiento del navegador es sintético exclusivo de tests; no acredita S3 real.

Código final [555168f](https://github.com/Ahcast/Bastion/commit/555168ff38876bf7f55015f02e234e8e193d65bd): [Core checks 38034503276](https://github.com/Ahcast/Bastion/actions/runs/38034503276), [documentación 38034503236](https://github.com/Ahcast/Bastion/actions/runs/38034503236) y [rama única 38034503206](https://github.com/Ahcast/Bastion/actions/runs/38034503206) satisfactorios. Unidad 4c.3 cerrada; Cotizaciones completo continúa pendiente. Próxima unidad: correo inmediato/programado con destinatarios y adjuntos autorizados, mismo PDF emitido y ledger SMTP; después bulk/FX y aceptación integral antes de OC. Crawler y migraciones 0001..0009 conservados byte a byte. No despliegue, migración productiva ni envío real.

## Unidad 4c.4a — Servicios Core para correo comercial

Revisión del último cierre confirmó main 4b164e9 y recuperó exactamente sus 231 archivos desde GitHub después de la limpieza del entorno temporal. Se completó el sobre Core To/CC/BCC/adjuntos, validación previa al transporte y serialización MIME con la dependencia go-mail existente. BCC conserva destinatarios del sobre SMTP sin exponerse en cabeceras. Message-ID no se regenera; adjuntos mantienen bytes/nombres/tipos. El propietario comercial sigue obligado a autorizar y congelar el envío.

Migración 0011 y EnqueueAt conservan fecha UTC/microsegundos entre intención transaccional y job; eventos existentes continúan inmediatos. Pruebas nuevas cubren inyección/duplicados/límites/Unicode, sobre/MIME, publicación concurrente, programación/vencimiento, publicación tardía, eventos anteriores, rollback y revocación de permisos. [Contrato único](../contracts/mail-transport.md).

Validación cerrada: [código 996a9d8](https://github.com/Ahcast/Bastion/commit/996a9d846127823634a1f8cef1bd200aaefb5dda) y [Core checks 38059653268](https://github.com/Ahcast/Bastion/actions/runs/38059653268) aprobados: 100 pruebas Go con race detector/MySQL5.7 real, formato/vet/builds, build/38 pruebas Angular y siete recorridos reales API/Angular/MySQL. [Documentación 38059653296](https://github.com/Ahcast/Bastion/actions/runs/38059653296) y [rama única 38059653287](https://github.com/Ahcast/Bastion/actions/runs/38059653287) aprobadas. Las 17 pruebas crawler offline aprobaron localmente con requirements.lock; crawler y migraciones 0001..0010 conservados byte a byte. Localmente aprobaron formato/vet/build Go y 35 pruebas sin BD; 65 integraciones se omitieron por DSN ausente y no se contaron como aprobadas. MySQL 5.7.44 recuperado no arrancó localmente porque el entorno rechazó bind de socket Unix (Operation not permitted); las 65 integraciones sí se ejecutaron y aprobaron en CI, sin SQLite. No hay nuevas rutas comerciales, estados sent, destinatarios inventados ni envíos reales. Unidad técnica 4c.4a cerrada. La solicitud/decisión de autorización vinculada a destinatarios/contenido, ledger y formulario de correo Quotes siguen en la próxima unidad propietaria; Cotizaciones completo y OC no se declaran terminados.

## 2026-10-11 — OC Unidad 2 cerrada

Transiciones adyacentes de cabecera/ítems, historial, versión/idempotencia, permisos/scopes y entrega con reauth implementados en API y UI española. Auditoría/eventos atómicos, concurrencia, rollback, revocación y rechazo de máquina cubiertos. Edición íntegra bloqueada tras cualquier avance; descuento general corregido al recargar.

Código certificado `0fa94796ffee0c248035f94a9166f22c0a955b42`; [Core checks 38154939036](https://github.com/Ahcast/Bastion/actions/runs/38154939036), documentación `38154939037` y rama única `38154939034` aprobados. Go/MySQL5.7/race/formato/vet/builds, Angular build/64 pruebas y navegador 9/9 aprobados en CI. Las 17 pruebas offline crawler aprobaron localmente. No se acredita infraestructura productiva. Próxima unidad: conversión Cotización → OC por PurchaseOrders; después documentos/clonación y cierre integral, antes de Facturas.

## 2026-10-11 — OC Unidad 3 cerrada

Conversión de Cotización Aceptada a OC recibida por servicios propietarios, una TX y una OC por fuente. Copia snapshots/finanzas sin refrescar catálogos, preserva ID/versión de origen, vínculo inverso y Quote convertida/versionada; evidencia e idempotencia atómicas. UI española con confirmación explícita y vínculos independientes. No copia documentos/notas privadas ni activa correo.

[HEAD 541bb101](https://github.com/Ahcast/Bastion/commit/541bb101db51b391265b95ca9a8acd0ff0a3ae85), [Core checks 38159858594](https://github.com/Ahcast/Bastion/actions/runs/38159858594), documentación `38159858792` y rama única `38159858687` aprobados. Backend formato/vet/race/MySQL5.7/builds; Angular build/68 pruebas; navegador **10/10** aprobado. Local: Angular build/68 pruebas, listado compilado de 10 escenarios y documentación. Go/BD/navegador certificados por CI, no por ejecución local. Concurrencia de 12 replays/ocho actores, rollback, snapshots/finanzas y scopes/revocación cubiertos. Selectores de prueba y aislamiento del expediente corregidos tras CI intermedio; regresión completa final aprobada. Crawler/migraciones anteriores intactos. Próxima unidad: documentos privados OC; después clonación y cierre integral, antes de Facturas. No producción.

## 2026-10-11 — Unidad 4 OC: documentos privados cerrada

Contrato único `purchase-order-documents.md`, migración 0018 y política propietaria mediante Core.Files. Documentos privados de cabecera/ítems; tabla de vínculos con FK compuesto, ID estable de ítem conservado al editar/reordenar, actor/fecha y documento único. Core conserva versiones, checksum/MIME/tamaño, cuarentena/análisis, preview/descarga autorizada y papelera/restauración sin borrar vínculos ni bytes. Permiso explícito ordenes.documents.write sin concesión automática; scopes de OC y documento independientes, actor humano/reauth, replay reautorizado, versión OC/idempotencia/auditoría/evento en una TX. Writes operativos hasta entregada; estados protegidos denegados.

Angular español: destino cabecera/ítem, upload privado con etapa/clave conservada tras fallo, vínculo de documento existente, filtros/paginación/papelera, reauth y controles por estado/permiso. Versión confirmada conservada tras fallo de recarga; respuestas tardías no afectan otra OC abierta. No copia adjuntos de la cotización origen.

HEAD de código certificado: `1073c261de0c8f7ab12aed0ff6bb7b6a5f361e7c`. [Core checks 38163339892](https://github.com/Ahcast/Bastion/actions/runs/38163339892), [documentación 38163339970](https://github.com/Ahcast/Bastion/actions/runs/38163339970) y [rama única 38163339924](https://github.com/Ahcast/Bastion/actions/runs/38163339924): success. Backend: gofmt/vet, `go test -race -count=1 ./...` con MySQL 5.7 y builds server/worker/admin. Angular: build/**73 pruebas**. Navegador: **10/10 escenarios**, incluidos upload/finalize real contra adapter sintético privado, adjuntos de cabecera/ítem en cuarentena, filtro, papelera/restauración y recarga.

Local: build/73 pruebas Angular, compilación de 10 escenarios Playwright, paridad/enlaces y diff sin errores. Go/BD/navegador certificados por CI, no por ejecución local. Pruebas nuevas: identidad estable al reordenar/editar, eliminación de ítem vinculado bloqueada incluso en papelera, IDs duplicados/ajenos, FK/pertenencia, cuarentena/scan/preview/descarga, 12 replays concurrentes, autorización de ambos propietarios/revocación, scopes técnicos self/team y membresía, estados protegidos, rollback del vínculo/versiones ante fallo de evento, DTO HTTP estricto/CSRF y flag de composición.

CI intermedio detectó sincronización pendiente al volver al listado Quotes: ahora el escenario espera navegación/carga inicial antes del filtro y verifica respuesta filtrada/total. Se agregó además defensa de producción contra respuestas de listado fuera de orden y su prueba. El escenario de conversión captura el cuerpo HTTP al recibirlo para impedir que navegación lo descarte. Suite completa final aprobada; sin omitir comprobaciones ni ampliar timeouts para estas correcciones. CI conserva contexto de navegador sólo ante fallo. Crawler y migraciones 0001..0017 intactos. S3/escáner productivo no certificados; no despliegue, envío ni migración productiva.

Próximo: clonación de OC y aceptación integral, antes de Facturas/pagos.

## 2026-10-11 — Unidad 5 OC: clonación cerrada

Contrato `purchase-order-cloning.md`, migración 0019 y POST clone estricto. Copia snapshots históricos de cliente/proveedor y versiones, moneda HNL/USD, cantidades, impuestos y descuentos. Número, fechas, destino, comentarios y gastos nuevos explícitos. OC independiente en Recibida/ítems Pendiente con identidad nueva, sin documentos, entrega, historial operativo ni vínculo Quote directo. La fuente conserva su versión/estado y el destino registra origen/versión durable. Permisos view/create y equipo actuales; replay reautoriza también lectura del destino. Idempotencia, cabecera/ítems, relación, auditoría/eventos comparten TX. Sin grants automáticos ni escritura técnica/MCP.

HEAD certificado `6667fe62f952fd1702c627ae88138bab9924a522`. [Core checks 38165562083](https://github.com/Ahcast/Bastion/actions/runs/38165562083), [documentación 38165562010](https://github.com/Ahcast/Bastion/actions/runs/38165562010) y [rama única 38165561926](https://github.com/Ahcast/Bastion/actions/runs/38165561926): success. Go fmt/vet, race/MySQL 5.7 y builds server/worker/admin; Angular build/**75 pruebas**; navegador **10/10** con clonación de OC entregada y adjuntos, cálculo preservado, reinicio operacional, adjuntos aislados y recarga.

Pruebas nuevas: vectores mixtos HNL/USD 15/18/exento/exonerado y descuentos, 12 replays concurrentes, versión/intención conflictivas, fuente inmutable, estados/IDs nuevos, documentos/historial no copiados, scopes de fuente/destino/revocación, catálogo actualizado y acceso de catálogo retirado con snapshots históricos conservados, creación en equipo ajeno y membresía perdida, concurrencia con transición y orden uniforme de locks, rollback de evento y retry, DTO/CSRF estricto, UI clave/destino conservados ante fallo. Local: build/75 pruebas Angular, compilación 10 escenarios, paridad/enlaces y diff. Go/MySQL/navegador acreditados por CI.

CI intermedio identificó formato de plantilla que separaba una expresión monetaria y fallos del buffer CDP al leer respuestas comerciales. Plantilla anterior conservada; pruebas de conversión/clonación capturan la respuesta del servidor mediante forwarding real sin alterar petición/cuerpo, y la devuelven al navegador. No datos simulados, aserciones retiradas ni timeouts ampliados. Regresión completa final aprobada. Sin despliegue, correo, migración productiva ni certificación S3/escáner/Bluehost.

Siguiente unidad: aceptación integral OC, incluyendo política de eliminación reversible y utilidad canónica, antes de Facturas/pagos.

## 2026-10-11 — Unidad 6 OC: aceptación integral en validación

Contrato purchase-orders-acceptance.md y matriz development/purchase-orders-acceptance.md. Migración 0020 con deleted_at, profit/profit_version, backfill decimal e índice de papelera; ordenes.delete sin concesión automática. Eliminación reversible solo Recibida/ítems Pendiente y sin conversión, documentos (incluida papelera), historial o clones derivados. Reauth/razón, scopes view/delete, versión/idempotencia/auditoría/evento en TX; protege número y snapshots. Ficha/papelera autorizadas; mutaciones/archivos de eliminadas bloqueados. Restauración conserva datos y devuelve fecha servidor. Utilidad canónica = ventas netas sin ISV menos inversión total; función finance.Profit compartida también por margen Quotes sin gastos adicionales. Monedas separadas, pérdidas permitidas.

Backend/UI implementados; pruebas de preservación, revocación/reauth/replay/rollback, evidencia protegida, identidad y número externo conservados sin unicidad global, backfill real MySQL, HTTP/CSRF, UI respuesta confirmada y navegador papelera/restauración/utilidad añadidas. Regresión y CI pendientes; no cerrar módulo ni iniciar Facturas hasta completar esta aceptación. Producción e integraciones externas pendientes.

## 2026-10-11 — OC: aceptación integral cerrada

HEAD certificado `a36de697a4c0d39d16f7095a76d443258deec3f5`. [Core checks 38175683879](https://github.com/Ahcast/Bastion/actions/runs/38175683879), [documentación 38175683895](https://github.com/Ahcast/Bastion/actions/runs/38175683895) y [rama única 38175683826](https://github.com/Ahcast/Bastion/actions/runs/38175683826): success. Go fmt/vet, pruebas race contra MySQL 5.7 y builds server/worker/admin aprobados; Angular build y **77 pruebas**; navegador **10/10**.

Papelera/restauración preserva identidad, número externo, ítems, snapshots y cálculo; bloquea cambios operativos, documentos, conversión e historial protegidos; replay reautoriza permiso/reauth vigente y rollback conserva versión. Número externo sin unicidad global. Utilidad exacta sin ISV menos inversión total, negativa permitida y monedas separadas; backfill 0020 validado. En navegador, clon independiente con gastos 9.99 conserva utilidad 85.01 HNL tras eliminar, recargar y restaurar. Fixture sintético posee ordenes.delete explícito; no grants productivos automáticos.

Unidades 1..6 y alcance operativo propietario de OC cerrados. OC→Factura/Facturado/Cerrado, búsqueda fiscal, Reportes/Inicio e IA se incorporan con sus propietarios según matriz de aceptación. Siguiente módulo: **Facturas/pagos**, comenzando por contrato financiero y borradores; emisión fiscal exige contrato de series/SAR y configuración validada. No producción ni integraciones externas certificadas.

## 2026-10-11 — Facturas/pagos, unidad 1 en validación

Contrato propietario `docs/contracts/invoices.md` definido antes del código. Motor interno InvoiceFinancials reutiliza finance.Calculate y finance.Profit; cálculo mixto ISV15/18/exento/exonerado, descuentos, inversión por línea/gastos y utilidad exacta negativa. Moneda explícita HNL/USD. ProjectPayments/ApplyPayment proyectan abonos, saldo y pago completo sin floats ni sobrepago, también por encima de 2^53. No escriben pagos, cambian estados, asignan número/SAR ni exponen acciones.

Archivos: backend/internal/modules/invoices/financials.go y financials_test.go. Unidad pura sin migración ni permisos/rutas nuevos. Formato WASM gofmt, documentación y diff aprobados localmente; Go fmt/vet/race y regresión MySQL/Angular/navegador pendientes en CI. No declarar facturación operativa. Después de certificar: contrato/migración y agregado Borrador con REST/UI, autorización, versionado y evidencia. Series/SAR e integración productiva permanecen pendientes.

## 2026-10-11 — Facturas/pagos, unidad 1 certificada

HEAD `c85a1e37e0d120d84415e02299c48dbebbc04588`: [Core checks 38176231851](https://github.com/Ahcast/Bastion/actions/runs/38176231851), [documentación 38176231870](https://github.com/Ahcast/Bastion/actions/runs/38176231870) y [rama única 38176231869](https://github.com/Ahcast/Bastion/actions/runs/38176231869): success. Go fmt/vet/race, regresión MySQL 5.7 y builds server/worker/admin aprobados; Angular build/**77 pruebas** y navegador **10/10**. Cuatro pruebas propietarias financieras cubren impuestos mixtos/descuentos/inversión/utilidad negativa/monedas, límites, abonos/sobrepago/pago completo y precisión por encima de 2^53.

Unidad 1 cerrada. Facturas/pagos sigue activo: siguiente unidad, Borradores persistentes con REST/UI y seguridad/evidencia; definir contrato y migración antes del código. El motor no expone acciones, registra pagos ni emite documentos fiscales. Producción, series/SAR y demás unidades siguen pendientes.

## 2026-10-11 — Facturas/pagos, unidad 2 Borradores en validación

Contrato/OpenAPI y migración 0021 definidos: invoices/invoice_items normalizados, cabecera y cliente manual/registrado, snapshots históricos, contado/crédito, moneda HNL/USD, notas públicas y cálculo/inversión/utilidad del motor certificado. Número/SAR nullable y sin contador/emisión. Saldo inicial igual al total, pagos cero; no cobro real.

Servicios REST create/get/list/update/calculate, permisos facturas.view/create/update sin grants, scopes all/self/team, versión/idempotencia/TX/auditoría/eventos e IDs de ítems preservados al editar/reordenar. Cambio de equipo requiere all+reauth; replay reautoriza destino. Flag BASTION_INVOICES_ENABLED, composición opcional Customers y UI española con menú/ruta protegidos. Cliente conserva clave tras ACK incierto, resultado confirmado si falla recarga y bloquea writes hasta recuperar la ficha; vista previa obsoleta se oculta si cambia cualquier dato financiero.

Pruebas: cuatro financieras certificadas; validación DTO/condiciones, cinco integraciones nuevas (round-trip/IDs, snapshots/revocación/scopes, 12 replays y ocho ediciones concurrentes/rollback, HTTP/CSRF/estado protegido, equipos/reauth/revocación), composición/flag, cinco Angular y escenario navegador real de Facturas. Localmente Angular build y **82 pruebas** aprobados; documentación/diff y compilación de **11 escenarios**. CI Go fmt/vet/race/MySQL/builds, Angular y navegador pendientes; no cerrar esta unidad hasta verde. Emisión fiscal, pagos auditados, conversiones, documentos/anulación/clonación y aceptación integral siguen en orden. No despliegue ni migración productiva.

## 2026-10-11 — Borradores: corrección de accesibilidad y selección

Primera ejecución 11f9dae5fa7a09b3afb4b7519f30c563dcfa2369 / Core checks 38177764409: backend y frontend success; navegador 10/11, falla únicamente al localizar etiqueta exacta del selector de condición de pago. Etiquetas for/id separan opciones del nombre accesible; nueva prueba DOM elige Crédito. Selección de cliente por nombre consulta Customers solo con permisos/flag, conserva snapshot al repetir identidad y recupera anterior ante revocación; selector de equipo por nombre solo con core.teams.read y elecciones filtradas por membresía/scopes. Sin pedir IDs en el formulario. Tres pruebas Angular adicionales: accesibilidad, catálogo/snapshot/revocación y consultas denegadas. Build y **85 pruebas** locales aprobados; repetir CI completo sobre el commit corregido antes de cierre.

## 2026-10-11 — Facturas/pagos, unidad 2 Borradores certificada

HEAD `88577fa0fa810958a65589beda3d9e331b8c3317`: [Core checks 38179176643](https://github.com/Ahcast/Bastion/actions/runs/38179176643), [documentación 38179176704](https://github.com/Ahcast/Bastion/actions/runs/38179176704) y [rama única 38179176676](https://github.com/Ahcast/Bastion/actions/runs/38179176676): success. Go fmt/vet/race, MySQL 5.7 y builds server/worker/admin aprobados; Angular build y **86 pruebas**; navegador **11/11** (32.5 s).

Borradores persistentes con REST/UI, contado/crédito, snapshots históricos, ítems estables, cálculo y saldos exactos, permisos/scopes, reauth al cambiar equipo, idempotencia concurrente, rollback y recuperación de respuesta confirmada validados. Facturas no asigna número/SAR, cobra ni cambia estados fiscales. Se corrigió la actualización del destinatario de correo Quotes y se comprueba su valor antes de autorizar; regresión real de PDF, aprobación y worker/SMTP sintético aprobada. Ninguna prueba acredita correo real ni facturación productiva.

Unidad 2 cerrada. Siguiente: unidad 3, configuración fiscal/correlativo/emisión, empezando por política interna de rango, vigencia y formato; después persistencia transaccional, permisos, REST/UI y emisión. Configuración SAR real, autorización del rango, formato legacy y validación fiscal productiva continúan pendientes; no inventar datos ni reglas legales.

## 2026-10-11 — Facturas, unidad 3: política fiscal interna en validación

Contrato previo en contracts/invoices.md. fiscal_policy.go implementa validación de configuración explícita, rango inclusivo, vigencia actual y fecha factura, moneda y formato preservado; proyección exacta de número/siguiente/snapshot sin mutaciones. Pruebas cubren agotamiento, overflow, fechas imposibles, backdating tras vencimiento y precisión mayor que 2^53. Bloque puro, sin migración, rutas, grants, contador persistido ni emisión. Gofmt WASM/documentación/diff locales aprobados; Go/CI pendiente.

Siguiente bloque de esta misma unidad, tras certificar: contrato/migración de configuración propietaria, document_counters transaccional, permisos/reauth/idempotencia y emisión REST/UI con snapshot histórico. No cambiar unidad ni declarar emisión implementada por esta proyección. Pendientes productivos concretos: autorización SAR, rango/fecha límite, formato legacy y modalidad/moneda fiscal aplicable. Consulta al portal oficial no sustituye esos datos ni validación legal; no activar producción.

## 2026-10-11 — Facturas, unidad 3: política interna certificada

HEAD `5d03d523f5340acf55e87c4659305ef445fe1f54`: [Core checks 38179768823](https://github.com/Ahcast/Bastion/actions/runs/38179768823), [documentación 38179768828](https://github.com/Ahcast/Bastion/actions/runs/38179768828) y [rama única 38179768819](https://github.com/Ahcast/Bastion/actions/runs/38179768819): success. Go fmt/vet/race/MySQL/builds, Angular build/86 pruebas y navegador 11/11 aprobados. Política pura de rango/vigencia/formato/moneda y precisión exacta certificada; no emisión ni contador persistido.

La unidad 3 sigue abierta. Próximo bloque: configuración persistente bajo contrato previo añadido a contracts/invoices.md, migración 0022 y contador document_counters, permisos/reauth/REST/UI y evidencia atómica. Después emisión. Activación fiscal productiva y autorización real continúan pendientes.

## 2026-10-11 — Facturas, unidad 3: configuración persistente en validación

Migración 0022 invoice_fiscal_settings y permisos fiscal.read/manage sin grants; configuración y primer contador document_counters.invoice atómicos. GET/PUT propietario con DTO estricto, permisos globales, humano/reauth/CSRF, versión/idempotencia/audit/event. Editar no cambia contador; protege formato tras numeración y snapshots históricos. Bootstrap/replay/rollback/error de locks protegidos. UI en Facturas, sin nuevo menú, solicita consulta explícita y reauth; clave de respuesta incierta conservada, resultado confirmado bloquea otros writes hasta recargar. PUT incluido en CORS/Client y probado.

Cuatro integraciones MySQL nuevas: scopes/revocación/reauth y versión, 12 replays concurrentes y rollback alta/edición, agotamiento/formato histórico, HTTP DTO/CSRF/reauth/CORS. Cuatro pruebas Angular nuevas y navegador con operador separado de configuración, sin crear/cobrar/emitir facturas. Build y **90 pruebas** locales aprobados; CI Go/race/MySQL/builds y navegador **12 escenarios** pendientes. No cerrar bloque ni iniciar código de emisión hasta verde. Producción y autorización fiscal real pendientes.

Configuración fiscal: UI distingue autorización deshabilitada/rango agotado usando enteros exactos; conserva estado de última consulta al editar. Prueba adicional cubre límite 2^53 y edición pendiente. Scopes self no habilitan configuración global. **91 pruebas Angular** locales aprobadas; nueva revisión requiere CI completo antes de cierre.

Prueba de configuración fiscal: el login contraseña+MFA ya constituye reauth reciente, como establece Core; el recorrido ejerce confirmación explícita con código de recuperación sintético independiente. Rechazo de sesión sin reauth y replay tras revocación siguen cubiertos en MySQL/HTTP. Primera ejecución 38180612222: Angular success y navegador 11/12; fallo era la expectativa incorrecta sobre login, no omisión del control backend. Repetir CI sobre la corrección antes de cerrar.

## 2026-10-11 — Facturas, unidad 3: configuración persistente certificada

HEAD `133b9dc05ef5c5465d13e50550437c86a1bdb940`: [Core checks 38180883877](https://github.com/Ahcast/Bastion/actions/runs/38180883877), [documentación 38180883969](https://github.com/Ahcast/Bastion/actions/runs/38180883969) y [rama única 38180883848](https://github.com/Ahcast/Bastion/actions/runs/38180883848): success. Go fmt/vet/race/MySQL 5.7 y builds server/worker/admin aprobados; Angular build/**91 pruebas**; navegador **12/12**. Cuatro integraciones nuevas validan configuración/counter atómicos, rollback alta/edición, 12 replays, versión/scopes/revocación/reauth, formato consumido, agotamiento y HTTP/CSRF/CORS.

La UI distingue estado consultado/edición pendiente, conserva clave ante ACK incierto y bloquea nuevo guardado tras confirmación hasta recargar. Operador de configuración sintético separado no crea/cobra/emite facturas. Login con contraseña+MFA cuenta como confirmación reciente; navegador también usa confirmación explícita de recuperación y backend rechaza sesiones sin reauth.

Bloque de configuración cerrado; unidad 3 sigue activa y continúa emisión. Contrato de emisión previo definido en contracts/invoices.md: migración 0023, versiones/snapshots de empresa y autorización, vencimiento y correlativo/evidencia en una TX. Aún sin código de emisión/pagos/PDF. Autorización/modalidad/moneda fiscal reales y despliegue productivo siguen pendientes.

## 2026-10-11 — Facturas, unidad 3 emisión en validación

Contrato/OpenAPI previos y migración 0023: datos de emisión/empresa/vencimiento nullable en Borrador; facturas.emit sin grants productivos. EmissionContext/Emit humanos por scopes view/emit, reauth/CSRF/versiones/idempotencia. Valida configuración/vigencia/fecha Honduras/moneda/cálculo e ítems exactos y empresa con RTN explícito; no corrige importes silenciosamente. Número/estado issued, snapshots fiscal/empresa y versiones, vencimiento, contador/audit/event en una TX. Replay reautoriza registro/acción/reauth y conserva evidencia aun tras configuración posterior. Emisión no registra pagos ni PDF.

UI revisa datos guardados, etiqueta número estimado sin reservar, conserva claves/respuesta confirmada y congela ficha mientras emite o hasta recuperar recarga fallida. Listado/ficha usan número fiscal y estado Emitida; mostrados CAI/empresa históricos. Dos pruebas de reloj/vencimiento, cinco integraciones MySQL (con subcasos de versiones/vigencia/cálculo/moneda/scopes/HTTP), 12 replays y seis emisiones por dos humanos concurrentes/rollback. Seis pruebas Angular nuevas (97 totales), compilación local aprobada y escenario navegador de emisión con operadores separados y API real. CI Go fmt/vet/race/MySQL/builds, Angular y **13 recorridos** pendientes; no cerrar emisión ni empezar pagos hasta verde.

Navegador usa datos sintéticos: empresa con RTN configurado por administrador, autorización configurada por fiscal, emisor self independiente, lectura después de cambiar CAI conservando snapshot. No activación productiva, validación legal de RTN/CAI ni envío al SAR. Producción requiere autorización real/rango/modalidad/moneda y revisión fiscal.


Validación de emisión: fuente 2f92e78405fa8043110fa27346df859418ae1ecd / Core 38182746385 pasó backend Go/MySQL/race/builds y frontend 97, navegador 12/13. Causa del nuevo recorrido: administrador con TOTP enrolado dinámicamente por Core, distinto al secreto estático de otros actores. Se añadió operador de empresa sintético separado y se reutiliza sesión fiscal para evitar doble login/TOTP. Revisión 924be75e81aea276ccebcb6f7919b783273cac2e llegó a revisión fiscal; la prueba editaba RTN antes de completar consulta de empresa. Corrección espera datos y verifica RTN persistido; el backend rechazó datos incompletos. Emisión todavía requiere CI completo verde; unidad 4 solo tiene contrato previo, sin código de pagos.


## 2026-10-11 — Facturas, unidad 3 emisión certificada; continúa unidad 4

Código certificado `fa8670bcf68c11a07a2ed9ff7aedc7d23a4bc890`: [Core checks 38183414256](https://github.com/Ahcast/Bastion/actions/runs/38183414256), [documentación 38183414230](https://github.com/Ahcast/Bastion/actions/runs/38183414230) y [rama única 38183414289](https://github.com/Ahcast/Bastion/actions/runs/38183414289): success. Go fmt/vet/race con MySQL 5.7 y builds server/worker/admin aprobados; Angular build y **97 pruebas**, navegador **13/13 en 44.4 s**. Emisión conserva snapshots fiscal/empresa, valida tres versiones/cálculo/vigencia/moneda/fecha Honduras, asigna correlativo con evidencia atómica y protege replays/scopes/reauth/rollback/concurrencia. Recorrido sintético verifica RTN persistido antes de revisión y CAI histórico tras modificar configuración.

Unidad 3 cerrada para desarrollo sintético. Continúa unidad 4 bajo contrato previo y OpenAPI de payments: cobros auditables, anulación sin borrado y proyección comercial exacta. Unidad 4 aún sin implementación; conversiones, documentos/anulación/clonación y aceptación integral siguen en orden. Ningún recibo/pago bancario/Siafi externo ni validación fiscal/despliegue productivo acreditados.


## 2026-10-11 — Facturas, unidad 4 pagos en validación

Contrato previo 2c733b22a84ba3436ed88553f6c1a7dac7bc985a y migración 0024 payments; permisos payment/payment.void sin grants productivos. Ledger normalizado inmutable/anulable con versión, fecha/método/moneda/importes/referencia/razón y receipt_number NULL. Servicio humano view/action con reauth/CSRF/versión/intención; factura bajo lock, saldo recalculado desde filas activas, no sobrepago ni FX/retenciones. Register/void cambian exclusivamente proyecciones/versiones con audit/event y evidencia atómica; rollback y replay reautorizados. Estado documental issued se conserva y commercial_status deriva Pendiente/Parcialmente pagada/Pagada/Vencida en reloj Honduras; filtro/listado/ficha y ledger sin agregados entre monedas.

UI consulta explícitamente ledger vigente, registra/anula con razón y reauth, conserva clave ACK incierto y resultado confirmado si falla recarga, bloquea padre durante HTTP y nuevas mutaciones hasta recuperar versión. Seis integraciones MySQL nuevas cubren persistencia/anulación/cuatro métodos, total cero/importes >2^53, versiones/invalidaciones/moneda/fechas, 12 replays, ocho cobros en contención por dos humanos, rollback register/void, HTTP/CSRF/scopes/reauth/revocación. Prueba pura de vencimiento y prioridades; seis Angular nuevas. Build local aprobado; 102 pruebas locales previas al último caso de padre (objetivo 103) y suite de navegador ampliada con abono/anulación/cobro completo y contador/snapshots inmutables. Go fmt/vet/race/MySQL/builds, Angular y navegador completos pendientes; no cerrar pagos ni empezar conversiones hasta CI verde. Sin recibos/PDF/Siafi bancario ni validación fiscal productiva.


## 2026-10-12 — Pagos: corrección de escenarios de aceptación

CI 38184452309 aprobó frontend y navegador, pero falló en tres escenarios backend. Dos usaban claves de intención inferiores al mínimo Core de ocho caracteres y recibían validation antes de alcanzar sobrepago/scopes; se corrigieron las claves sin alterar controles productivos. El tercero leía una emisión manipulada sin vencimiento; ahora captura el borrador antes de modificar únicamente el estado para verificar que Save rechaza escrituras sobre issued. Cambios en pruebas publicados en fa11c138317b770b2d9d2d18493f017f00692ff8. Regresión completa Go/race/MySQL/builds, Angular y navegador en curso; unidad 4 permanece abierta hasta CI verde.


## 2026-10-12 — Facturas, unidad 4 pagos certificada

Código certificado `0db033c44d57326d42bb1e83459b5a1709a04c46`: [Core checks 38195876549](https://github.com/Ahcast/Bastion/actions/runs/38195876549), [documentación 38195876513](https://github.com/Ahcast/Bastion/actions/runs/38195876513) y [rama única 38195876471](https://github.com/Ahcast/Bastion/actions/runs/38195876471): success. Go fmt/vet/race con MySQL 5.7 y builds server/worker/admin aprobados; Angular build y **103 pruebas**; navegador **13/13**. Ledger de abonos/anulaciones, saldo exacto, sobrepago, scopes/revocación/reauth, rollback, idempotencia y cobros concurrentes certificados. Número/correlativo y snapshots fiscales/empresa se conservan; ningún recibo ni integración bancaria/Siafi real acreditados.

Las fallas previas de aceptación provenían de claves de prueba inferiores al mínimo Core y lectura de una emisión incompleta fabricada por el test; se corrigieron escenarios sin relajar reglas productivas. La corrección inicial cambió accidentalmente dos nombres version del test HTTP; se restauraron y la suite íntegra final aprobó. Unidad 4 cerrada. Siguiente unidad 5: conversiones Cotización→Factura y OC→Factura por servicios propietarios, contrato/migración previos y evidencia transaccional/idempotente. Documentos/anulación/clonación y aceptación integral mantienen su orden; Facturas completo y producción todavía pendientes.


## 2026-10-12 — Facturas, unidad 5: Cotización→Factura en validación

Contrato previo 01b150e79025f8bf175c8b186133041e97e314dd y migración 0025: vínculos únicos Quote.invoice_id e Invoice.source_quote_id/version. Source Quotes bloquea/autoriza cotización accepted sin otro destino; Target Invoices crea borrador íntegro desde snapshots/cálculo bajo la misma TX. No consulta catálogo ni copia documentos/notas privadas; términos/notas/gastos/razón explícitos. Fuente converted versionada con historial/auditoría y eventos, destino draft con origen inmutable y sin número/SAR/contador/pagos. Replays autorizan fuente y destino actuales; competencia contra Quote→OC produce un solo derivado. Clonar Quote elimina ambos vínculos.

REST estricto/flags, interfaz española con recuperación de ACK y vínculo confirmado, ficha bloqueada durante conversión, enlaces cruzados por query id y distinción Convertida a OC/factura. Cuatro integraciones MySQL nuevas: finanzas mixtas USD/cliente histórico inactivo y catálogo revocado/origen conservado/clone, 12 replays/competencia de destinos por dos actores, rollback/scopes/HTTP/CSRF/revocación/propietario deshabilitado, estados/versiones/metadata/cálculo alterado. Angular build local aprobado y **107 pruebas** locales; Playwright compila **14 escenarios**, con nuevo recorrido de conversión/recarga/vínculos. Go fmt/vet/race/MySQL/builds y navegador completos pendientes. No cerrar bloque ni comenzar código OC→Factura antes de CI verde; unidad 5 completa sigue pendiente, así como documentos y producción.


## 2026-10-12 — Facturas, unidad 5 primer bloque certificado

Código `20bd810901ed2762cb1c7d828f86ee63c5b10f8e`: [Core checks 38197746643](https://github.com/Ahcast/Bastion/actions/runs/38197746643), [documentación 38197746694](https://github.com/Ahcast/Bastion/actions/runs/38197746694) y [rama única 38197746683](https://github.com/Ahcast/Bastion/actions/runs/38197746683): success. Go fmt/vet/race MySQL 5.7 y builds server/worker/admin, Angular build/**107 pruebas**, navegador **14/14 en 45.7 s**. Cotización aceptada→Borrador de factura íntegro y vínculos/versiones, snapshots históricos y cálculo mixto, 12 replays, competencia de destinos entre actores, rollback/HTTP/scopes/revocación y recarga/vínculos reales certificados. La prueba de pagos captura respuesta real antes de entregar al navegador, corrigiendo pérdida intermitente del buffer sin simular API.

Primer bloque cerrado; unidad 5 continúa OC→Factura y Facturado/Cerrado según contrato previo publicado en 72b6533719338880d77788db21a9ae3ea0ae8ed7. No crear número/emisión/pago implícitos por convertir. Documentos/anulación/clonación Facturas y aceptación integral continúan después; fiscalidad/despliegue productivos no acreditados.


## 2026-10-12 — Facturas, unidad 5: OC→Factura/Facturado/Cerrado en validación

Contrato previo 72b6533719338880d77788db21a9ae3ea0ae8ed7 y migración 0026: vínculos únicos y versiones OC/Invoice, número fiscal snapshot OC, permisos convert/close/reopen sin grants productivos. Solo OC delivered con todos ítems delivered genera draft íntegro, con cliente/cálculo/costo/gastos históricos y origen exclusivo; no contador ni Facturado por crear borrador. Emisión revisa versión de OC y paridad de cliente/ítems/moneda/finanzas, usa puerto público OC→Invoice en orden de locks con fiscal/counter antes y empresa después, y marca OC invoiced con número/historial/evidencia en la misma TX. Cada propietario escribe sus tablas. Replay reautoriza fuente/acción/view/reauth y no repite efectos.

Cierre explícito invoiced→closed con versión de Invoice, saldo cero, reauth y razón; reapertura explícita closed→invoiced antes de corregir pagos. Ledger source-linked comprueba invariante de OC bajo lock antes de Invoice; closed rechaza mutaciones. Consulta de ledger comunica source_order_closed sin otorgar permisos OC ni exponer su registro. GET normal Invoice no toma lock OC. Puertos de lectura para cierre verifican ledger exacto bajo TX; rollback revierte número/contador/ambos propietarios/historial/evidencia. Sin actualizaciones cruzadas privadas ni cierre automático por cobrar.

REST/OpenAPI/flags, UI española con vínculos y revisión de versión, confirmación de identidad para cierre/reapertura, clave ACK incierto y evidencia ante recarga fallida; ficha bloqueada durante petición. Seis integraciones MySQL nuevas: ciclo completo con void bloqueado/reapertura y replay histórico, rollback conversión/emisión/cierre y 12 replays, estado/versión/paridad/scopes/reauth, contención close vs void por dos humanos, HTTP estricto/CSRF/flag, USD y total cero sin fabricar pago. Seed nuevo usa transiciones reales del propietario OC y retira grants de setup antes del login. Angular build/**114 pruebas** locales aprobados; Playwright compila **15 escenarios**, con ciclo OC→Factura/emisión/pago/cierre/reapertura/void. Go fmt/vet/race/MySQL/builds y navegador completos pendientes; no cerrar unidad 5 ni iniciar documentos hasta CI verde. Fiscalidad/infraestructura productivas siguen pendientes.


## 2026-10-12 — Bloqueo de ejecución y checkpoint de OC→Factura

Código vigente `e45d1caa2f6866e9812e3859d87a1656e9b8d0ca` (corrección de aceptación de OC→Factura). Fuente anterior `77d5a29b1a344fe126290eaf7eaab1a480b2c9f5` / Core 38200295427: frontend success (114 pruebas); Go fmt/vet y seis integraciones nuevas OC/Invoice/ledger/cierre/reapertura pasaron, pero suite global falló en el test antiguo de emisión que consideraba source_order_version obligatorio en factura standalone. Preparación de navegador falló por Config.Reauth=0 del seed. Ambos escenarios corregidos en e45d1caa; source_order_version opcional standalone y obligatorio positivo linked OC, ventana sintética 5 minutos igual al runtime. Sin relajar controles productivos.

En e45d1caa, Core 38200999714, documentación 38200999707 y rama única 38200999709 terminaron sin pasos ejecutados ni logs. Reintento de Core (attempt 2) tampoco inició jobs. No causa de cuenta/cuota confirmada; no afirmar que fue un error de código ni que hubo CI verde. No repetir indefinidamente jobs sin evidencia de recuperación.

Se preparó validación local con Go 1.27.2, MySQL Community 5.7.44 y Chrome for Testing 156.0.8078.4 oficiales. Formato/análisis estático y builds locales server/worker/admin aprobaron. Entorno impide sockets Unix y aísla redes por ejecución; se configuró MySQL por TCP local y DB/API/frontend/pruebas dentro de una ejecución. Intento local de integración falló durante reconstrucción de tablas (1146/1051/1813, quote_email_intents/tablespace); no acredita validación global. Antes de repetir se debe usar un datadir sintético nuevo y comprobar migraciones/reset. El entorno luego devolvió `409 Conflict, environment_offline: Environment is not connected` y no pudo ejecutar ni pwd; no hay pipeline local verde ni navegador completo de este bloque.

Reanudación: sincronizar main, leer contrato/estado, verificar disponibilidad de ejecución y CI. Corregir preparación local solo si se utiliza, levantar MySQL 5.7 aislado con datadir nuevo, ejecutar la suite completa con race/builds, npm ci/test/build y 15 escenarios de navegador contra API/Angular/MySQL reales. Verificar las dos correcciones de fixture en e45d1caa; no omitir pruebas ni sustituir MySQL. Certificar unidad 5 únicamente con todos los checks aprobados. Entonces iniciar unidad 6 (documentos, anulación y clonación) bajo contrato previo y QA visual de PDFs. Gastos/Reportes/Colaboración mantienen el orden. Sin despliegue/migración productivos, cobro bancario/Siafi externo ni fiscalidad real acreditados.


## 2026-10-12 — Regresión local OC→Factura recuperada; navegador bloqueado

Se restauró la copia local desde main c8af2346dc9cb5d5c63eabddb0e7065ef59cd588, verificando hashes de todos los archivos frente al árbol remoto. Código de dominio e45d1caa2f6866e9812e3859d87a1656e9b8d0ca sin cambios. scripts/check_repository.py y git diff --check aprobados. Go 1.27.2: gofmt sin archivos pendientes, go vet ./... y builds de server/worker/admin aprobados, incluidos los tres binarios make build. Suite completa go test -race -count=1 ./... aprobada sobre MySQL Community 5.7.44 y datadir sintético nuevo en /tmp: paquete integration aprobado en 88.345 s, 146 funciones de prueba. Las correcciones de fixture de emisión y ventana de reauth del seed quedaron verificadas; browserseed completó con transiciones propietarias reales. npm ci, build y 114 pruebas Angular (18 archivos) aprobados.

DB/API/Angular/pruebas se ejecutaron dentro de una misma ejecución con MySQL por TCP local y sockets Unix desactivados. El datadir nuevo eliminó el fallo anterior de reconstrucción de tablas; no se modificaron migraciones ni controles de dominio. Preparación del navegador inicialmente falló por permisos del ejecutable auxiliar de Chrome extraído; se corrigió ese permiso en el runtime temporal. Después, Chrome for Testing 156.0.8078.4 tampoco pudo arrancar: process_singleton_posix.cc socket() failed: Operation not permitted. La restricción de sockets Unix del entorno bloquea Chrome antes de interactuar con la aplicación. No hay 15 escenarios aprobados, no se omiten ni se relajan restricciones para declararlos verdes.

Core 38200999714 continúa con backend/frontend/browser failure y cero pasos ejecutados; no causa de cuenta/cuota acreditada. Las verificaciones locales no sustituyen la aceptación de navegador y CI requerida por plan.md. Unidad 5 permanece abierta, unidad 6 sin implementación. Reanudar con un entorno autorizado compatible con Chrome y ejecutar los 15 escenarios completos y CI antes de cerrar conversiones e iniciar documentos/anulación/clonación. No despliegue, migración productiva, envío de correo ni integración bancaria/fiscal real.


## 2026-10-12 05:51 UTC — Reanudación para completar Facturas: CI sigue bloqueado

Solicitud vigente: continuar hasta terminar el módulo, preservando aceptación y orden. HEAD revisado e4fe6110da9101b5f5135d4d76f75dff5ab91332. Se realizó un nuevo reintento acotado de Core 38200999714; attempt 3 terminó failure: browser 114681268088, backend 114681268274 y frontend 114681268309, todos sin pasos ejecutados. Consulta independiente de steps del backend devolvió lista vacía. No hay evidencia de recuperación ni motivo de cuenta/runner/cuota disponible mediante el conector; no atribuir una causa sin verificar. No seguir disparando reintentos idénticos.

La limpieza automática del espacio temporal retiró el checkout y los runtimes locales; los archivos y evidencia permanecen preservados en main. No se repitieron ni se inventaron resultados locales. La aceptación anterior sigue siendo Go/race/MySQL 5.7/builds y Angular 114 aprobados; navegador bloqueado antes de arrancar Chrome por sockets Unix. Para desbloquear la aceptación hace falta identificar el mensaje de fallo que GitHub Actions presenta antes de iniciar los jobs y recuperar ejecución de runners, o disponer de un entorno de pruebas autorizado compatible con Chrome, conservando la exigencia de CI del plan. Documentos, anulación, clonación y aceptación integral siguen pendientes; no declarar Facturas terminado ni iniciar Gastos.


## 2026-10-12 — Facturas, unidad 5 conversiones certificada

Código e45d1caa2f6866e9812e3859d87a1656e9b8d0ca: Core checks 38200999714 attempt 5 aprobado. Backend: gofmt/vet/race con MySQL 5.7 y make build server/worker/admin aprobados; Angular build/114 pruebas; navegador 15/15 en 44.6 s. Documentación del checkpoint 696cff45: 38209907963 reintento aprobado; rama única 38209907982 aprobada. GitHub Actions volvió a ejecutar los jobs después de la recuperación de cuenta informada por el usuario. Unidad 5 cerrada: Quote→Factura y OC→Factura, emisión/Facturado, cobro/cierre/reapertura/void, scopes/CSRF/reauth, versiones, idempotencia, rollback y concurrencia certificados. Ninguna integración fiscal/bancaria productiva acreditada. Continúa unidad 6 documentos, anulación y clonación; unidad 7 aceptación integral antes de Gastos.

## 2026-10-12 — Facturas, unidad 6: documentos certificados

Código 370ba44fa67a001cd3bac94775a4a602740840fc: Core checks 38259786051, documentación 38259786115 y rama única 38259786156 aprobados. Go fmt/vet/race con MySQL 5.7 y builds; Angular build y pruebas; navegador 15/15 con generación real de factura PDF, nota de entrega y recibos activo/anulado. La prueba de persistencia usa columnas reales `document_versions.number` y `size_bytes`. Proyección pública, snapshots, almacenamiento inmutable, reintentos, revocación, concurrencia, scope y descarga por Core están cubiertos. Muestras sintéticas A4 de los tres tipos fueron renderizadas y revisadas visualmente; se corrigió encabezado que excedía el margen y el relleno del rango fiscal.

Bloque documentos cerrado para desarrollo sintético. Siguen anulación y clonación, luego aceptación integral de Facturas. No hay validación fiscal productiva, S3 real ni despliegue acreditados.

## 2026-10-12 — Facturas, unidad 6: anulación conservadora certificada

Código 7badf26db671f8d5804596f2e21300718224ca04: Core checks 38261734730 (reintento de browser) aprobado; Go fmt/vet/race MySQL 5.7 y builds, Angular build/123 pruebas, navegador 16/16. La primera ejecución de browser tuvo 15/16: falló un campo de correo de Cotizaciones que se vació durante refresco; el recorrido de anulación pasó. El reintento completo de browser pasó sin cambiar código.

Factura emitida independiente o derivada de Cotización, sin filas de pago, documentos generados ni OC vinculada, puede anularse con permiso, reauth, motivo y declaración explícita de original/copia. Conserva número, SAR, empresa, montos y contador; PDF posterior se marca «Factura anulada». Se bloquean edición, cobro y nota/recibo. Casos con pagos, documentos previos y OC vinculada quedan fuera de este bloque hasta un procedimiento coordinado; validación fiscal productiva pendiente. Continúa clonación y luego aceptación integral de Facturas.

## 2026-10-12 — Facturas, unidad 6: clonación en validación

Contrato previo `docs/contracts/invoice-cloning.md` y migración 0029: POST estricto/idempotente crea un borrador independiente desde draft/issued, preserva cálculo y snapshot comercial histórico, fecha/notas explícitas y vínculo fuente/version; no copia número/SAR, emisión, origen Quote/OC, pagos ni documentos. Autoriza fuente/destino bajo transacción, rechaza anuladas y divergencia histórica. UI española ofrece abrir el borrador y recupera intención ante ACK incierto. Pruebas de integración cubren emisión previa, aislamiento, replay, versión obsoleta, anulada, borrador y dos destinos concurrentes; Angular 125 pruebas y build local aprobados. CI Go/race/MySQL 5.7 y navegador integral pendientes; no cerrar bloque ni módulo hasta aprobación de unidad 7.

## 2026-10-12 — Facturas, unidad 6 clonación certificada; aceptación integral en validación

Código 13787513bbdc65abfd6c09f0d9d7d4127d653646: Core checks 38263606889 aprobó Go fmt/vet/race MySQL 5.7 y builds, Angular build/125 pruebas y navegador completo con clonación de factura emitida y regreso a anulación; documentación 38263606907 y rama única 38263606951 aprobaron. Se amplió `invoice_cloning_test.go` para comprobar rollback por fallo de evidencia y revocación de permiso de creación incluso durante replay. Contrato de aceptación integral en `docs/contracts/invoice-acceptance.md`; requiere nueva regresión completa verde antes de cerrar el módulo. Los límites productivos permanecen explícitos.

## 2026-10-12 — Facturas/pagos terminado para desarrollo sintético

Código `65e8ff1d2966c144c630f3cf4404464b85b16c09`: [Core checks 38264462959](https://github.com/Ahcast/Bastion/actions/runs/38264462959), [documentación 38264462895](https://github.com/Ahcast/Bastion/actions/runs/38264462895) y [rama única 38264462888](https://github.com/Ahcast/Bastion/actions/runs/38264462888) aprobados. Suite Go con race/MySQL 5.7 y builds, Angular build/125 pruebas y navegador 16/16. Siete unidades y aceptación integral completas según `docs/contracts/invoice-acceptance.md`: finanzas/borradores/emisión/pagos/conversiones/documentos/anulación/clonación; reauth/CSRF/scopes/versiones/replay/concurrencia/rollback. Clonación conserva snapshot comercial y aísla serie/SAR/pagos/documentos, con prueba de rollback y revocación en replay. Siguiente módulo en orden: Gastos, desde contrato previo. Sin despliegue ni validación fiscal o bancaria real; anulación con pagos, documentos previos u OC vinculada requiere procedimiento coordinado antes de habilitarla.

## 2026-10-12 — Gastos, unidad 1 registro operativo en validación

Contrato previo Gastos publicado; migración 0030 expenses y permisos sin grants productivos. Cálculo canónico ISV 15% decimal/half-up sin FX; CRUD/snapshot proveedor/versionado/scopes/equipo, papelera/restauración auditada con reauth, intención idempotente y eventos atómicos. Filtros/totales de HNL/USD separados y duplicados informativos visibles al actor. Angular en menú aprobado, cálculo servido por Go, ACK incierto y resultado confirmado aun si falla recarga. Tres pruebas puras, seis integraciones nuevas MySQL (registro/totales, supplier histórico/duplicados/scopes, 12 replays/8 versiones/rollback/revocación, papelera/reauth, HTTP/CSRF/DTO, equipos/transferencia/replay); módulo/flag compuesto probado. Local: gofmt/vet/pruebas puras y compilación de integraciones, Angular build/131 pruebas, Playwright lista 17 escenarios. MySQL/race y navegador reales pendientes de CI; no cerrar unidad ni iniciar vínculos/documentos antes de verde. Ninguna migración productiva, OCR, pago, Siafi ni validación fiscal real.

## 2026-10-12 — Gastos, unidad 1 certificada

Código 3b9b4d35ab10d1e71ed52b486b4b6566ba2ca32b: Core checks 38270786714, documentación 38270786847 y rama única 38270786697 aprobados. Go fmt/vet/race MySQL 5.7 y builds; Angular build/131 pruebas; navegador 17/17. Dos fallos de aceptación previos corregidos: etiquetas explícitas de select y espera por identidad antes de editar ficha; prueba verifica notas antes/después de guardar/restaurar, sin simular API ni relajar seguridad. Registro/cálculo/snapshot/proveedor/scopes/equipos/totales/duplicados/papelera/versiones/replays/concurrencia/rollback/reauth certificados. Continúa unidad 2 referencias/comprobantes bajo contrato previo, luego aceptación integral. No implementación de Reportes ni activación de OCR/integración fiscal/infraestructura productiva.

## 2026-10-12 — Gastos, unidad 2 cierre de integridad en validación

Main d3fcc52b4b17b4ed142f94617f91ee756a3a246b / Core 38295935693 aprobó implementación de referencias/comprobantes (puertos propietarios, doce replays, rollback/HTTP, Angular y navegador 18). La revisión contractual encontró falta de permiso view en writes/replays de vínculos, coordinación del padre mientras el panel realiza HTTP y papelera/restauración de comprobantes. Se corrigen y se amplía cobertura: tres integraciones de view revocado, metadata paginada por scope/papelera/restauración y carrera real entre dos actores. Core alinea locks actor→propietario→documento y revalida binding; la política Expenses bloquea propietario en writes. Metadata se pagina por scope en SQL, sin cargar todos los archivos.

UI bloquea padre, descarta confirmación tardía, mantiene cuerpo/clave de referencia ante ACK incierto, conserva etapas uploaded/verified, permite vínculo existente y papelera de comprobantes. Test browser Unit 2 usa identidad propia `expense-links@example.test`, sin watcher que cambie correos durante sesión; Core mantiene anti-replay TOTP. OpenAPI/contrato/decisiones actualizados. Local gofmt/vet/pruebas puras y compilación de integraciones, Angular build/142 pruebas, lista Playwright 18 aprobados. MySQL/race/regresión global/18 browser de esta revisión pendientes; unidad 2 y aceptación integral siguen abiertas. Sin despliegue ni aprobación fiscal/productiva/OCR.

## 2026-10-12 — Gastos, unidad 2 certificada; aceptación integral en validación

Código d99b6f670efeb248b7d790eb0147d149b669831d: Core checks 38298830116 attempt 2, documentación 38298830209 y rama única 38298829954 aprobados. Go fmt/vet/race MySQL 5.7 y builds; Angular build/142 pruebas; navegador 18/18. Primer browser 17/18: timeout de 30 s en el recorrido antiguo de aprobaciones Quotes, sin causa concluyente en contexto; Gastos y demás recorridos pasaron. Reintento completo de browser pasó sin cambiar código ni controles.

Permiso view adicional en vínculos/replays/política Core, paginación por scope SQL, papelera/restauración de comprobantes y carrera de dos actores certificados. Padre bloqueado, intención de referencia y etapas de carga recuperables, respuesta tardía descartada y actor browser separado sin watcher de correo. Continúa unidad 3 bajo contrato `expense-acceptance.md`: binding con ID canónico, índice documental, consulta opcional según permisos y regresión completa. No iniciar Reportes antes de cerrar esta aceptación.

Aceptación integral de Gastos preparada: contrato/matriz `docs/contracts/expense-acceptance.md`, migración aditiva 0032 índice documental, binding owner con ID canónico, prueba de recuperación por ID alternativo/canónico y UI sin solicitud documental cuando falta core.documents.read. Gofmt/vet, pruebas puras/compilación de integraciones y Angular build/143 pruebas locales aprobados. Regresión completa Go/race/MySQL/builds/18 browser pendiente sobre esta revisión antes de cerrar Gastos y avanzar a Reportes.

La primera CI de aceptación integral e00ec688 / Core 38300195778 no ejecutó pruebas backend: falló pull de contenedor MySQL en public.ecr.aws con `toomanyrequests: Data limit exceeded`. Angular build/143 aprobó, navegador en curso. Se verificó HTTP 200 y digest OCI idéntico de MySQL 5.7.44 en Docker Hub oficial y se cambia el origen de descarga de CI a docker.io con el mismo hash fijado. Requiere nueva CI verde; no confundir bloqueo del registro con error de dominio ni declarar aceptación antes de ejecutar MySQL.

## 2026-10-12 — Gastos terminado: unidad 3 y aceptación integral certificadas

Código f3dd6121df2695cf89d9eb89c40dc28644f7a2c5: Core 38300556293, documentación 38300556205 y rama única 38300556380 aprobados. Go fmt/vet/race contra MySQL 5.7 y builds server/worker/admin; Angular build/143 pruebas; navegador 18/18. El cambio de registro de imagen conservó el digest OCI oficial y recuperó la descarga; migración 0032, binding/consulta por ID canónico y operación manual sin permisos de archivo verificados junto a regresión global.

Unidades 1–3 cerradas para desarrollo sintético según expense-acceptance.md. Referencias documentales y comprobantes no modifican importes, saldos ni costos de propietarios. Permisos view/acción y reauth por registro, replay/revocación/rollback/carrera entre actores, paginación SQL por scope, papelera/restauración y etapas/ACK UI certificados. Checkpoint compacto vigente reemplaza encabezados activos antiguos; historial preservado en status/Git. Siguiente módulo: Reportes, contrato previo y proyección común con Inicio antes de exportación Excel. Sin despliegue, migración productiva, S3/Bluehost real, validación fiscal, OCR ni automatización IA acreditados.

## 2026-10-12 — Reportes unidad 1 implementada, certificación pendiente

Contrato previo `contracts/reports.md`, OpenAPI y migración 0033 (permisos sin grants). Tres proyecciones de lectura SQL con scopes Reportes/fuente intersectados, DECIMAL como cadenas, monedas separadas, fechas DATE inclusivas, exclusión de canceladas/papelera y snapshot REPEATABLE READ de totales/página. Inicio consume misma función y omite fuentes denegadas/deshabilitadas. Angular con filtros/estados/páginas, limpieza ante error y respuestas tardías. Sin modificar fuentes ni multiplicar gastos por referencias.

Local: Go gofmt/vet/test (integración MySQL omitida sin DSN), Angular npm ci/build y 147 pruebas, paridad documental. Pruebas MySQL nuevas: intersección all/self/team/revocación, fechas/exclusiones, importes grandes/HNL/USD, página e Inicio, lectura concurrente coherente y HTTP. Escenario navegador contra API/Angular/MySQL agregado. CI real pendiente antes de Excel.

## Reportes unidad 1 certificada — 2026-10-12

HEAD `6e615338d0e5310c33dda05682f4bdfa2776186f`, Core checks [38309240350](https://github.com/Ahcast/Bastion/actions/runs/38309240350): success completo. Go fmt/vet/race/MySQL5.7 y builds; Angular build/148 pruebas; navegador 19/19. Documentación/rama única aprobadas. Fallos intermedios: selector sin etiqueta inequívoca corregido con aria-label y prueba DOM; OC esperaba render antes del ACK de restore, escenario ahora espera respuesta real; dos escenarios fiscales reutilizaban MFA dentro del mismo paso, browserseed usa identidad independiente sin cambiar seguridad productiva.

Continúa unidad 2 según `contracts/reports-export.md`: XLSX acotado de Go estándar, streaming autenticado Core, scopes/reauth/CSRF, texto monetario exacto, fórmulas auxiliares cents con cota SUM ABS SQL y auditoría transaccional.

Reportes unidad 2 implementada, certificación pendiente: Core Download bounded mantiene middleware; POST XLSX strict DTO, CSRF/humano/reauth/scopes de tres permisos, <=10000 filas/24MiB XML/dos exports por proceso, auditoría antes de bytes. Libro determinista seguro, importes/IDs inlineStr exactos y fórmulas cents acotadas por SUM ABS SQL. UI filtros confirmados/Blob/errores/late responses/reauth; categorías españolas completas. Pruebas Go ZIP/XML/precisión/inyección/tamaño/cancelación, MySQL scope/reauth/revocación/límite/audit rollback y HTTP stream agregadas. Local gofmt/vet/test/builds (SQL omitido sin DSN), Angular build/152 pruebas y lector openpyxl independiente aprobados. Navegador extendido con reauth y descarga/inspección XLSX; CI real pendiente antes del cierre.

## 2026-10-12 — Reportes unidad 2 y aceptación integral cerradas

Código `84fcc123e1ea2b10e11b0d0c78427a90f41ea553`, [Core checks 38310992510](https://github.com/Ahcast/Bastion/actions/runs/38310992510), [documentación 38310992497](https://github.com/Ahcast/Bastion/actions/runs/38310992497) y [rama única 38310992521](https://github.com/Ahcast/Bastion/actions/runs/38310992521): success. Go fmt/vet/race/MySQL5.7 y builds server/worker/admin; Angular build/152 pruebas; navegador 19/19 con reauth/descarga/ZIP real. Lector openpyxl local independiente verificó hojas, filtros/panel congelado, IDs/montos texto, fórmula/control de cache. No nueva dependencia runtime.

Aceptación integral (unidad 3): matriz revisada contra pruebas existentes de ambas unidades, sin cambios adicionales de código ni repetición innecesaria de tests aprobados. Tests MySQL export real verificaron scope export self independiente de fuentes/view, reauth/revocación, utilidad negativa/precisión grande, 128 filas aunque página sea 1, rechazo de 10001 filas, rollback de auditoría y HTTP stream/strict DTO/CSRF. Go renderer verificó ZIP/XML/determinismo/texto protegido/inyección/cents/monedas/tamaño/cancelación. Primer bloque cubre all/self/team/membresía, fechas/exclusiones y snapshot concurrente, Inicio idéntico y flags. UI conserva filtros confirmados y limpia error/ignora respuesta tardía.

Reportes canónicos cerrado; README/plan/checkpoint/runbook actualizados. Siguiente Colaboración, con contrato previo de carpetas/organización/comentarios y reutilización Core.Files/políticas propietarias. Ampliaciones analíticas de propietarios aún incompletos/RFQ/IA y gates de infraestructura/producción siguen pendientes; no se fabrican estados/precios competitivos ni se suman gastos documentales como inversión nuevamente.


## 2026-10-12 — Colaboración: revisión de aceptación integral

El código vigente ya contiene U1–U4 (0034–0037): biblioteca/conversación/tareas/vínculos y bandeja Workflow. El checkpoint anterior sólo registraba Reportes y quedó desactualizado. `4211f5e`, Core 38331234661: backend fmt/vet/race/MySQL5.7/builds y Angular aprobados; browser 18/20 falló selector Asignar a y destinatario Para. Corrección UI `43418a7` con etiqueta explícita/estado reactivo, build y 163 unitarias locales aprobados; CI 38332910384 en validación.

Revisión de tareas: permiso view vigente en tareas personales/escrituras/replays; validación de estado dentro de efecto idempotente permite recuperar ACK de completar/reabrir después del cambio. Cursor de asignados cerrado antes de autorizar candidatos. Integración ampliada con replays, revocación de view y pool de una conexión; requiere MySQL5.7/CI antes de certificar. Contrato U3/U4 detallado junto a implementación; no nueva migración ni grants. Colaboración permanece activa hasta aceptación integral; no producción ni integraciones externas acreditadas.


## 2026-10-12 — Colaboración: edición y recuperación U3/U4

Corrección UI `43418a7`, Core 38332910384: backend Go fmt/vet/race/MySQL5.7/builds, Angular build/163 pruebas y navegador 20/20 aprobados; documentación 38332910388/rama única 38332910353 verdes. Corrige los dos fallos de browser previos sin omitir escenarios.

Continuación de aceptación: UI de tareas ahora edita/reasigna/vencimiento por PATCH versionado, pagina y conserva intención ante ACK incierto; éxito con recarga fallida conserva confirmación y bloquea escrituras hasta actualizar. Vínculos tienen bloqueo/recuperación equivalente, descarte de respuestas tardías y limpieza tras denegación. Bandeja de aprobaciones pagina y abre el ID propietario correcto. Browser agrega edición de tarea real. Angular build/169 pruebas locales aprobados.

Backend evita bloquear usuario asignado mediante JOIN al bloquear tarea; prueba real de dos actores (editar vs completar), rollback por evento fallido, doce creates concurrentes y aislamiento de terceros añadida. Formato/vet/pruebas locales aprobados; integración MySQL omitida localmente por ausencia de DSN. Revisión completa nueva requiere CI MySQL5.7/race/20 browser antes de cerrar aceptación. No iniciar fase siguiente ni acreditar producción por estos resultados.


Colaboración U4, revisión Core.Files: GET/PUT vínculos ahora llama DocumentMetadata para revalidar permiso Core, policy propietaria, binding y papelera antes de exponer/modificar/replay. Integración prueba revocación de core.documents.read y replay previo, versión sin cambio tras rechazo, y papelera por TrashEntry real seguida de lectura/escritura denegadas. No cambia bytes ni requiere grants nuevos. Requiere CI de esta revisión antes de aceptación.


CI 38333700732 sobre 2ec3c03 aprobó Angular/169 y navegador20/20, pero la prueba nueva de dos actores detectó deadlock MySQL1213. Quitar JOIN no fue suficiente: UPDATE también requiere locks FK de participantes. Se corrige adquisición de usuarios creador/asignado/actor/nuevo asignado en orden numérico antes de autorización/fila de tarea, con revalidación de participantes al bloquear tarea. Creates cruzados usan mismo orden. No se oculta el deadlock ni se cambia la expectativa de éxito+conflicto; requiere nueva CI.


La revisión de log completo detectó además timeout global de paquete a los 360.027s en CI 38333700732, al iniciar TestPurchaseOrderHTTPStrictContract (0s), después de progresar por la suite. El límite 6m no alcanza de forma estable para la suite creciente de ~190 integraciones MySQL con reconstrucción de esquema. Se aumenta a 12m, conservando todos los tests/race y fallos explícitos. Este ajuste no elimina la prueba de deadlock ni su corrección de orden de locks.


## 2026-10-12 — Colaboración: bloque de correcciones U3/U4 certificado

Código `3af770964ebcea2cd87202211b8059ece1b1952a`, Core checks [38335121065](https://github.com/Ahcast/Bastion/actions/runs/38335121065), documentación [38335121107](https://github.com/Ahcast/Bastion/actions/runs/38335121107) y rama única [38335121100](https://github.com/Ahcast/Bastion/actions/runs/38335121100): success. Backend formato/vet/race/MySQL5.7 y builds; Angular build/169 pruebas; navegador 20/20. Edición real de tareas, cierres/reapertura y vínculos cubiertos en browser. Integración pasó doce creates, rollback por evento, carrera de dos actores con éxito+conflicto y asignaciones cruzadas; sorted participant locks eliminó deadlock observado. Revocación Core.Files/replay/papelera y view en tareas personales aprobados.

Este bloque está certificado; módulo sigue activo para revisión/ampliación de aceptación transversal de conversación concurrente y recursos/Workflow con propietarios reales. No comenzar API/MCP IA ni declarar cierre integral antes de revisar esos casos de la matriz. No despliegue, migración productiva, S3/scanner/SMTP/Percona/DNS reales acreditados.
