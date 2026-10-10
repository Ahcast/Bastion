# Contrato ejecutable de Proveedores

Unidad 2; depende únicamente de Core. Clientes cerró sus pruebas locales y CI antes de iniciarla. Migración global 0003_suppliers.sql, MySQL 5.7/InnoDB. Tablas propias: suppliers, supplier_contacts, supplier_tags, supplier_products, supplier_notes y supplier_price_observations. Sin escrituras en futuros propietarios OC/Gastos/AI/Workflow.

## Maestro y datos editables

Ficha completa: name (180), rtn opcional (30, canónico 14 dígitos), country (80, nombre libre), website (2048, HTTP/HTTPS sin credenciales), contact_name (120), phone (50), email (254), address (1000), notes (4000), payment_terms (1000), lead_time (120), evaluation (null o entero 1..5), reliability (unknown/low/medium/high), response_hours (null o entero 0..8760), is_active, team_id, version string, contacts (20), categories y brands (20 cada una), products (50). Evaluación, confiabilidad y respuesta son apreciaciones humanas, no métricas calculadas ni recomendaciones automáticas.

Contacto: name/role/phone/email. Producto: name (120), brand/part_number (80), description (1000), lead_time (120), moq (cadena decimal opcional DECIMAL(18,4), sin redondear). Colecciones reemplazadas dentro de la transacción/versionado del maestro; productos descriptivos sin referencias externas a filas de esta versión. No borrado ni merge. Inactivar conserva identidades, precios y documentos.

Nombre canónico NFKD/diacríticos/minúsculas/espacios, manteniendo puntuación; UNIQUE incluso inactivo. RTN vacío permitido para extranjeros; RTN presente UNIQUE canónico, sin afirmar validación fiscal. Country conserva el nombre aportado y su filtro exacto usa normalización canónica. Categorías/marcas únicas por nombre canónico dentro de su tipo/proveedor; duplicados devuelven 422. No catalogar automáticamente resultados web como proveedores definitivos.

POST/PATCH exigen todos los campos editables, incluso vacíos/null explícitos donde se permiten. PATCH reemplaza ficha completa y exige version vigente; POST version "0". Campos desconocidos/protegidos rechazados. GET listado devuelve resumen; únicamente GET ficha devuelve colecciones completas. IDs y versiones siempre cadenas.

## Evidencia e historial

Notas append-only de hasta 4000 caracteres; no sustituirlas con el campo editable notes. Observación de precio append-only: source_url HTTP/HTTPS, source_type web/catalog, source_observed_at RFC3339 no futuro, part_number (80), observed_price cadena opcional DECIMAL(18,6), currency vacía sin precio o tres letras con precio, availability (500), lead_time (120), moq decimal opcional, taxes/shipping unknown/included/excluded y version vigente del proveedor. Fecha de captura y actor los fija el servidor. No floats, exponentes, negativos ni redondeo silencioso. Se almacenan valores observados, sin calcular totales ni convertir monedas. Fuente/fecha obligatorias incluso si no se encontró precio. Una web/catalog no se convierte en cotización oficial. No se descargan URLs ni se envían correos al registrarlas.

Notas y evidencia incrementan la versión, auditan cambios mínimos y emiten hechos suppliers.noted/price_observed en la misma transacción; suppliers.created/updated para ficha. Ningún hecho dispara jobs/correo/AI implícitos. Auditoría no contiene notas, contactos, precios ni URLs completos. Historial/notas/precios/documentos paginados, sin topes silenciosos.

Snapshot devuelve valores copiados de un proveedor activo autorizado para futuros OC/Gastos; LegacySnapshot conserva supplier_id null y nombre libre sin buscar, crear ni fusionar un maestro. ResolveExact consulta nombre/RTN activos autorizados, sin match devuelve null. Cambiar maestro no altera snapshots entregados.

## Permisos y transporte

proveedores.view/create/update asignables all/self/team, sin otorgamiento automático al administrador técnico. Propietario created_by inmutable. Alta en equipo propio o create all para equipo ajeno; cambio de equipo update all y reautenticación. Inactivar/reactivar requiere reautenticación. Las escrituras, notas y evidencia son humanas; conexiones técnicas de lectura solo proveedores.view y permisos vigentes por registro. Revocación efectiva en la siguiente consulta.

REST: GET/POST /api/v1/suppliers, GET/PATCH /suppliers/{id}; GET /suppliers/teams; GET/POST /suppliers/{id}/notes; GET /suppliers/{id}/history; GET/POST /suppliers/{id}/price-history; GET/POST /suppliers/{id}/documents. Lista: search/country/is_active/page/per_page, máximo 100. Escrituras con Idempotency-Key; replays autorizados, diferente cuerpo 409; versión obsoleta 409; 401/403/404/422 según Core, registro ajeno 404. Documentos privados mediante BindDocument, ambas políticas, versión del archivo/reautenticación y cuarentena intactas.

MCP solo lectura: search_suppliers, get_supplier, resolve_supplier_exact, get_supplier_price_history; mismos servicios, actor contextual, schemas explícitos y auditoría de consulta. BASTION_SUPPLIERS_ENABLED false al reiniciar bloquea rutas/tools/menú/política de archivos y conserva datos.

## Aceptación y dependencias

Cerrar normalización/validación/decimales/DTO, MySQL real (duplicados, idempotencia, rollback, versión concurrente, scopes/IDOR/revocación, filtros/paginación, snapshots/historial/evidencia/archivos), paridad REST/MCP SDK, Angular build y comportamiento, navegador real y regresión Core/Clientes/crawler. Publicar y esperar CI satisfactorio antes de comenzar Procesos.

RFQ/borradores/envío aprobado, cotizaciones recibidas oficiales y métricas de respuesta calculadas dependen de AI/Workflow y sus documentos/operaciones. IA conserva sus candidatos, runs y evidencias propias; promoción a maestro usará este servicio humano/autorizado mediante contrato futuro. Selección/snapshots en OC/Gastos se incorporan cuando existan esos propietarios. Estas conexiones mantienen su especificación funcional y no se declaran implementadas en esta unidad. Servicios productivos Bluehost/S3/SMTP/Work siguen pendientes de validación externa.
