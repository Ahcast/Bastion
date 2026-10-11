# Plan ejecutable del proyecto

Core de plataforma y base implementados: ver [checkpoint operativo](../ESTADO_DESARROLLO.md), [estado histórico](status.md) y [runbook](core-runbook.md). Los criterios de infraestructura externa permanecen pendientes aunque las pruebas locales/CI pasen. Clientes, Proveedores, Procesos/receptor crawler y Cotizaciones están cerrados para su alcance aprobado y certificados en CI. La unidad activa siguiente es **Órdenes de Compra de clientes**, desarrollada de forma incremental desde su contrato propietario. Consultar el checkpoint para evidencia vigente y continuar una unidad por vez.

El agente implementa cada fase hasta su aceptación, actualiza el checkpoint/estado y continúa. No sustituir implementación por plan nuevo. Las fases pueden desarrollarse con datos sintéticos y MySQL 5.7 local/CI de prueba; los bloqueos de infraestructura se verifican antes de producción, sin migrar la BD a AWS.

| Fase | Trabajo y entregables | Criterio de aceptación |
| --- | --- | --- |
| 0. Base | Inicializar Go/Angular, lockfiles, configuración/env.example, comandos de desarrollo, SQL migrations runner, CI; inventario del crawler | Build reproducible; pruebas arrancan; secretos ausentes; versión de esquema y health/readiness definidos |
| 1. Core | Usuarios/invitaciones, sesiones, TOTP/recovery, RBAC configurable, settings/SMTP, auditoría, API/errores, cliente Angular, navegación y Configuración | Login→MFA→me→logout; sesión parcial denegada; permisos por registro; revocación; CORS/CSRF real en navegador; SMTP solo fake local hasta autorización |
| 2. Servicios compartidos | Archivos privados/metadatos/S3, outbox/jobs/retries, idempotencia, contexto/correlación, límites y registry módulos | Doble solicitud/concurrencia no duplica; commit+outbox atómico; crash/retry recuperable; archivo no publicado antes de verificar; acceso ajeno denegado |
| 3. Datos base y Procesos | Clientes, proveedores, procesos/items/versiones, receptor crawler/ACK, filtros, Inicio; conservar CLI Python | Pruebas crawler existentes; importar/reimportar lote; observación antigua/parcial no borra; ACK solo tras commit; listados autorizados y cliente snapshot |
| 4. Comercial | Quotes, cálculo decimal/versiones/adjuntos/PDF/correo programado; OC de clientes; Invoices/payments; Expenses | Ejemplos 15/18/exento/exonerado y descuento unitario/general; control de versión; conversiones transaccionales/idempotentes; saldos consistentes; correo incierto no reenvía a ciegas |
| 5. Reportes y colaboración | Reportes/exportaciones, documentos/versiones/folders, colaboración y workflows/aprobaciones configurables | Filtros/permisos idénticos a dominio; totales conciliados; documentos privados; autorización por acción/hash/version/vigencia; sin OCR local |
| 6. API/MCP IA | SDK MCP, autenticación limitada/delegación conforme, tools mínimo, AI tasks/runs/leases y score/research/RFQ/pricing/artifacts | Mismas reglas REST/MCP; lease vencido/resultado tardío rechazado; schema/permissions; prueba cliente local; Work real solo si disponible, no invocar Plus como API |
| 7. IA y UI | Pipeline/skills versionadas, progresos por ítem, RAG S3/MySQL, revisión humana y selección hacia Quotes | No repetir score completado; rezagados recuperables; evidencia/ausentes explícitos; cotización humana no sobrescrita; casos de evaluación sintéticos; umbrales no aprobados permanecen configurables y automatización off |
| 8. Operación | Backup/restore, deploy scripts frontend/backend, observabilidad, incidentes, límites y smoke tests | Restore probado; rollback código/esquema documentado; memoria crawler+backend medida; MySQL TLS/DNS/SMTP/S3 validados en entorno autorizado; no claims de producción por tests locales |

## Secuencia obligatoria de módulos

Clientes → Proveedores → Procesos/receptor crawler → Cotizaciones → Órdenes de Compra de clientes → Facturas/pagos → Gastos → Reportes → Colaboración. Workflow/aprobaciones e IA se integran según dependencias, sin entradas adicionales de menú. Antes de avanzar, cerrar pruebas unitarias, MySQL/integridad, autorización, UI/navegador, regresión y CI de la unidad actual; registrar evidencia y límites externos en el checkpoint/status. No escribir el siguiente módulo mientras haya pruebas pendientes/fallidas o reglas comerciales indispensables sin contrato. Las vistas cruzadas se incorporan cuando ambos propietarios existan, sin inventar datos ni declarar integración anticipada.

## Antes de implementar cada módulo

1. Leer Core, modelo, API y spec funcional propietaria.
2. Definir migraciones/índices, entidades/DTO, estados/transiciones, permisos, eventos/jobs y errores. No copiar toda la documentación al código.
3. Implementar servicios y pruebas de invariantes; handlers REST/MCP finos; Angular consume contratos.
4. Verificar casos de fallo reales: duplicados, payload/version/lease inválidos, acceso denegado, caída antes/después del commit.
5. Actualizar OpenAPI, docs solo si cambia contrato, y status/checkpoint con comandos/resultados. Commit en main.

## Definición de terminado

Función completa, error explícito y manejo de fallos; build y pruebas relevantes; migración reproducible; autorización/auditoría; UI española; documentos/snapshots consistentes; estado actualizado. Sin stubs productivos, sin promesas de capacidades de terceros ni tests que solo repitan implementación.

## Bloqueos externos acotados

MySQL remoto TLS o túnel SSH verificado, acceso y soporte Percona; DNS/certificados; S3/IAM; SMTP/IMAP; fiscalidad SAR/series/plantillas reales; Work/Grokbot y skills comerciales. Registrar evidencia ausente. Mantener escenarios locales de prueba y continuar módulos independientes. Feature flags dejan conexiones/envíos externos deshabilitados hasta configurar y validar.

No implementar integración pagada de inferencia como solución automática a un bloqueo Work. RPO/RTO, retención y reglas de scoring/envío se configuran sin inventar valores aprobados.

## Cotizaciones — cierre vigente

Cotizaciones está cerrado para el alcance aprobado en el HEAD `0a98bf756c4e7a3965b7b625473258509fbaa571`: HNL/USD explícitos y separados, sin conversión automática; cálculo exacto, snapshots/versiones, PDF/adjuntos, workflow/aprobación, emisión, correo comercial, estados finales, bulk y registro FX auxiliar. El mismo HEAD aprobó backend con race/MySQL 5.7/builds, Angular, Playwright real, documentación y política de rama única. Consultar `../ESTADO_DESARROLLO.md` para runs y límites externos.

Las conversiones Quotes→OC/Factura se integran con los servicios propietarios cuando esos módulos existan; Quotes no escribe sus tablas directamente.

## Órdenes de Compra de clientes — unidad activa

El maestro y `development/decisions.md` resuelven la nomenclatura: el menú OC representa órdenes **recibidas de clientes**; usar `customer_purchase_orders` / `customer_purchase_order_items` y REST `/purchase-orders`. Las futuras órdenes emitidas a proveedores serán `supplier_purchase_orders` y quedan fuera de este módulo.

`modules/purchase-orders.md` conserva tanto estados legacy como un flujo actual más amplio. No mezclar ambos grafos implícitamente. La primera unidad, ya cerrada, implementa únicamente el agregado íntegro de OC de cliente con estado inicial `Recibida`: cabecera, ítems, cálculo exacto, CRUD/versionado, autorización, idempotencia, auditoría, API y pruebas. Estados/transiciones posteriores se incorporan en una unidad separada después de fijar su contrato.

Fuera de la primera unidad: Cotización→OC, OC→Factura, entrega, clonación, documentos, AI, OC a proveedores y cualquier estado distinto de `Recibida`.

## Unidad 2 cerrada — estados de OC

Contrato congelado en `../contracts/purchase-orders-states.md`: un solo grafo vigente. Transiciones transaccionales, REST, historial, entrega con reauth y Angular certificados en `0fa94796ffee0c248035f94a9166f22c0a955b42`, Core checks `38154939036` aprobado. Consultar checkpoint para evidencia completa.

La Unidad 3 siguiente incorporó conversión Cotización → OC; consultar su cierre y el checkpoint vigente.

## Unidad 3 cerrada — conversión Cotización → OC

Contrato normativo `../contracts/quote-purchase-order-conversion.md`. Código/UI certificados en `541bb101db51b391265b95ca9a8acd0ff0a3ae85`, Core checks `38159858594` aprobado: Go/race/MySQL5.7/builds, Angular build/68 pruebas y navegador 10/10. Evidencia completa en checkpoint.

La Unidad 4 incorporó documentos privados de cabecera/ítems (cierre inferior). Siguiente: clonación de OC y aceptación integral. Facturas permanece después del cierre correspondiente.

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

Backend/UI implementados; pruebas de preservación, revocación/reauth/replay/rollback, evidencia protegida, reserva de número, backfill real MySQL, HTTP/CSRF, UI respuesta confirmada y navegador papelera/restauración/utilidad añadidas. Regresión y CI pendientes; no cerrar módulo ni iniciar Facturas hasta completar esta aceptación. Producción e integraciones externas pendientes.
