# Gastos: primer corte local

Fecha: 2026-10-24. Estado: implementación inicial en el repositorio; no desplegada ni activada.

## Alcance

Implementa `gastos` como módulo registrado, con migración propia, API HTTP y pantalla React. El registro general incluye listado/búsqueda, filtros de fecha, alta, consulta, edición y eliminación. El servidor normaliza importes, calcula ISV 15% y total, y vuelve a validar identidad, permiso, estado del módulo y campos en cada operación.

La tabla presenta factura, fecha, proveedor, moneda, base gravada, base exenta, ISV 15%, total y acciones. El formulario conserva los campos técnicos de la ficha Legacy. La sección Gastos aparece bajo Ajustes sin campos editables confirmados.

Este corte no incorpora gastos de línea de Órdenes ni de Facturas, no los combina con el registro general y no cambia sus cálculos. También quedan fuera los filtros por usuario/orden descritos en el inventario API, exportación, MCP, importación/migración de datos Legacy, reportes conectados y equivalencia visual verificada.

## Reglas y decisiones técnicas provisionales

Estas decisiones permiten revisar el flujo local; no se presentan como reglas Legacy confirmadas y deben cotejarse con la sesión activa, el controlador y el esquema MySQL original:

- Permisos: `gastos/registros/ver`, `gastos/registros/editar` para alta/edición y `gastos/registros/eliminar` para borrado. Todos los usuarios habilitados con `ver` consultan el registro compartido del módulo; el alcance Legacy por usuario/orden está pendiente.
- Campos: fecha, número de factura, proveedor, moneda, base gravada y base exenta. Fecha, número y proveedor se requieren. Una de las dos bases debe ser mayor que cero; no se aceptan importes negativos.
- Moneda: se acepta HNL, único valor observado en la ficha. No se ofrecen monedas adicionales hasta confirmar el catálogo.
- Límites técnicos nuevos: factura 120 caracteres, proveedor 255; importes con hasta 12 dígitos enteros y 4 decimales; persistencia normalizada a centavos en `DECIMAL(14,2)`. Valores fuera del rango representable se rechazan.
- Cálculo provisional: se redondean las bases ingresadas a centavos con mitad hacia arriba; ISV = 15% de la base gravada redondeado a centavos con mitad hacia arriba; total = base gravada + base exenta + ISV. El cálculo fiscal y su secuencia exacta de redondeo requieren verificación contable.
- Listado: búsqueda literal de número de factura/proveedor, fechas inclusivas, orden por fecha descendente e ID descendente. Se rechaza un intervalo invertido. Página por defecto 25, máximo 100 y página máxima 10000.
- Alta idempotente por actor y clave; reutilizar clave con contenido distinto devuelve conflicto. Edición usa `ETag`/`If-Match`; una versión obsoleta devuelve 412 y preserva cambios locales. El borrado físico requiere permiso separado, confirmación web y evento de auditoría; la bitácora conserva el evento según la política común.
- Límite de cuerpo 16 KiB, cuota 120 solicitudes por actor/minuto y timeout de 30 segundos. Son topes técnicos de este corte, no límites observados del Legacy.
- Persistencia nueva: `gastos_records`, `gastos_create_receipts` y `gastos_actor_limits`, con ledger `_gastos_migrations`. No se enlaza a identificadores Legacy.

## API

`GET/POST /api/v1/gastos`; `GET/PUT/DELETE /api/v1/gastos/{id}`. El contrato completo está en [gastos.openapi.yaml](gastos.openapi.yaml). Todas las mutaciones usan la identidad/cookie, CSRF, CORS, autorización y auditoría compartidas del Core. Crear devuelve 201, ETag y Location; actualizar exige `If-Match`; borrar devuelve 204.

## Interfaz

La navegación principal muestra Gastos si el usuario tiene `ver`; los controles de alta/edición/eliminación requieren sus permisos respectivos. La confirmación de borrado identifica la factura. El editor conserva datos ante error y conflicto, ofrece abrir la última versión, y advierte cambios sin guardar al salir. Las preferencias del módulo permanecen pendientes en Ajustes.

## Pendientes de cierre

- Contrastar campos, valores de moneda, mensajes, fórmulas/redondeo, búsqueda y filtros con el Legacy activo y el esquema real.
- Definir asociación y consistencia con gastos de Órdenes y Facturas, incluidos los filtros por usuario/orden.
- Acordar si se conserva el borrado físico o se requiere papelera/retención, y completar los settings propios si corresponde.
- Validar MySQL 5.7 en CI, permisos por rol, límites, visualmente en escritorio/móvil y flujo con sesión autenticada.
- Definir despliegue/activación y plan de migración de registros Legacy.

La implementación local no demuestra paridad completa, no está desplegada y no habilita automáticamente el módulo.
