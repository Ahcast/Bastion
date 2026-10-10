# Contrato de Cotizaciones

## Unidades implementadas y unidad activa

Quotes posee el agregado comercial y sus extensiones mediante las migraciones 0006–0013: quotes/quote_items/quote_versions/document_counters, snapshots PDF, estados, aprobaciones Workflow, emisión, programación de outbox, autorizaciones de correo y lotes bulk. Core no importa negocio. Composición inyecta servicios públicos transaccionales de Clientes y Procesos.

Están implementados y cubiertos en CI: borradores manuales o desde proceso, editor, consulta paginada, cálculo canónico, clonación, historial, papelera/restauración, estados/revisiones, PDF/adjuntos, aprobación humana, emisión segura de PDF, correo comercial inmediato/programado y bulk trazable. S3/IAM/scanner y SMTP/IMAP reales siguen siendo validaciones externas de producción; los stores/fakes de prueba no los certifican.

La unidad funcional siguiente es FX fechado. El sistema admite HNL o USD como moneda explícita pero no convierte entre ellas. No existe todavía contrato normativo para fuente/tipo de tasa, fecha efectiva, snapshot/historial, precisión/redondeo, autoridad de modificación ni representación. No implementar conversión implícita inventando esas reglas. Las conversiones a OC/Facturas se completarán mediante sus servicios propietarios cuando existan; Quotes no escribe sus tablas privadas. No avanzar a OC mientras la aceptación restante de Cotizaciones siga abierta.

## DTO y rutas base

IDs/versiones son cadenas JSON. Input reemplaza todos los campos editables: customer {id,name,rtn,contact,phone,email,address}, reference, date (DATE ISO), validity_days (1..365), currency (HNL/USD), terms, private_notes, author_contact, exoneration_reference, general_discount_type (amount/percent), general_discount y items[] {description,unit,quantity,unit_price,unit_cost,discount_type,discount,tax}. Máximo 200 ítems; creación version="0", edición versión confirmada. Items vacíos solo borrador. La vinculación a cliente obtiene snapshot autorizado/activo; conservar mismo ID al editar mantiene el snapshot original. Cliente manual con id="" permite datos libres validados. Origen técnico no es editable; reference visible sí. Flete se captura como línea explícita con impuesto propio; no se inventa tratamiento fiscal.

Rutas base: GET/POST /api/v1/quotes; GET/PATCH /api/v1/quotes/{id}; POST /api/v1/quotes/calculate; POST /api/v1/quotes/{id}/clone; GET /api/v1/quotes/{id}/versions; DELETE /api/v1/quotes/{id}; POST /api/v1/quotes/{id}/restore. GET list acepta search literal (número/referencia/cliente/descripciones), deleted=true/false y page/per_page máximo 100. GET versions paginado. POST /api/v1/processes/{id}/create-quote con {date,version:"0"} usa fecha comercial explícita, copia solo entidad/expediente y deja ítems vacíos. GET /api/v1/processes/{id}/quotes devuelve cotizaciones visibles (incluida papelera si solicitado), sin filtrar permisos por navegación solamente.

Salida Quote contiene id, number, status=draft/costing/review/approved, source_process_id, customer snapshot, campos editables, version, created_by/team_id, deleted_at, items con resultado por línea y calculation; List solo summaries. Migración mantiene cabecera e ítems como fuente maestra relacional y versiones JSON append-only de agregado/calculador (calculator_version=1); JSON no sustituye tablas actuales. Resultados de escrituras {id,version}. Idempotency-Key obligatorio en escrituras, normalizado antes de hash; replay reautoriza y nunca refresca un snapshot ni consume otro consecutivo. Clonar genera nuevo número, conserva origen y snapshots, devuelve borrador; no copia adjuntos ni aprobaciones.

## Cálculo exacto v1

shopspring/decimal v1.4.0, solo constructores desde cadenas; ninguna aritmética financiera float64 ni dependencia del estado global de división. Cadenas sin exponentes/signos, hasta 18 dígitos totales y escalas quantity=4 (>0), unit_price/unit_cost=6 (>=0), descuentos de unidad=6, porcentaje=4 (0..100), descuento general absoluto=2. Valores negativos, descuentos mayores al precio/base, sobreflujo DECIMAL(18,2) y exoneración sin referencia son 422.

Descuento amount por unidad; percent = precio*porcentaje/100 exacto. Bruto de línea = round(quantity*price,2), neto antes de general = round(quantity*(price-unit_discount),2), descuento de línea = bruto-neto. Half-up a 2 decimales. Descuento general amount o round(suma netos*porcentaje/100,2). Distribución proporcional en centavos: cociente entero exacto, restos mayores primero, desempate por posición; suma asignada idéntica al descuento solicitado y ninguna base negativa. Base final por línea = neto-asignación. Tax enum isv15/isv18/exempt/exonerated, bases separadas. ISV por grupo = round(base*0.15/0.18,2), total = suma bases+ambos impuestos. Costos = suma round(quantity*unit_cost,2); margen antes de impuestos = suma bases-costos. Costos/margen/notas son internos, nunca salida pública. Desglose persistido se devuelve sin recalcularlo con otra versión.

Moneda no implica conversión. Un agregado contiene importes de una sola moneda declarada HNL o USD. FX y cálculos entre monedas son una unidad separada y no deben mezclar importes hasta que exista su contrato normativo.

## Seguridad, estados y aceptación

Permisos base: cotizaciones.view/create/update/delete; scopes all/self/team y RBAC/MFA live. Edición/clone requieren acceso de vista y acción. Estados draft/costing/review y aprobación interna approved; aprobación humana usa acciones específicas; emisión no se simula con PATCH libre. Soft-delete/restauración requieren delete y reautenticación, versión e intención; sin hard-delete. Propietario derivado del actor; equipo inicialmente null (self/all), team para futuras asignaciones autorizadas, consulta respeta team. No escritura de máquinas. MCP get_quote/search_quotes solo lectura y mismo servicio, auditoría mínima. Flag BASTION_QUOTES_ENABLED=false retira rutas/tools sin borrar datos. Dependencia Core obligatoria; Clientes/Procesos opcionales: vincular/conversión falla cerrado si deshabilitados, manual sigue operativa.

Crear desde proceso exige procesos.view y cotizaciones.create, comprobados dentro de TX antes de replay. Resolver exacto aplica clientes.view; falta de grant no bloquea snapshot manual ni revela existencia. Entidad hasta 1000 caracteres; no autocrear maestro, no copiar descripción/documentos/notas/fecha/items. Varias intenciones crean varias cotizaciones. Numeración lock transaccional document_counters, UNIQUE number; fallo de auditoría revierte agregado/version/contador/eventos/intención. Eventos quote.created/updated/cloned/deleted/restored y process.quote_created transaccionales, sin jobs AI implícitos.

Aceptación ya cubierta por unidades implementadas: descuentos absolutos por unidad, porcentajes, cuatro impuestos, residuo exacto, límites/redondeos; MySQL5.7 migración/repetición, contador concurrente, intención concurrente, rollback, versión/IDOR/scopes/revocación, snapshots y conversión múltiple desde Procesos; PDF/adjuntos, estados, aprobación, emisión, correo comercial y bulk tienen sus pruebas específicas de integración/UI/navegador. El módulo completo sigue abierto por FX y por conversiones hacia módulos propietarios aún inexistentes. Las validaciones externas de producción no se sustituyen por CI.

## Unidad de archivos y PDF

Migración 0007 agrega documents.deleted_at/is_generated (Core) y quote_pdf_snapshots (Quotes). Adjuntos se vinculan mediante Core.BindDocument; GET/POST /quotes/{id}/documents aplican ambas políticas. DELETE /documents/{id} y POST /documents/{id}/restore son papelera Core, con permiso documents.write, política de entidad, reauth, version e intención. Metadata permanece; descarga/reemplazo/vínculo bloqueados en papelera. PDFs generados son inmutables: no reemplazo ni papelera genérica, pues preservan evidencia/snapshot. Desactivar Quotes retira su política y bloquea acceso desde Core Files.

POST /quotes/{id}/pdf con {version:string}, Idempotency-Key y cotizaciones.view/pdf + core.documents.write crea snapshot público de versión confirmada y compañía configurada. S3/Files debe estar habilitado. GET /quotes/{id}/pdfs pagina metadata, GET /quotes/{id}/pdfs/{snapshot}/download requiere cotizaciones.view/pdf y core.documents.read. Respuesta generación {id,document_id,quote_version,status}, ninguna URL/object key ni bytes PDF en base64. Descarga Core con presign cinco minutos y auditoría. Snapshot público excluye costos, margen, notas privadas, IDs del proceso, usuario/equipo y clientes, historial técnico. Captura plantilla v1, compañía y activos gráficos locales aprobados opcionales; no URL externa. Snapshot BORRADOR no certifica aprobación/envío.

PDF se renderiza en Go sin navegador: A4, fuente Unicode integrada, encabezado/pie numerado, saltos de página y separación de ítems, descuentos/bases/ISV, total en letras, términos/contacto/expediente y constancia de exoneración. Logos/sellos opcionales se configuran como PNG locales mediante BASTION_PDF_LOGO/BASTION_PDF_STAMP, máximo 1 MiB cada uno/2000x2000; no sello inventado. Nombre/RTN/dirección usan configuración company de Core y se congelan. Render limitado a 250 páginas/10 MiB; glifos no soportados fallan 422, sin omisión silenciosa.

Reserva/input público/idempotencia en TX; render/S3 fuera del lock SQL; objeto inmutable con If-None-Match y comprobación SHA256 en replay; registro del documento disponible y estado final en TX reautorizada. Fallo render/S3/SQL conserva snapshot pendiente y retry de misma intención recupera, sin segundo PDF/documento. Cambios posteriores no reescriben ese snapshot. Cliente debe consultar versión confirmada antes de otra intención. No jobs sin handler ni activación de cuenta S3 durante pruebas. Pruebas con store sintético no acreditan S3 real.

GET /documents/{id}/preview usa la misma política Core/de entidad, cuarentena y papelera que descarga. Presign cinco minutos inline con tipo firmado permitido exclusivamente PDF/PNG/JPEG/text/plain (UTF-8), nunca HTML ni SVG; abre visor del navegador fuera del origen de aplicación con noopener. No es scanner ni autorización para contenido pendiente.

## Unidad de estados preliminares y revisión

Migración 0008 registra cotizaciones.transition, sin otorgarlo a roles. No nuevas tablas: Quotes conserva estado/versiones y Workflow valida un grafo explícito versionado en código; edición de definiciones/aprobaciones configurables sigue pendiente. PATCH /quotes/{id}/status exige {status,version:string}, Idempotency-Key, usuario humano, cotizaciones.view y cotizaciones.transition con scope del registro. Respuesta {id,version,status}. Grafo: draft→costing; costing→draft/review; review→draft/costing. Revisión exige al menos un ítem guardado; no se impone un umbral de margen ni precio positivo inventado. Estado desconocido o versión cero: 422; salto, mismo estado, papelera o versión obsoleta: 409; registro ajeno: 404; permiso revocado: 403. No existe acceso directo a approved/sent/accepted/rejected/expired.

Cada transición bloquea el agregado, incrementa versión y registra snapshot, audit y quote.status_changed en la misma TX/intención. Replay reautoriza, no agrega versiones. Guardar cambios desde costeo/revisión/aprobación interna vuelve a draft con nueva versión (evento quote.revised), preservando número, origen y snapshots previos. Papelera solo desde draft; volver explícitamente a borrador antes de eliminar. Adjuntos editables en draft/costing, bloqueados en review/approved; PDF de borrador continúa marcado BORRADOR. UI muestra estado real, ofrece únicamente pasos permitidos y exige guardar/cancelar cambios pendientes antes de transicionar.

Aceptación: secuencia/revisión/replay/versiones, prohibición de aprobación/emisión directa por PATCH, rollback completo ante fallo audit, ocho transiciones concurrentes, scopes all/self/team y revocación, DTO estricto, UI conserva intención ante timeout y estado/versión confirmados ante fallo de recarga, navegador real costeo→revisión→edición/reinicio. Aprobación/emisión/correo son acciones separadas descritas abajo.

## Unidad de aprobación humana

Migración 0009: approvals/approval_events pertenecen a Workflow; registra cotizaciones.request_approval/approve/revoke_approval sin grants. Workflow es transversal (flag BASTION_WORKFLOW_ENABLED, true por defecto), sin menú ni HTTP genérico. Si se deshabilita, Quotes mantiene borradores y retira rutas de aprobación; versiones/hashes siguen impidiendo reutilizar aprobaciones antiguas al reactivar.

POST /quotes/{id}/approvals exige {quote_version:string,expires_at:RFC3339,reason:string}; GET pagina aprobaciones (page/per_page). POST /quotes/{id}/approvals/{approval}/decision exige {quote_version:string,version:string,decision:approved|rejected,reason:string}; POST .../revoke exige {quote_version:string,version:string,reason:string}. Todas las escrituras requieren Idempotency-Key y actor humano. Solicitar requiere view/request_approval y estado review; decidir exige view/approve y reauth reciente, y actor distinto del solicitante y creador; revocar exige view/revoke_approval y reauth. Razón obligatoria al decidir/revocar (1..1000 caracteres); solicitud admite cadena vacía. Vigencia futura explícita hasta año 9999, truncada a microsegundos; ningún plazo/margen se aprueba automáticamente.

Solicitud congela acción quote.emit, versión destino actual+1 y SHA256 de agregado comercial destino, compañía/version, plantilla/activos públicos y metadata ordenada de todos los adjuntos activos no generados. Adjuntos exigen ambas políticas Core/Quotes y estado available; cuarentena/pending/rejected bloquean solicitud/decisión. No se copian bytes ni secretos al ledger. Solo una solicitud por entidad/acción/versión destino. Solicitar no cambia versión de Quote; decidir incrementa Quote: approved crea estado approved; rejected conserva review. Revocar pending/approved incrementa Quote y regresa a review. Resultado {id,version,status,approval_id}; request devuelve metadata de aprobación.

Workflow conserva evento append-only por versión de aprobación y auditoría transaccional; Quotes conserva su snapshot/evento. Editar approved crea draft/revisión e invalida la aprobación. Otras mutaciones versionadas invalidan pending/approved en la misma TX. approved→draft permitido por transición preliminar, nunca PATCH→approved. Estado approved es aprobación histórica interna; la vigencia se calcula nuevamente, no es permiso de envío. Verificación exige coincidencia acción/entidad/versión/hash, expiry y permisos/MFA/cuenta actuales del aprobador. Quotes consume Workflow una sola vez y tras revalidar al emitir. La autorización para correo es distinta y se liga a destinatarios/contenido/evidencia como se describe más abajo.

GET informa valid/invalid_reason sin ejecutar efectos; paginación máxima 100. Consulta verifica políticas de adjuntos cuando calcula vigencia y permiso view del registro. Solicitantes/creadores no deciden ni por replay. 422 DTO/vigencia/razón inválidos; 409 versión/intención/estado/hash/vigencia conflictivos; 403 permiso/self-aprobación/reauth; 404 registro o aprobación ajenos; 503 Workflow o compañía no disponibles. PDF de borrador sigue BORRADOR incluso con aprobación interna; emisión y correo requieren acciones posteriores independientes.

Aceptación: solicitar/decidir/rechazar/revocar, expiración, invalidación de cambios en dinero/notas/compañía/activos/adjuntos, scopes/IDOR/reauth/autorización live, ocho decisiones concurrentes y consumo único, rollback de Workflow+Quotes+auditoría+intención, replay tras revocación, UI conserva edición/claves/versiones y recorrido con dos usuarios en navegador real.

## Emisión segura de PDF

Migración 0010 amplía quote_pdf_snapshots con kind=draft|issued, approval_id único, binding_hash, issued_by/issued_at; registra cotizaciones.emit sin grants. POST /quotes/{id}/emit exige {version:string,approval_id:string}, Idempotency-Key, actor humano, cotizaciones.view/pdf/emit, core.documents.write y reautenticación reciente. Workflow y Files deben estar habilitados. La emisión conserva quotes.status=approved y su versión: la evidencia se vincula a esa versión inmutable; no declara estado sent ni entrega al cliente.

Reserva en TX bajo lock del agregado, verifica aprobación quote.emit y congela proyección pública/hash de binding. Render y PutImmutable fuera de TX; objeto privado sin documento descargable mientras pending. Finalización vuelve a comprobar versión/estado/contenido/compañía/activos/todos los adjuntos, vigencia y autoridad live del aprobador, permiso/reauth del emisor y Files. Consumo Workflow, documento disponible, issued_at/by, auditoría y quote.issued se confirman en la misma TX. Un fallo SQL revierte todos esos efectos y permite retry sobre el mismo objeto inmutable. Sin SMTP dentro de la emisión.

Un único PDF emitido por aprobación, aun con varias intenciones o actores concurrentes. Replay de emisión completada devuelve la evidencia histórica tras reautorizar permisos/reauth actuales, aunque la cotización se haya editado o la aprobación haya vencido; nunca consume otra aprobación ni reescribe bytes. Un pending debe seguir cumpliendo toda la autorización original al recuperarse; edición/revocación/vencimiento impiden finalizarlo. Resultado {id,document_id,quote_version,status:available,kind:issued,approval_id}, IDs como cadenas; sin keys ni bytes. GET /pdfs añade kind,approval_id,issued_at; descarga usa las políticas existentes de Core y Quotes.

PDF issued muestra EMITIDA - no acredita envío en lugar de BORRADOR; misma proyección pública/plantilla, nunca costos/margen/notas privadas. Ediciones posteriores conservan el PDF emitido histórico y exigen nueva revisión/aprobación para emitir otra versión. La emisión por sí sola no autoriza destinatarios ni correo; eso se resuelve por la unidad siguiente. 422 DTO inválido; 409 versión/estado/hash/aprobación conflictivos; 403 reauth/autoridad; 404 ámbito ajeno; 503 servicios no disponibles. Workflow deshabilitado retira POST /emit; PDFs históricos conservan sus políticas de consulta.

Aceptación: emisión idempotente/concurrente, replay histórico, rollback tras S3, fallo render/store, cambios/revocación/vencimiento durante I/O, scopes/IDOR/reauth/grants live y disabled, privacidad/determinismo/paginación del PDF final, UI sin ediciones pendientes y misma intención ante timeout, navegador con API/Angular/MySQL reales y almacenamiento sintético exclusivo de tests.

## Correo comercial inmediato y programado

Migración 0011 agrega programación durable al outbox del Core. Migración 0012 agrega quote_email_intents y la evidencia necesaria para vincular autorización comercial, entrega y resultado. Las rutas comerciales se registran solo cuando Workflow está disponible:

- GET /api/v1/quotes/{id}/email-authorizations
- POST /api/v1/quotes/{id}/email-authorizations
- POST /api/v1/quotes/{id}/email-authorizations/{intent}/decision
- POST /api/v1/quotes/{id}/email-authorizations/{intent}/revoke
- POST /api/v1/quotes/{id}/email-authorizations/{intent}/send

Solicitar autorización exige versión de cotización, pdf_snapshot_id emitido/disponible, arrays explícitos to/cc/bcc, hasta 9 attachment_ids únicos, subject, message, scheduled_at RFC3339, expires_at futuro RFC3339 y reason. `core.ValidateEmail` valida destinatarios/asunto/cuerpo y límites del transporte. PDF y adjuntos deben ser documentos disponibles, autorizados y pertenecer al agregado esperado; el total de adjuntos respeta el límite Core. La evidencia captura SHA/size/version y se ordena/canonicaliza antes del hash.

La autorización de correo es una acción humana separada de `quote.emit`: congela destinatarios, contenido, PDF emitido, adjuntos, horario y evidencia mediante binding/hash/version/vigencia. Decidir requiere separación de funciones y permisos live según Workflow; revocar mantiene evidencia histórica. Cambiar destinatarios, asunto, cuerpo, PDF, adjuntos, versión de cotización o evidencia invalida el binding original y requiere otra autorización. No existe autorización genérica para “enviar esta cotización a cualquiera”.

`send` no ejecuta SMTP dentro de la petición: crea/recupera el trabajo durable ligado al intent y a su programación. El worker revalida actor/permiso, versión/estado, binding, autorización vigente e integridad SHA/size inmediatamente antes del efecto externo. El transporte usa el mismo PDF emitido; no re-renderiza la cotización al enviar.

El ledger conserva estado y Message-ID. Éxito SMTP se registra una sola vez; una respuesta definitivamente fallida puede seguir la política de retry del Core. Si el resultado del efecto externo es incierto, el envío queda `uncertain` y no se reenvía a ciegas. Un reinicio/lease vencido no debe convertir incertidumbre en duplicado. La programación durable usa UTC internamente y no depende de mantener una sesión web abierta.

Permisos específicos de email y permisos Core/Quotes se reevalúan; deshabilitar Workflow retira las rutas de autorización/envío sin borrar evidencia. Los tests usan transporte sintético; pasar CI no acredita SMTP/IMAP real de Bluehost ni sus límites operativos.

Aceptación cubierta: binding completo, separación de funciones, revocación, replay/idempotencia, programación, revalidación live del worker, integridad previa a SMTP, ledger, resultado incierto sin reenvío ciego, UI de solicitud/decisión/envío e integración de navegador. La validación SMTP real permanece como gate de producción.

## Bulk trazable

Migración 0013 crea quote_batches y quote_batch_items. Rutas:

- POST /api/v1/quotes/bulk/status
- POST /api/v1/quotes/bulk/clone
- POST /api/v1/quotes/bulk/export

Todas exigen `Idempotency-Key`. `items` contiene 1..100 pares `{id,version:string}` sin IDs repetidos. Status añade `status` de destino permitido por la misma lógica individual. El batch persiste action, usuario, target, estado y cada ítem con posición, quote/version, item_status, error_code y resultado congelado. Respuesta: `{batch_id,action,status,results[],filename?,content?}`; cada result conserva posición/id/version/status/error_code/result opcional.

Creación del batch usa idempotencia del Core. Cada efecto individual recibe una clave determinista derivada de action/batch/position y reutiliza las operaciones propietarias: Transition, Clone o Get. Por ello permisos, scopes, versión y reglas de estado se revalidan por cotización. Replay de la misma intención devuelve el mismo batch y no duplica efectos; cambiar payload con la misma clave genera conflicto.

Errores de cliente por ítem (forbidden/not_found/conflict/invalid) se registran como `failed` sin ocultar los éxitos de otros ítems. Un error interno/transitorio no se liquida como fallo definitivo: la ejecución retorna error dejando el ítem pendiente, de modo que el retry pueda determinar/continuar el efecto con la misma identidad. El batch solo pasa a completed cuando no quedan ítems pending.

Export no genera una extracción privada: congela únicamente Summary autorizado de cada cotización visible con Número, Cliente, Referencia, Fecha, Moneda, Total, Estado y Versión. No contiene unit_cost, margen ni private_notes. El CSV UTF-8 con BOM neutraliza valores que podrían interpretarse como fórmulas (`=`, `+`, `-`, `@`) ante espacios iniciales. El replay devuelve el CSV congelado aunque la cotización cambie después.

Angular permite seleccionar únicamente filas visibles de la página actual y muestra conteo/errores por lote. Bulk no aparece al navegar la papelera. Si una solicitud falla antes de confirmar, el cliente conserva la misma clave mientras action+payload sean idénticos. Playwright real cubre selección de dos cotizaciones y descarga del CSV, incluyendo ausencia de nota privada.

Aceptación cubierta: batch trazable, idempotencia/replay, fallo parcial, selección duplicada rechazada, DTO REST estricto, export congelado y seguro, Angular retry/fallos parciales y navegador contra API+Angular+MySQL de CI. Checkpoint certificado: commit `e3f70fee85a8fd1be6dcdd56fd9fc0ada20f6328`, Core checks `38088666258`.

## FX fechado — unidad activa y gate contractual

La moneda de una cotización es explícita (`HNL` o `USD`) y todos sus importes pertenecen a esa moneda. En la versión actual no existe conversión HNL↔USD, tasa global implícita ni consulta automática a una fuente externa.

Antes de crear una migración FX o modificar el calculador se deben aprobar y documentar como mínimo estas decisiones:

1. fuente oficial o manual de la tasa y mecanismo de captura;
2. significado de la tasa aplicable (compra, venta u otra) por operación;
3. fecha/hora efectiva y zona horaria;
4. snapshot histórico de la tasa y regla para correcciones posteriores;
5. precisión de almacenamiento y redondeo de tasa/importes convertidos;
6. permisos y separación de funciones para crear/corregir/sustituir una tasa;
7. efecto de un cambio de tasa sobre borradores, versiones, aprobaciones y PDFs ya emitidos;
8. representación exacta de tasa/fecha/fuente en UI, PDF y exportaciones;
9. comportamiento cuando no existe tasa válida para la fecha requerida;
10. regla de conversión para futuras integraciones OC/Factura sin reescribir snapshots históricos.

Hasta que esas decisiones estén cerradas, una cotización puede crearse y operar completamente en HNL o USD, pero el sistema debe rechazar/omitir cualquier operación que pretenda mezclar o convertir monedas de forma implícita. Ninguna tasa obtenida de internet, configuración local o conversación previa se considera autorizada por defecto.

## Integraciones Cotización → OC / Factura

Estas conversiones son server-side, transaccionales e idempotentes y deben conservar referencia de origen. Se implementan cuando existan los módulos propietarios. Quotes entrega un snapshot/DTO público autorizado al servicio destino; OC/Factura valida sus propias reglas y persiste en su transacción/composición definida. Quotes no debe importar ni escribir directamente tablas privadas de OC/Facturas.

No crear endpoints simulados ni tablas temporales en Quotes para adelantar estas conversiones. La ausencia actual de OC/Factura no invalida las unidades ya cerradas, pero impide declarar la integración cruzada terminada.
