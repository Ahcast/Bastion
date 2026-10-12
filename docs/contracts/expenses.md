# Gastos — contrato propietario

## Desarrollo en orden

1. Registro operativo: cálculo decimal legacy, proveedor libre/catalogado, categorías, CRUD/versionado, filtros, advertencia de duplicados, totales por moneda, papelera/restauración, REST/UI y seguridad. Migración 0030.
2. Vínculos comerciales y comprobantes: referencias opcionales a Proceso/Cotización/OC/Factura mediante puertos públicos y snapshots versionados; adjuntos privados Core/S3. Sin alterar costos, pagos ni snapshots de otro propietario. Migración siguiente.
3. Aceptación integral: Go/race/MySQL 5.7, autorización/concurrencia/rollback, Angular y navegador real, documentación y rama única. Cerrar sólo tras CI verde.

## Unidad 1

`ExpenseCalculator` recibe `currency` HNL|USD, `taxable_amount` y `exempt_amount` como cadenas decimales no negativas, máximo dos decimales. ISV = gravado × 0.15 redondeado a centavos half-up; total = gravado + exento + ISV. Rechazar desbordamiento DECIMAL(18,2). Importes calculados normalizados a dos decimales y tasa `0.1500`, versión 1. Cero válido; ningún monto float ni FX implícito. Esta fórmula conserva legacy; no acredita tratamiento fiscal de gastos reales.

`expenses` InnoDB: ID, número externo de documento nullable, fecha DATE, supplier_id FK nullable, supplier_name_snapshot y supplier_version, categoría ASCII, moneda, gravado/exento/tasa/impuesto/total DECIMAL, notes, calculator_version, version, created_by/updated_by, team_id FK nullable, timestamps, deleted_at/by/reason. Índices fecha, supplier, owner, team, currency/category/deleted. No unicidad de número externo: proveedores pueden repetir números; advertencias no bloquean ni fusionan.

Categorías: product_purchase, freight, customs, taxes, transport, travel, software, services, advertising, office, other; UI en español. DTO completo `{invoice_number, expense_date, supplier:{id,name}, category, currency, taxable_amount, exempt_amount, notes, team_id, version}`. POST version `0`, PATCH versión vigente positiva; IDs/versiones JSON cadenas. Campos requeridos (vacío permitido en referencia, notas, supplier.id y equipo); unknown/null rechazados. Fecha ISO real, año 1000–9999; fecha histórica/futura explícita permitida. Referencia máximo 100, proveedor 1000, notas 12000.

Proveedor libre sigue válido. Al asignar/cambiar supplier_id, resolver proveedor activo/autorizado mediante Suppliers.SnapshotInTransaction y descartar nombre cliente. Edición con mismo ID conserva snapshot/version histórico incluso si el catálogo cambia o se desactiva. Para refrescar, quitar y volver a asignar explícitamente. No se modifica catálogo.

Permisos `gastos.view/create/update/delete/restore`, sin grants productivos automáticos. all/self/team sobre creador/equipo. Humano requerido para writes. Transferir equipo, eliminar/restaurar exigen reauth. Destino requiere acción y view; asignación fuera del equipo necesita permiso global. Lecturas ocultas 404, falta de permiso 403, versión/estado divergente 409, entrada 422. CSRF y MFA Core; flag expenses bloquea rutas/menú. Versiones se incrementan sin overflow; idempotencia/audit/event dentro de la misma TX; replays reautorizan acceso actual. Restablecer papelera usa motivo; ningún hard-delete.

REST: GET/POST `/expenses`, GET/PATCH/DELETE `/expenses/{id}`, POST `/expenses/{id}/restore`, POST `/expenses/calculate`, GET `/expenses/totals`, GET `/expenses/{id}/duplicates`. DELETE/restore DTO `{version,reason}`; Idempotency-Key obligatorio para todos writes. Filtros search/supplier_id/date_from/date_to/currency/category/deleted/page/per_page; deleted true requiere restore además de view por registro. Búsqueda por referencia/proveedor/notas. GET de eliminado exige restore; edición eliminado 409. Totals usa mismo filtro/scope sin paginación y devuelve filas HNL/USD separadas, excluye papelera por defecto. Duplicados de gasto guardado busca sólo activos visibles al actor, mismo proveedor (ID o nombre exacto), moneda, fecha, total y referencia no vacía, excluyendo sí mismo; lista paginada, sin IDs de otros scopes. Advertencia informativa, no eliminación/fusión automática.

UI española con cálculo servido por Go, proveedor manual y catálogo opcional, categorías/filtros, ficha y papelera. Respuesta confirmada e intención ante ACK incierto; bloquear otros writes hasta recuperar versión si falla recarga. No usar cálculo del navegador como autoridad. Aceptación: decimales frontera/redondeo/desbordamiento, snapshot histórico, scopes, equipos/reauth/revocación, 12 replays, competencia de versiones, rollback de evidencia, HTTP estricto/CSRF/flag, filtros/totales por moneda/duplicados, eliminar/restaurar, Angular y navegador.

IA/OCR futura propone campos pero no determina montos ni registra automáticamente. Gastos manual funciona sin IA; OCR/extracción externa quedan en fase IA, sin ejecutor Lightsail.
