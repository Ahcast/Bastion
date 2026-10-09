# Contrato ejecutable de Clientes

Unidad 1 de módulos comerciales. Customers depende únicamente de Core. Tablas propias: customers, customer_contacts, customer_addresses y customer_notes. Migración global 0002_customers.sql; InnoDB/MySQL 5.7, IDs/versiones como cadenas y timestamps UTC.

## Reglas y DTO

Ficha: name, rtn opcional, contact_name, phone, email, address, institution_type (private/public/other), is_active, team_id opcional, contacts (máximo 20) y addresses (máximo 20). Cada contacto contiene name, role, phone y email; dirección contiene label y address. PATCH es reemplazo completo de la ficha editable con version obligatoria; created_by es inmutable. No admite asignación masiva de propietarios, IDs, permisos o historial. Notas append-only de hasta 4.000 caracteres con idempotencia y versión del cliente; no se borran referencias históricas ni clientes.

Normalización versión 1: Unicode NFKD, remover marcas diacríticas, minúsculas, espacios colapsados; no quitar puntuación ni palabras para evitar fusionar entidades diferentes. Nombre canónico UNIQUE y RTN canónico ASCII 14 dígitos UNIQUE nullable; separadores espacio/guion se eliminan. RTN vacío permitido. Duplicado concurrente devuelve 409 sin fusionar. No valida elegibilidad fiscal ni inventa RTN. Las restricciones también abarcan fichas inactivas.

Permisos clientes.view/create/update, configurables all/self/team; propietario = created_by, equipo = team_id. Crear requiere permiso para el propietario/equipo elegido; asignar un equipo ajeno exige create all, incluso con create self. Cambiar equipo exige permiso all y reautenticación; el usuario team no puede mover el registro a otro equipo. Inactivar/reactivar exige reautenticación reciente. Identidad de servicio solo lectura explícita clientes.view; no escrituras MCP. Roles técnicos existentes no reciben estos permisos automáticamente.

GET /api/v1/customers: search, rtn, is_active, page (1..100000), per_page (1..100); total y data usan el mismo predicado de acceso y filtros SQL. El listado devuelve resúmenes (id/name/rtn/email/is_active/version); solo GET de ficha incluye contactos/direcciones. No representar colecciones sin cargar como arrays vacíos ni usar el resumen para reemplazar la ficha. Sin límites silenciosos. GET /customers/{id}: ficha completa autorizada. POST /customers y PATCH /customers/{id}: Idempotency-Key, transacción, auditoría y evento durable. GET/POST /customers/{id}/notes y GET /customers/{id}/history: paginados y autorizados. Los errores siguen Core: 401/403/404/409/422; el acceso a un registro ajeno devuelve 404.

Servicios públicos Snapshot y ResolveExact respetan clientes.view y devuelven valores copiados independientes del maestro; el match solo utiliza clientes activos y exactos canónicos/RTN. Si no hay match, devuelve null sin crear cliente. El futuro documento comercial persistirá ese snapshot en su propia transacción; esta unidad no declara cotizaciones/facturas implementadas. No se fabrica un resumen 360° de módulos todavía inexistentes.

MCP search_customers, get_customer, resolve_customer_exact reutilizan los mismos servicios y actor verificado del Core. Resultados limitados/paginados y consultas MCP auditadas; no SQL ni merges automáticos. Historial con acción, actor, fecha y versión; notas y datos de contacto no se copian en logs. Eventos customers.created/updated/noted incluyen solo ID/versión y no disparan correos. Core conserva eventos comerciales sin convertirlos en jobs desconocidos; consumidores futuros se registran explícitamente.

Documentos: vínculo explícito a un archivo privado Core existente, autorizado por ambas políticas, con versión y reautenticación; ningún vínculo autoriza un archivo en cuarentena. GET /customers/{id}/documents aplica también permisos del archivo. Transporte/descarga/scan siguen Core y S3. Archivos reales/scanner permanecen pendientes de validación externa.

## Aceptación y orden

Antes de Proveedores: unit tests de normalización/validación; MySQL 5.7 con carreras de duplicado/idempotencia, versiones obsoletas, rollback de ficha/auditoría/evento, scope self/team/all y revocación, filtros/paginación, snapshots, notas/historial, documentos y cuarentena; coherencia REST/MCP con SDK cliente; Angular build/tests y navegador real creando/editando/buscando un cliente. Después regresión completa Core/crawler y CI remoto satisfactorio. Sin desplegar ni migrar la BD productiva.
