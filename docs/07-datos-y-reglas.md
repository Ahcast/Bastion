# Datos y reglas de negocio

## Fuente de verdad

`01-aplicacion-actual-y-paridad.md` registra modelos, tablas confirmadas y reglas visibles. La copia disponible no contiene el esquema MySQL de negocio completo. Obtener un dump de estructura sin filas para cerrar tablas, columnas, claves, índices y restricciones.

## Entidades identificadas

- Core: usuarios, personas, roles, pantallas/objetos, asignaciones de permisos, estados y logs.
- Catálogos: clientes, proveedores, destinos.
- Compras: órdenes y detalle; estados, documentos y comentarios.
- Comercial: cotizaciones e ítems, contactos y adjuntos; facturas e ítems/pagos.
- Gastos: gastos generales y gastos de orden/factura.
- Procesos: filas importadas y marcas/notas.
- Colaboración: archivos/metadatos y alcance empresa/persona.

Es una lista conceptual. Confirmar nombres de tablas, relaciones y duplicidades con el esquema real.

## Reglas conocidas que deben conservarse

- Personas: solo el nombre completo es obligatorio. DNI, país y género son opcionales; si se proporciona DNI, debe ser único. El backend valida formato, catálogo y longitudes sin convertir los campos opcionales en requeridos.
- Usuarios y Personas: cada cuenta debe referenciar una Persona existente al invitarla/crearla; no se crea una Persona implícitamente desde el formulario de usuarios. La relación es uno a uno: una Persona no puede estar vinculada a más de una cuenta, incluso si la cuenta está desactivada. El Administrador puede reasignar la Persona en cualquier estado de la cuenta solo a una Persona libre; la Persona anterior se libera únicamente por esa reasignación y el cambio queda auditado. Aplicar clave foránea e índice único en la relación para garantizar existencia y unicidad incluso con operaciones concurrentes.
- Cuentas de usuario: no se eliminan físicamente desde la aplicación; se desactivan para conservar el historial y las referencias de auditoría.
- Correo de acceso: un cambio requiere confirmar la nueva dirección antes de sustituir la actual; hasta confirmarse, el correo vigente conserva el acceso y la recuperación. La dirección confirmada debe ser única según normalización acordada para el esquema.
- Invitaciones: desactivar una cuenta con invitación pendiente revoca inmediatamente el enlace; al reactivar la cuenta debe emitirse y enviarse una invitación nueva, con vencimiento contado desde el envío exitoso.
- Roles: el nombre debe tener contenido tras normalizar espacios y no duplicar otro rol al crear o editar. La descripción es opcional; límites concretos deben concordar con el esquema MySQL.
- Pantallas/objetos: validar campos, tipo permitido, identificador/relaciones y unicidad en backend; los módulos de la nueva aplicación se registran con ID estable en el catálogo del Core.
- Permisos: `ver` consulta, `editar` crea y actualiza, y `eliminar` borra/purga. La autorización se comprueba por objeto/tenant en cada operación; UI, API y MCP comparten el mismo servicio.
- Cotizaciones admiten descuentos y tipos de impuesto (exento, ISV 15%, ISV 18%) y cálculo de totales.
- Facturas usan los estados `Borrador`, `Enviado` y `Pagado`. Los pagos parciales conservan `Enviado`; la factura pasa a `Pagado` solo cuando el saldo llega a cero. Factura de contado exige pago total para guardar. La condición que marca `Enviado` y la corrección después del envío siguen pendientes.
- Conversión cotización→orden/factura y orden→factura crea documentos derivados relacionados.
- Marcar una orden entregada solicita número de factura.
- Reporte de órdenes presenta venta, inversión, ISV, inversión de productos, otros gastos y utilidad.
- Gastos exigen número de factura, fecha y proveedor; importe gravado o exento debe ser mayor que cero.
- Importación de procesos acepta CSV, permite marcas/notas y exportación CSV.
- Envío de cotización puede ser inmediato o programado con adjuntos.

## Decimales, impuestos y moneda

Mostrar y contabilizar importes con precisión monetaria (sin `f32`/`f64` como fuente de verdad). Confirmar reglas de redondeo, base imponible, exoneración y moneda con ejemplos de producción anonimizados y verificación contable. HNL es la moneda observada; la conversión o multi-moneda no se presume.

## Persistencia

Bluehost usa MySQL 5.7, confirmado por el usuario, y es el host acordado inicialmente. Evaluar aislamiento por módulo y evitar que las escrituras JSON paralelas sean fuente de verdad sin transacciones/consistencia definida. Definir índices, claves foráneas, concurrencia y estrategias de backup. Confirmar la versión de mantenimiento exacta y las capacidades de TLS del plan de Bluehost.

Los settings del Core y módulos se guardan en una tabla común por módulo, alcance, clave y versión de esquema. MySQL 5.7 ofrece el tipo nativo `JSON`; se usará para el valor estructurado, mientras que Rust valida el contenido contra el esquema declarado. Los campos secretos se cifran antes de persistirlos y no se devuelven en claro. No asumir restricciones o funciones de versiones posteriores; confirmar la versión de mantenimiento exacta de Bluehost y probar el esquema/migraciones sobre esa versión.

## Retención acordada

La zona horaria compartida del sistema es `America/Tegucigalpa`. Las marcas de tiempo se almacenan en UTC; las horas se convierten a la zona acordada al mostrarse y al ejecutar tareas programadas. Los valores de fecha sin hora conservan su fecha de calendario.

- Historial de accesos e IP vinculadas a autenticación/eventos de seguridad: 30 días.
- Bitácora de acciones administrativas y de negocio: 1 año desde cada evento. Después se elimina la entrada de auditoría vencida.
- La retención de la bitácora no elimina registros de negocio, usuarios desactivados ni documentos originales. Los PDFs/adjuntos siguen ligados al registro asociado y no caducan automáticamente; su eliminación ocurre junto con el registro mediante su flujo autorizado.
- El Administrador del sistema consulta la bitácora en `Ajustes > Core > Bitácora` con filtros por fecha, actor, módulo, acción y resultado; es de solo lectura.
- PDFs y adjuntos no tienen vencimiento automático mientras exista el registro asociado. Al borrar el registro mediante una operación autorizada, confirmada y auditada, se eliminan sus archivos asociados.

## Migración

Antes de migrar: inventario y exportación de esquema, conteos, perfiles de datos anonimizados, mapeo de IDs, conversión de archivos/manifiestos, reconciliación de importes y plan de reversión. No copiar datos productivos en documentación.

## Pendientes

Esquema completo MySQL, versión de mantenimiento exacta de Bluehost, estados válidos, reglas fiscales actualizadas, fórmulas exactas/redondeo, integridad de conversiones, CSVs de ejemplo anonimizados y retención de registros de negocio que requiera una regla específica por módulo. MySQL 5.7 entró en Sustaining Support de Oracle en 2023; revisar con Bluehost la actualización soportada antes de producción.

## Órdenes: precisión y catálogo nuevo

El corte local conserva `sale_price` como costo y `purchase_price` como venta, según el controlador Legacy. Usa enteros escalados para agregar bases antes de redondear; ISV 15%, gastos a centavos e inversión/total persistidos atómicamente. No presupone validación contable ni IDs Legacy. Destinos tiene identificadores nuevos y pertenece a Órdenes por confirmación del usuario del 2026-10-23; no se importa catálogo real. Ver `modulos/ordenes-de-compra-implementacion.md`.

Colaboración conserva metadatos en MySQL y bytes en el bucket privado S3 del Core. `trashed_at` y `purge_after` fijan la retención; la limpieza elimina versiones del objeto exacto y deja recibo mínimo auditable.

## Gastos: persistencia del primer corte

El primer corte guarda el registro general en `gastos_records`, `gastos_create_receipts` y `gastos_actor_limits`; no reutiliza JSON Legacy ni crea asociaciones a órdenes/facturas. HNL es el único valor habilitado por ser el único observado. El cotejo estático de GastosController confirma bases a centavos antes del ISV y total; queda pendiente la revisión contable de negocio. La migración 2 conserva los registros eliminados mediante deleted_at; consulta y resumen excluyen esos registros. Ver [implementación y evidencia](modulos/gastos-implementacion.md).

Órdenes conserva identificadores internos estables de productos para asociar un archivo privado por línea. El módulo posee ordenes_documents, sus recibos y cola de limpieza. Clonar asigna identidades nuevas y excluye archivos. document-flows posee la relación única orden/factura y su instantánea; Facturas posee source_order_json y crea su borrador mediante puerto de dominio. No se inventa numeración fiscal ni se modifica n_bill al convertir.

## Licitaciones: entidades de diseño

El nuevo dominio comprende expediente, lotes/ítems, oferta, valores de variables, copias/revisiones de trabajo, formularios, anexos y versiones PDF. Adjudicaciones y relación con Facturas requieren reglas posteriores. La [infraestructura inicial](modulos/licitaciones-infraestructura.md) implementa sólo expedientes, copias de esquemas/valores y recibos en tablas propias; Colaboración conserva familias, versiones publicadas y recibos de esquemas. Las tablas del documento fuente no se adoptan automáticamente. Una empresa por base, sin campos de negocio precargados ni multimoneda implícita. Los decimales viajan como cadenas y no se calculan importes hasta incorporar sus reglas.

Colaboración conserva las futuras definiciones/plantillas reutilizables; los valores y copias del expediente son independientes. Versiones finales congelan entradas, referencias y hashes. El borrado del original no elimina la copia; definir retención, propiedad y recuperación MySQL/S3 antes de programar. El usuario confirma copias visibles solo en el expediente y un único borrador por adjudicación: el futuro contrato debe respaldar esa unicidad, fijar la propiedad de relación/recibos y conservarla ante concurrencia o claves de reintento distintas.

Decisiones adicionales del 2026-10-27: formularios y formatos incorporados individualmente, con aprobación/publicación de plantillas por el Administrador y edición de usuario limitada a sus valores de variables. La creación del borrador requiere adjudicación registrada. Todas las páginas del PDF, incluidas portada/índice, se folian y el índice se genera automáticamente. Solo se elimina el expediente/oferta Borrador; Enviada no se elimina y la comprobación se coordina transaccionalmente con el envío. La corrección posterior y las referencias a borradores de Facturas requieren reglas específicas.
