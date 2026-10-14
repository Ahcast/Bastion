# Decisiones de consolidación — 9 de octubre de 2026

## 2026-10-13 — Dirección visual Bastion

`docs/ui/BASTION-UI.md` y `docs/ui/tokens.json` son la referencia visual para el frontend. Se integra la base Concreto del kit entregado: barra superior, logo, íconos, fuentes locales y estilos `bs-*`. La migración de las pantallas de negocio al detalle en línea y patrones de indicadores sigue la secuencia de la sección 13 de la guía; este cambio de base no acredita que cada módulo ya haya completado esa migración. Mantener contratos, permisos y flujos vigentes durante la transición.

| Contradicción o redundancia | Resolución vigente |
| --- | --- |
| Backend Bluehost/PHP frente a Go AWS | Go en Lightsail; Angular/BD/correo Bluehost |
| Misma origin frente a frontend/API separados | CORS exacto; cookie API host-only; CSRF entregado en JSON y enviado por Angular explícitamente |
| clients vs customers | customers, customer_id y customer_*_snapshot; REST /customers |
| orders vs OC de clientes | customer_purchase_orders/items; REST /purchase-orders; futuras OC proveedores separadas |
| invoice_payments vs payments | payments vinculados a invoice; no tabla paralela |
| quote_counter específico vs correlativos comunes | document_counters con tipo/serie y bloqueo transaccional |
| ai_tool_calls vs agent_actions | agent_actions para auditoría de llamadas; ai_runs para ejecuciones |
| /ingestion vs API del crawler existente | /api/internal/v1/crawler y ACK existente; sin alias ni 202 IMPORTED |
| README repetidos, maestro extenso y arquitectura v2 alternativa | Una spec por tema; maestro breve con enlaces; eliminados duplicados, carpeta vacía no es requisito |
| OCR/IA local implícita | OCR externo futuro; sin inferencia/OCR en Lightsail |
| CI protected-main/PR frente a rama única | Commits en main, sin ramas/PR; CI y revisión de commits; control detectivo no sustituye restricción preventiva GitHub |
| Aprobación senior global antes de todo código | Implementar por fases con aceptación; bloqueos externos no paralizan módulos independientes |
| Angular 22/TS6 asumidos | Elegir matriz soportada al inicializar y fijar lockfiles; no supuestos incompatibles |
| Roles comerciales/umbrales fiscales/IA no definidos | Configurables; sin automatización ni privilegios implícitos; registrar validación pendiente |

Las reglas de precisión/redondeo/IDs JSON son convenciones de implementación, no cambios a impuestos o precios comerciales. El cálculo conserva descuento absoluto por unidad y distribución proporcional; validar con ejemplos legacy antes de migrar datos.

Las ramas antiguas se conservan como padres históricos del commit de consolidación para no perder sus commits divergentes; sus tecnologías obsoletas no se reintroducen en la spec vigente. La limpieza remota elimina solo refs que ya son ancestros de main, con verificación SHA antes de borrar.

No hubo despliegue productivo, migración real ni envío. Las verificaciones locales se reportan separadas de las de GitHub.

## Implementación Core

Go 1.27.2 y dependencias fijadas; Angular 22.2.2/TypeScript compatible fijados tras build. Argon2id 32 MiB/3 iteraciones/paralelismo 1, máximo dos cálculos por proceso; calibrar en Lightsail. Sesiones simultáneas permitidas dentro de vencimientos, sin imponer una sesión única. Bootstrap técnico protegido, sin conceder permisos financieros. Cookies de desarrollo separadas de producción. Scope all/self/team, global administrativo all. Archivos comerciales sin política de módulo fallan cerrados. Entrega SMTP incierta exige conciliación. MCP inicial bearer técnico limitado, separado de OAuth delegado pendiente. No se amplió el menú ni se implementaron módulos comerciales ficticios.

## 2026-10-09 — Clientes y desarrollo secuencial

Clientes primero, luego Proveedores y demás propietarios según plan. Cada unidad cierra sus pruebas y CI antes de comenzar la siguiente, según instrucción explícita del usuario. Normalización canónica Unicode NFKD/diacríticos/espacios, RTN opcional con 14 dígitos; UNIQUE conserva deduplicación incluso en inactivos. Ficha con versión, contactos/direcciones relacionales y notas append-only; no merge/borrado automático. Scopes all/self/team con propietario inmutable; cambios de equipo exigen all y reauth. RegisterPermissions añade capacidades asignables, no permisos automáticos a administradores.

Hechos comerciales en domain_events, sin transformarlos en trabajos desconocidos; outbox queda para jobs registrados. DocumentPolicies permite a Core combinar permisos del archivo y la entidad sin importar módulos. MCP usa contexto autenticado y schemas explícitos para mantener IDs/versiones como strings. OAuth/Work y servicios externos siguen pendientes. Vista 360° se integra por propietario disponible, sin fabricar actividad futura.

CI usa la imagen oficial MySQL 5.7.44 desde el mirror Docker Official Images de ECR Public, fijada por digest sha256:4bc6bc963e6d8443453676cae56536f4b8156d78bae03c0145cbe47c2aad73bb. Se verificaron manifiesto amd64 y MYSQL_VERSION=5.7.44-1.el7. Docker Hub bloqueó el primer run de Clientes por límite anónimo antes de ejecutar backend/browser. El cambio de registro afecta solo pruebas; no crea MySQL productivo en AWS. Fuente: https://gallery.ecr.aws/docker/library/mysql y manifiestos públicos del registro.

Listado de Clientes devuelve CustomerSummary (ID/nombre/RTN/email/activo/versión), no colecciones vacías que aparenten una ficha completa. GET devuelve el agregado con contactos/direcciones; PATCH requiere todos los campos editables. Las conexiones técnicas de lectura admiten scope team cuando el creador posee el permiso y pertenece a un equipo; cada consulta filtra/reevalúa la pertenencia actual. Se probaron creación limitada y revocación de ese acceso.

La asignación de equipo durante el alta requiere pertenencia actual o clientes.create all. Un permiso self no permite asignar equipos ajenos aunque el creador siga siendo propietario. Prueba de integración reproduce y rechaza esa intención por API de servicio.

## 2026-10-09 — Proveedores, unidad 2

Unidad iniciada tras cerrar Clientes en CI. Catálogo independiente de Customers, con normalización equivalente versión 1 y permisos explícitos all/self/team. Ficha con contactos, etiquetas categorías/marcas, productos/MOQ, condiciones y evaluación manual; historial, notas y observaciones de precio append-only con fuente/fecha. Valores DECIMAL exactos como cadenas, sin cálculos financieros ni redondeo silencioso. No requiere una dependencia nueva: la normalización usa x/text ya fijado y el almacenamiento no realiza aritmética financiera.

Snapshot/LegacySnapshot conservan proveedor autorizado o texto histórico con ID null sin crear fichas automáticamente. price_observations manuales no son cotizaciones oficiales ni sustituyen candidatos/runs de AI. RFQ/correo aprobado, cotizaciones recibidas y métricas calculadas quedan para sus propietarios AI/Workflow/comerciales; las conexiones OC/Gastos se añaden en sus fases. Ningún evento encola trabajos desconocidos.

La interfaz conserva cambios pendientes de ficha al registrar notas/evidencia y el ID/version confirmados si falla recarga posterior a POST. Captura de observaciones y presentación de timestamps usan Honduras UTC−06:00; persistencia UTC. Corrección del DatePipe global para evitar depender de la zona del navegador. Datos de ensayo MySQL en directorio temporal nuevo por ejecución; no restaurar/checkpoint de archivos de un InnoDB activo. Pruebas y límites reales en status.md.

## 2026-10-09 — Procesos y receptor wire 1.0

Se conserva el cliente Python y su entrega de snapshots reintentables. Orden por campo, clocks de catálogo/documentos y evidencia atrasada impiden pérdidas por parcialidad o entrega fuera de orden. Idempotencia durable del lote independiente del token, con autorización vigente antes de replay; cierre por snapshot valida sumas, sin sellar permanentemente el run. Hasta AI, EmitEvent conserva hechos sin crear jobs de tipo desconocido; el outbox AI del contrato conceptual se conectará al consumidor propietario.

Scopes crawler explícitos son las únicas escrituras técnicas añadidas; no autorizar escrituras MCP comerciales. Máximo HTTP 24 MiB/cuatro importaciones concurrentes; exportación máximo explícito 10000 con error y filtros. Observaciones/raw hashes no prueban que los bytes S3 existan; no descargar URLs externas desde el receptor. Campos comerciales/asignaciones y Quotes/AI/Colaboración permanecen en sus fases, sin stubs.

El receptor tiene presupuesto 75 s y deadline de respuesta/proxy 85 s, inferiores al read timeout 90 s del cliente existente. El límite general de 25 s se mantiene para las demás rutas. Evita hacer incompatible el ACK síncrono de 100 registros con la latencia remota MySQL; límites reales siguen por medir. Cambios por versión se insertan en un único statement parametrizado, sin un viaje por documento. El proxy solo amplía tamaño/tiempo en la ruta machine del crawler.

El lote conserva checksum/metadata/ACK; cada observación conserva su cuerpo. Evitar duplicar todo el lote en un único parámetro SQL permite importar cuerpos HTTP mayores que max_allowed_packet sin exigir privilegios/configuración global Bluehost. Complete limita extracción a 64 KiB.

## 10 de octubre de 2026 — Cotizaciones por unidades verificables

Primera unidad de Quotes: borradores/cálculo/conversión, sin declarar completada emisión ni módulo. shopspring/decimal v1.4.0 fijado; descuento general en centavos con cociente/resto exacto y mayor resto, empate por posición. Impuesto por grupo después del descuento, half-up a dos decimales. Costos/margen/notas internos. USD/HNL explícitos sin mezcla/conversión implícita; FX fechado conserva alcance futuro. Flete como ítem con impuesto explícito, sin inventar fiscalidad.

Snapshots de servicios públicos dentro de TX del agregado evitan transacciones anidadas, carreras y lectura/escritura cruzada de tablas maestras. Replays reautorizados conservan snapshot/intención incluso si cambia fuente. Número correlativo transaccional, papelera y versiones durables. Workflow/aprobación/PDF/S3/correo y servicios OC/Factura no se sustituyen con estados libres ni endpoints simulados. No avanzar al siguiente módulo hasta cerrar aceptación completa de Quotes.

## PDF de Cotizaciones (unidad 4b)

Renderer github.com/signintech/gopdf v0.38.1 y golang.org/x/image v0.47.0 fijados en go.mod/go.sum. Font goregular integrado y validación sfnt evita omisiones silenciosas; sin Chrome/headless en Lightsail. Coordenadas PDF usan float64 de la librería, importes siguen siendo cadenas del calculador exacto. Fuentes primarias: https://github.com/signintech/gopdf y https://pkg.go.dev/golang.org/x/image/font/gofont/goregular. Reserva SQL + subida condicional fuera del lock + registro final transaccional permite recuperar fallos sin cola sin consumidor, duplicación ni reescritura del objeto. Render determinista de proyección pública, sin costos/margen/notas. PDF se etiqueta BORRADOR; emisión/aprobación/correo conserva su próxima unidad.

## Estados preliminares de Quotes

Workflow aporta evaluación de grafos; Quotes conserva reglas, persistencia y TX. Implementar costeo/revisión antes de aprobación/emisión sin rutas que simulen esos efectos. cotizaciones.transition es independiente de edición y no se asigna por defecto. Revisar exige ítems, sin inventar umbrales de margen/precio. Guardar ediciones desde costeo/revisión regresa a borrador y mantiene historial; escritura documental bloqueada en revisión con lock del agregado durante políticas transaccionales. PDF permanece BORRADOR hasta implementar aprobación y emisión. Definiciones editables y decisiones/vigencia siguen pendientes.

## 2026-10-10 — Reservas de límite HTTP y búsqueda Unicode

El recorrido de aprobación de dos personas reveló MySQL 1213 durante lecturas HTTP: middleware Limit usaba INSERT IGNORE seguido de SELECT FOR UPDATE sobre un bucket compartido. Sustituido por ON DUPLICATE KEY UPDATE de la misma clave, que reserva bloqueo exclusivo antes de leer/incrementar; sin reintentos automáticos de escrituras. Prueba con 64 llamadas y límite 24 comprueba 24 aceptadas, rechazo explícito de las demás y reinicio de ventana.

Buscar aprobación/Revisión/conexión reveló MySQL 1267 al comparar parámetro Unicode contra quotes.number (ascii_bin). El término y el consecutivo se convierten explícitamente a utf8mb4_unicode_ci para LOCATE, conservando búsqueda literal parametrizada y sin alterar migraciones publicadas. Prueba de búsqueda en número, cliente, expediente e ítem. Diagnóstico API registra códigos numéricos/contexto sin mensajes SQL o datos privados. El navegador no puede cerrar una unidad con estos fallos.

## 2026-10-10 — Emisión PDF separada de entrega

Emitir conserva quotes.status=approved y su versión inmutable, con evidencia issued dentro de quote_pdf_snapshots. Cambiar a sent sin SMTP/entrega autorizada sería falso; agregar un estado comercial distinto del grafo aprobado no es necesario. La reserva no consume aprobación: render/PutImmutable fuera de TX y revalidación/consumo/documento/hecho atómicos al finalizar. Replay confirmado recupera evidencia histórica, pending no elude vigencia ni cambios. Permiso emit específico sin grants automáticos. El test browser usa una composición aislada con store en memoria en tests/browserapp; cmd/server y adapter S3 productivos no incorporan mocks.
# Transporte y programación previos al correo Quotes — 2026-10-10

Ampliar Core.Mail sin dependencias nuevas: sobre explícito To/CC/BCC y adjuntos en memoria, validación estricta y MIME mediante go-mail fijado. El adaptador no resuelve rutas/URLs ni autoriza destinatarios. Cota técnica inicial de 10 MiB de adjuntos y 20 destinatarios documentada; no acredita límites ni conectividad Bluehost. Preservar Recipient para la prueba SMTP existente.

La fecha programada pertenece a la intención durable outbox, no al momento del publicador. EnqueueAt y migración 0011 transportan UTC/microsegundos hasta jobs.available_at; eventos legacy conservan ejecución inmediata. Los servicios técnicos se cierran por separado antes de componer el ledger/autorización/UI comercial de Quotes. No se modifica el significado de emisión ni se activa correo.

## 2026-10-10 — Alcance de cierre Cotizaciones

El usuario escogió explícitamente HNL y USD separados sin conversión automática. FX queda fuera del cierre actual; no se elige una tasa BCH ni se inventa política cambiaria. Conversiones OC/Factura se incorporan mediante propietarios futuros. Se completa flujo comercial posterior al SMTP; estados finales y reapertura requieren motivo y versión. La fecha de vencimiento se evalúa en Honduras después del día fecha+validez completo. Lotes comparten transacción entre efecto e ítem; la aprobación consumida se revalida antes de enviar. Sin dependencias ni migraciones nuevas.

## 2026-10-11 — Conversión propietaria Quotes→OC

La regla legacy «cambiar estado después del commit» se sustituye por una transacción compartida entre servicios propietarios: OC completa y Quote converted con vínculo/versiones/evidencia atómicos. Sólo Aceptada, una OC por fuente y número explícito; sin borradores vacíos ni aceptación interna confundida con aceptación comercial. El contrato único `../contracts/quote-purchase-order-conversion.md` fija permisos de ambos registros y replay. Se copian snapshots históricos autorizados sin refrescar maestros; documentos permanecen en su propietario. Fuente convertida inmutable; clonar cotización permite una nueva intención comercial sin vínculo heredado.

## 2026-10-11 — Identidad estable para documentos de OC

La edición conserva los IDs enviados de ítems y actualiza sus filas; reordenar mueve posiciones sin cambiar identidad. IDs nuevos vacíos; duplicados/ajenos se rechazan. Una FK compuesta y el propietario impiden eliminar ítems con vínculos documentales, también en papelera. No reutilizar posiciones para adjuntos ni eliminar binarios como efecto de edición. DTO previo sin IDs sigue permitido para reemplazar ítems sin documentos; si omite un ítem vinculado, conflicto/rollback. Core posee archivos; PurchaseOrders posee destino y evidencia, con política registrada según flag. Contrato único `../contracts/purchase-order-documents.md`.

## 2026-10-11 — Unidad 5 OC: clonación cerrada

Contrato `purchase-order-cloning.md`, migración 0019 y POST clone estricto. Copia snapshots históricos de cliente/proveedor y versiones, moneda HNL/USD, cantidades, impuestos y descuentos. Número, fechas, destino, comentarios y gastos nuevos explícitos. OC independiente en Recibida/ítems Pendiente con identidad nueva, sin documentos, entrega, historial operativo ni vínculo Quote directo. La fuente conserva su versión/estado y el destino registra origen/versión durable. Permisos view/create y equipo actuales; replay reautoriza también lectura del destino. Idempotencia, cabecera/ítems, relación, auditoría/eventos comparten TX. Sin grants automáticos ni escritura técnica/MCP.

HEAD certificado `6667fe62f952fd1702c627ae88138bab9924a522`. [Core checks 38165562083](https://github.com/Ahcast/Bastion/actions/runs/38165562083), [documentación 38165562010](https://github.com/Ahcast/Bastion/actions/runs/38165562010) y [rama única 38165561926](https://github.com/Ahcast/Bastion/actions/runs/38165561926): success. Go fmt/vet, race/MySQL 5.7 y builds server/worker/admin; Angular build/**75 pruebas**; navegador **10/10** con clonación de OC entregada y adjuntos, cálculo preservado, reinicio operacional, adjuntos aislados y recarga.

Pruebas nuevas: vectores mixtos HNL/USD 15/18/exento/exonerado y descuentos, 12 replays concurrentes, versión/intención conflictivas, fuente inmutable, estados/IDs nuevos, documentos/historial no copiados, scopes de fuente/destino/revocación, catálogo actualizado y acceso de catálogo retirado con snapshots históricos conservados, creación en equipo ajeno y membresía perdida, concurrencia con transición y orden uniforme de locks, rollback de evento y retry, DTO/CSRF estricto, UI clave/destino conservados ante fallo. Local: build/75 pruebas Angular, compilación 10 escenarios, paridad/enlaces y diff. Go/MySQL/navegador acreditados por CI.

CI intermedio identificó formato de plantilla que separaba una expresión monetaria y fallos del buffer CDP al leer respuestas comerciales. Plantilla anterior conservada; pruebas de conversión/clonación capturan la respuesta del servidor mediante forwarding real sin alterar petición/cuerpo, y la devuelven al navegador. No datos simulados, aserciones retiradas ni timeouts ampliados. Regresión completa final aprobada. Sin despliegue, correo, migración productiva ni certificación S3/escáner/Bluehost.

Siguiente unidad: aceptación integral OC, incluyendo política de eliminación reversible y utilidad canónica, antes de Facturas/pagos.

## 2026-10-11 — Unidad 6 OC: aceptación integral en validación

Contrato purchase-orders-acceptance.md y matriz development/purchase-orders-acceptance.md. Migración 0020 con deleted_at, profit/profit_version, backfill decimal e índice de papelera; ordenes.delete sin concesión automática. Eliminación reversible solo Recibida/ítems Pendiente y sin conversión, documentos (incluida papelera), historial o clones derivados. Reauth/razón, scopes view/delete, versión/idempotencia/auditoría/evento en TX; protege número y snapshots. Ficha/papelera autorizadas; mutaciones/archivos de eliminadas bloqueados. Restauración conserva datos y devuelve fecha servidor. Utilidad canónica = ventas netas sin ISV menos inversión total; función finance.Profit compartida también por margen Quotes sin gastos adicionales. Monedas separadas, pérdidas permitidas.

Backend/UI implementados; pruebas de preservación, revocación/reauth/replay/rollback, evidencia protegida, identidad y número externo conservados sin unicidad global, backfill real MySQL, HTTP/CSRF, UI respuesta confirmada y navegador papelera/restauración/utilidad añadidas. Regresión y CI pendientes; no cerrar módulo ni iniciar Facturas hasta completar esta aceptación. Producción e integraciones externas pendientes.

## 2026-10-11 — Numeración externa y aceptación OC

El número externo de OC no tiene unicidad global conforme a purchase-orders.md y migración 0015. Se corrigieron el contrato de clonación, papelera, OpenAPI y la prueba que suponía una reserva global; la restauración conserva ID, número, ítems y cálculo aun cuando otra OC comparte el número. No se modifica la regla de persistencia ni se introduce un índice único. La prueba de navegador del clon descuenta sus gastos explícitos: ventas netas 175.00 − productos 80.00 − gastos 9.99 = utilidad 85.01 HNL. La consulta de cotizaciones captura la respuesta real mediante route.fetch/fulfill para evitar pérdida del buffer Chromium, sin alterar su contenido.

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

## 2026-10-12 — Comprobantes Gastos: permisos y orden de locks

La revisión final de unidad 2 exige permiso view además de acciones específicas sobre el gasto. Core protege mutaciones con orden actor→propietario→documento y revalida binding/owner/equipo al bloquear; evita inversión documento→actor/propietario frente a LinkDocument/revisión. El helper Core permanece genérico con políticas inyectadas, sin imports de negocio. Se exige regresión global de documentos y carrera real MySQL entre dos actores para gasto/papelera de comprobante. Paginación de metadata autorizada se ejecuta en SQL antes de recuperar la página, sin recorrer todos los archivos en memoria. Las referencias son documentales, no costos adicionales aplicados automáticamente.

## 2026-10-12 — Recuperación del pull de MySQL de CI

Core 38300195778 falló en backend antes de ejecutar pasos de prueba: public.ecr.aws devolvió `toomanyrequests: Data limit exceeded` al descargar MySQL. Docker Hub oficial library/mysql:5.7.44 devuelve HTTP 200 y el mismo digest OCI `sha256:4bc6bc963e6d8443453676cae56536f4b8156d78bae03c0145cbe47c2aad73bb`, verificado mediante la API primaria registry-1.docker.io. CI backend/browser usa docker.io con ese digest fijado. No se sustituye versión/artefacto ni se desactiva validación. Afecta exclusivamente descarga de contenedores de prueba; la configuración productiva de MySQL permanece en su contrato Bluehost.

## 2026-10-12 — Excel Reportes determinista y precisión

Renderer XLSX fijo con archive/zip y encoding/xml de Go estándar, sin dependencia runtime adicional. Dos hojas OOXML, texto financiero/IDs exacto, SUM SQL por moneda y fórmula auxiliar de centavos enteros únicamente cuando SUM ABS SQL <=999999999999999 cents. Se evita cambiar silenciosamente DECIMAL(18,2) a precisión Excel de 15 dígitos; aviso y total exacto siguen disponibles. Export privado como stream autenticado con reauth, scopes intersectados, auditoría y límites explícitos de filas/XML/concurrencia, sin URLs ni archivos persistidos. Fuente primaria/restricciones documentadas en contracts/reports-export.md. Lector independiente openpyxl local verificó el libro sintético; no se añade esa biblioteca al runtime del servidor.
