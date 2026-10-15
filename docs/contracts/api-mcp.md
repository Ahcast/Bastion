# Contratos REST y MCP

## Convenciones comunes

Origin API propuesto https://api.bastion.castilloit.net. Usuario SPA autenticado por sesión opaca/TOTP; identidades de servicio por bearer limitado; delegación MCP por OAuth/PKCE validado. Validadores separados. Ningún requested_by libre acredita identidad.

JSON tipado: IDs y decimales como strings; fechas ISO, timestamps UTC. Errores: code, message en español, field_errors si corresponde, request_id. HTTP 401/403/404/409/422/429 según causa, sin revelar registros ajenos. Listados paginados page/per_page (máximo inicial 100), filtros/orden permitidos, total cuando calculable. Operación sensible requiere permiso, estado, aprobación/reautenticación cuando aplique.

Escrituras reintentables admiten Idempotency-Key y hash normalizado de solicitud. Igual clave/cuerpo devuelve resultado autorizado previo; distinto cuerpo 409. Cambios sobre registro incluyen version esperado; conflicto 409. Estado, auditoría y eventos durables en transacción: domain_events conserva hechos comerciales sin consumidores implícitos; outbox_events conserva intenciones de jobs registrados. POST largo puede devolver 202 + job_id salvo el contrato crawler, que exige ACK de importación 200/201.

## Recursos canónicos

| Recurso REST | Módulo |
| --- | --- |
| /api/v1/auth/*, /users, /roles, /settings, /dashboard | Core |
| /api/v1/customers | Customers |
| /api/v1/suppliers | Suppliers |
| /api/v1/processes | Processes |
| /api/v1/quotes | Quotes |
| /api/v1/purchase-orders | PurchaseOrders: OC de clientes |
| /api/v1/invoices | Invoices |
| /api/v1/expenses | Expenses |
| /api/v1/reports | Reports |
| /api/v1/collaboration | Collaboration |
| /api/v1/documents | Core Files; permisos de entidad propietaria |
| /api/v1/ai-tasks, /rfqs, /approvals | AI/Workflow; reglas del propietario |

Prefijos de tabla/UI no cambian estos recursos. No implementar aliases /clients, /orders ni /ingestion como otro contrato. Conversiones usan /convert-to-purchase-order y /convert-to-invoice; proceso a cotización /processes/{id}/create-quote. Rutas de acción, DTO y estados concretos se definen con OpenAPI y spec del módulo antes de escribir handler.

## SPA entre orígenes

GET /api/v1/auth/csrf entrega token vinculado a sesión en cuerpo JSON no cacheable; Angular lo conserva en memoria y lo envía como X-CSRF-Token en mutaciones al origen API autorizado. Cookie de sesión HttpOnly/Secure/Lax/Path=/, sin Domain; fetch/HttpClient credentials incluido. CORS solo origen frontend, credentials=true, métodos y headers explícitos. Origin/CSRF comprobados en login y mutaciones; renovar token/ID al autenticar. No compartir cookie accesible de API para lectura desde frontend ni confiar en XSRF automático de Angular cross-origin.

Login/MFA/logout/me, enrolamiento y recuperación según Core. Login parcial no habilita negocio. Reauth separado para acciones sensibles. Solo invitación/alta administrativa.

## Crawler — contrato único

Base /api/internal/v1/crawler. POST /runs, /runs/{run_id}/processes y /runs/{run_id}/complete. Identity crawler-lightsail. El contrato exacto del cliente está en [IMPORT_CONTRACT](../../backend/crawler/docs/IMPORT_CONTRACT.md): schema, ACK, checksums, fields_present, orden por observed_at y reintentos se conservan. Sin DB directa crawler. FTPS es legado opcional del código existente, deshabilitado en despliegue objetivo; nombre interno bluehost persiste por compatibilidad.

## MCP

Ruta /mcp del mismo backend. Tools/resources/prompts con esquemas y límites. La composición registra tools de módulos; permiso de conexión no concede todas las herramientas. Identidad + scope + permiso del usuario + acceso al registro + estado/aprobación. REST y MCP llaman el mismo servicio, no copian cálculo ni autorización.

Herramientas IA vigentes en [ai-pipeline](ai-pipeline.md); nombres search_clients se normalizan a search_customers. Catálogo inicialmente mínimo de lectura/claim/save, ampliación por fase. No herramientas SQL/shell/S3 global ni email arbitrario. Cada escritura sensible registra actor, delegación, correlación, entrada/salida validada y resultado, sin secretos.

Pruebas: denegación anónima, MFA incompleto, acceso a registro ajeno, revoke/expiry, payload inválido, idempotencia concurrente, conflicto de versión y mismo resultado/reglas REST–MCP.

## Contrato implementado del Core

[OpenAPI Core](core-openapi.json) contiene DTO/rutas de plataforma. Core ofrece perfil/preferencias y regeneración recovery/password, usuarios/invitaciones/roles/equipos/settings/integraciones, auditoría/jobs/notificaciones, SMTP de prueba/conciliación, archivos versionados y lectura MCP técnica. OAuth Work permanece deshabilitado hasta validar proveedor.

Clientes, Proveedores, Procesos/receptor crawler, Cotizaciones, Órdenes de Compra de clientes, Facturas/pagos, Gastos, Reportes y Colaboración tienen contratos implementados y aceptación certificada; evidencia vigente en [checkpoint](../ESTADO_DESARROLLO.md). Las rutas comerciales y conversiones completas están en sus OpenAPI propietarios, incluidos estados, PDF, correo y aprobaciones; no se deducen del catálogo mínimo MCP de lectura.

IA U1 incorpora [tareas y reservas](ai-tasks.md) y [OpenAPI](ai-tasks-openapi.json): crear/cancelar por humano REST, consultar/reservar/renovar/registrar fallo mediante servicios comunes REST/MCP. Servicios read/execute requieren también procesos.view. No hay resultado genérico/completed, puntuación validada, ejecución de modelos ni conexión Work acreditada en esta unidad. Etapas siguientes siguen [pipeline](ai-pipeline.md).


## Restricción vigente de identidades de servicio

Los permisos de lectura de la UI/comerciales no se convierten en scopes de servicio por registrar un módulo. Core rechaza nuevas conexiones para `clientes.view`, `proveedores.view` y `cotizaciones.view`; un token antiguo que conserve esas cadenas tampoco puede leer mediante REST/MCP. `procesos.view` permanece como marcador de compatibilidad para IA, pero no autoriza consultas genéricas de Procesos. La lectura revisada de tareas IA exige `ai.tasks.read` y vuelve a validar el permiso vivo del usuario sobre el proceso; no abre los módulos completos.

`core.modules.read` y `core.jobs.read` son consultas técnicas revisadas. `core.documents.read` puede estar almacenado en una conexión heredada, pero no autoriza entrega privada ni URLs S3 y no existe tool MCP de descarga. Las capacidades técnicas de ingestión del crawler se registran explícitamente por composición. Las acciones humanas siguen con sesión/MFA/scopes actuales; OAuth delegado para lecturas comerciales de Work continúa pendiente. Las pruebas de tokens heredados insertan datos exclusivamente en la BD sintética para comprobar que la restricción también cubra conexiones previas; no habilitan scopes productivos.

## SPA: OTP recordado y QR

[Contrato de autenticación](remembered-mfa.md): login informa `mfa_required` real; cookie adicional HttpOnly `__Host-bastion_mfa`, ligada a cuenta/IP y limitada a 30 días absolutos. Caducidad de sesión conserva la prueba; salida manual/cambio IP y revocaciones la invalidan. Confirmación sensible pide contraseña; `code` es opcional para compatibilidad. Enrolamiento entrega `otpauth_uri` y `qr_data_url` PNG local, ambos secretos temporales con no-store. API/OpenAPI mantienen los DTO y errores existentes con estas extensiones. Esto no cambia autenticación MCP ni identidades de servicio.
