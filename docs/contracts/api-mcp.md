# Contratos REST y MCP

## Convenciones comunes

Origin API propuesto https://api.bastion.castilloit.net. Usuario SPA autenticado por sesión opaca/TOTP; identidades de servicio por bearer limitado; delegación MCP por OAuth/PKCE validado. Validadores separados. Ningún requested_by libre acredita identidad.

JSON tipado: IDs y decimales como strings; fechas ISO, timestamps UTC. Errores: code, message en español, field_errors si corresponde, request_id. HTTP 401/403/404/409/422/429 según causa, sin revelar registros ajenos. Listados paginados page/per_page (máximo inicial 100), filtros/orden permitidos, total cuando calculable. Operación sensible requiere permiso, estado, aprobación/reautenticación cuando aplique.

Escrituras reintentables admiten Idempotency-Key y hash normalizado de solicitud. Igual clave/cuerpo devuelve resultado autorizado previo; distinto cuerpo 409. Cambios sobre registro incluyen version esperado; conflicto 409. Estado + auditoría + outbox en transacción. POST largo puede devolver 202 + job_id salvo el contrato crawler, que exige ACK de importación 200/201.

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
