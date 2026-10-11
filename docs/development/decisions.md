# Decisiones de consolidación — 9 de octubre de 2026

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
