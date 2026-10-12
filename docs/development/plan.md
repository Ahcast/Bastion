# Plan ejecutable del proyecto

Core de plataforma y base implementados: ver [checkpoint operativo](../ESTADO_DESARROLLO.md), [estado histórico](status.md) y [runbook](core-runbook.md). Los criterios de infraestructura externa permanecen pendientes aunque las pruebas locales/CI pasen. Clientes, Proveedores, Procesos/receptor crawler y Cotizaciones están cerrados para su alcance aprobado y certificados en CI. Órdenes de Compra de clientes tiene seis unidades y aceptación integral cerradas en CI. **Facturas/pagos** completó siete unidades y aceptación integral de desarrollo sintético en CI; **Gastos** completó tres unidades y aceptación integral en CI; **Reportes** cerró consultas canónicas/Inicio, Excel privado y aceptación integral; el siguiente módulo es **Colaboración**. Consultar el checkpoint para evidencia vigente y continuar una unidad por vez.

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

## Órdenes de Compra de clientes — desarrollo por unidades

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


## 2026-10-11 — Facturas, unidad 3 emisión certificada; continúa unidad 4

Código certificado `fa8670bcf68c11a07a2ed9ff7aedc7d23a4bc890`: [Core checks 38183414256](https://github.com/Ahcast/Bastion/actions/runs/38183414256), [documentación 38183414230](https://github.com/Ahcast/Bastion/actions/runs/38183414230) y [rama única 38183414289](https://github.com/Ahcast/Bastion/actions/runs/38183414289): success. Go fmt/vet/race con MySQL 5.7 y builds server/worker/admin aprobados; Angular build y **97 pruebas**, navegador **13/13 en 44.4 s**. Emisión conserva snapshots fiscal/empresa, valida tres versiones/cálculo/vigencia/moneda/fecha Honduras, asigna correlativo con evidencia atómica y protege replays/scopes/reauth/rollback/concurrencia. Recorrido sintético verifica RTN persistido antes de revisión y CAI histórico tras modificar configuración.

Unidad 3 cerrada para desarrollo sintético. Continúa unidad 4 bajo contrato previo y OpenAPI de payments: cobros auditables, anulación sin borrado y proyección comercial exacta. Unidad 4 aún sin implementación; conversiones, documentos/anulación/clonación y aceptación integral siguen en orden. Ningún recibo/pago bancario/Siafi externo ni validación fiscal/despliegue productivo acreditados.


## 2026-10-11 — Facturas, unidad 4 pagos en validación

Contrato previo 2c733b22a84ba3436ed88553f6c1a7dac7bc985a y migración 0024 payments; permisos payment/payment.void sin grants productivos. Ledger normalizado inmutable/anulable con versión, fecha/método/moneda/importes/referencia/razón y receipt_number NULL. Servicio humano view/action con reauth/CSRF/versión/intención; factura bajo lock, saldo recalculado desde filas activas, no sobrepago ni FX/retenciones. Register/void cambian exclusivamente proyecciones/versiones con audit/event y evidencia atómica; rollback y replay reautorizados. Estado documental issued se conserva y commercial_status deriva Pendiente/Parcialmente pagada/Pagada/Vencida en reloj Honduras; filtro/listado/ficha y ledger sin agregados entre monedas.

UI consulta explícitamente ledger vigente, registra/anula con razón y reauth, conserva clave ACK incierto y resultado confirmado si falla recarga, bloquea padre durante HTTP y nuevas mutaciones hasta recuperar versión. Seis integraciones MySQL nuevas cubren persistencia/anulación/cuatro métodos, total cero/importes >2^53, versiones/invalidaciones/moneda/fechas, 12 replays, ocho cobros en contención por dos humanos, rollback register/void, HTTP/CSRF/scopes/reauth/revocación. Prueba pura de vencimiento y prioridades; seis Angular nuevas. Build local aprobado; 102 pruebas locales previas al último caso de padre (objetivo 103) y suite de navegador ampliada con abono/anulación/cobro completo y contador/snapshots inmutables. Go fmt/vet/race/MySQL/builds, Angular y navegador completos pendientes; no cerrar pagos ni empezar conversiones hasta CI verde. Sin recibos/PDF/Siafi bancario ni validación fiscal productiva.


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


## 2026-10-12 — Facturas, unidad 5 conversiones certificada

Código e45d1caa2f6866e9812e3859d87a1656e9b8d0ca: Core checks 38200999714 attempt 5 aprobado. Backend: gofmt/vet/race con MySQL 5.7 y make build server/worker/admin aprobados; Angular build/114 pruebas; navegador 15/15 en 44.6 s. Documentación del checkpoint 696cff45: 38209907963 reintento aprobado; rama única 38209907982 aprobada. GitHub Actions volvió a ejecutar los jobs después de la recuperación de cuenta informada por el usuario. Unidad 5 cerrada: Quote→Factura y OC→Factura, emisión/Facturado, cobro/cierre/reapertura/void, scopes/CSRF/reauth, versiones, idempotencia, rollback y concurrencia certificados. Ninguna integración fiscal/bancaria productiva acreditada. Continúa unidad 6 documentos, anulación y clonación; unidad 7 aceptación integral antes de Gastos.

## 2026-10-12 — Facturas/pagos: siete unidades y aceptación integral certificadas

Código `65e8ff1d2966c144c630f3cf4404464b85b16c09`: [Core checks 38264462959](https://github.com/Ahcast/Bastion/actions/runs/38264462959), [documentación 38264462895](https://github.com/Ahcast/Bastion/actions/runs/38264462895) y [rama única 38264462888](https://github.com/Ahcast/Bastion/actions/runs/38264462888) aprobados. Go fmt/vet/race sobre MySQL 5.7, compilaciones server/worker/admin, Angular build/125 pruebas y navegador 16/16 contra API/base reales. El contrato `docs/contracts/invoice-acceptance.md` mapea finanzas, borradores, emisión fiscal interna, pagos, conversiones Quote/OC, documentos PDF, anulación y clonación. Última ampliación verifica rollback de clonación por fallo de evento y revocación de permiso en replay; la regresión completa pasó. Unidad 6 y unidad 7 cerradas para alcance de desarrollo sintético. Siguiente módulo: **Gastos**, empezando por contrato propietario y sus dependencias.

No implica despliegue/migración productiva, aprobación fiscal de configuración real, procesamiento bancario/Siafi, S3 externo, correo real ni resolución legal de anulación con pagos/documentos u OC vinculada. Esos casos siguen bloqueados explícitamente.

## 2026-10-12 — Gastos: secuencia propietaria

Contrato previo `docs/contracts/expenses.md`: unidad 1 registro operativo/cálculo/proveedor/categorías/filtros/totales/papelera/duplicados con migración 0030, REST/UI/seguridad; unidad 2 vínculos comerciales y comprobantes privados mediante propietarios; unidad 3 aceptación integral. Una unidad se certifica antes de implementar la siguiente. Reportes/Colaboración conservan orden y usan proyecciones autorizadas; gastos no reescriben costos ni pagos de otros agregados. IA/OCR se incorpora en su fase, no bloquea operación manual.

## 2026-10-12 — Gastos: tres unidades y aceptación integral certificadas

Código `f3dd6121df2695cf89d9eb89c40dc28644f7a2c5`: [Core checks 38300556293](https://github.com/Ahcast/Bastion/actions/runs/38300556293), [documentación 38300556205](https://github.com/Ahcast/Bastion/actions/runs/38300556205) y [rama única 38300556380](https://github.com/Ahcast/Bastion/actions/runs/38300556380): success. Go fmt/vet/race MySQL 5.7 y builds, Angular build/143 pruebas y navegador 18/18. Matriz en `docs/contracts/expense-acceptance.md`; runbook en `docs/development/expenses-runbook.md`. Registro exacto, proveedor/categorías/filtros/totales, papelera, duplicados visibles, referencias propietarias, comprobantes privados, reauth/scopes/view/replay/rollback/concurrencia y recuperación UI certificados. Metadatos paginados en SQL, índice documental y binding canónico verificados. CI se recuperó del límite de datos ECR usando Docker Hub oficial con digest idéntico; no se redujeron pruebas ni versión MySQL.

Gastos cerrado para desarrollo sintético. **Siguiente: Reportes**, desde contrato previo de filas/totales/filtros y permisos compartidos con Inicio, después Excel y aceptación integral. Referencias de gasto no ajustan automáticamente inversión ni permiten duplicarla en métricas. OCR/IA y propietarios aún inexistentes se integran en su fase; producción/infraestructura/fiscalidad reales siguen pendientes.

## 2026-10-12 — Reportes canónicos y Excel cerrados

Tres unidades completadas: consultas/Inicio, Excel privado y aceptación integral. Contratos reports.md/reports-export.md/reports-acceptance.md y OpenAPI; permisos 0033 sin grants. Código `84fcc123e1ea2b10e11b0d0c78427a90f41ea553`, Core checks [38310992510](https://github.com/Ahcast/Bastion/actions/runs/38310992510) aprobado: Go/race/MySQL5.7/builds, Angular build/152 pruebas, navegador 19/19. Evidencia y siguiente paso Colaboración en checkpoint. Las categorías analíticas futuras de RFQ/IA y métricas sin estados/evidencia propietaria siguen en sus fases; no se sustituyen por inferencias ni se acredita su integración.
