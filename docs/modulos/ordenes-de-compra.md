# Módulo: Órdenes de compra

## Propósito

Registrar y seguir órdenes, productos, gastos asociados, documentos, estados, entrega y conversión a factura.

## Pantallas y campos

| Pantalla | Campo | Nombre técnico | Requerido/validación conocida |
|---|---|---|---|
| Listado | Buscar orden, factura o cliente | `search` | Opcional. |
| Listado | Cliente, estado | `id_client`, `id_estado` | Filtros opcionales. |
| Listado | Valor mínimo/máximo | `min_total`, `max_total` | Filtros numéricos opcionales. |
| Cabecera | Número de orden/factura | `n_orders`, `n_bill` | Número de orden; factura opcional/asociada. |
| Cabecera | Fechas de orden y entrega | `order_date`, `received_date` | Orden requerida al crear; entrega opcional. |
| Cabecera | Cliente o cliente manual | `id_client`, `client_name` | Uno requerido, confirmar reglas exactas. |
| Cabecera | RTN, destino, estado, comentarios | `client_rtn`, `id_destination`, `id_estado`, `comments` | Destino/estado dependen de validación/catálogos. |
| Producto | Descripción, cantidad, venta, costo | `description`, `cantidad`, `sale_price`, `purchase_price` | Cantidad/precios numéricos. |
| Producto | Proveedor, tracking, ISV | `id_supplier`, `tracking`, `isv` | Opcionales según línea/flujo. |
| Adjuntos | Archivo | Documento | Un archivo privado por producto, hasta 10 MiB; descarga autenticada. |

## Botones y efectos

| Acción | Resultado |
|---|---|
| Nueva orden | Abre formulario de creación. |
| Aplicar/Limpiar filtros | Actualiza/restaura búsqueda y filtros. |
| Abrir/Editar | Navega al detalle y edición. |
| Guardar | Crea/actualiza orden y productos; recarga estado. |
| Agregar/Quitar producto | Añade/elimina línea y recalcula totales. |
| Agregar/Quitar gasto | Actualiza gastos e inversión asociada. |
| Adjuntar/Ver/Descargar/Eliminar documento | Administra documentos de la orden. |
| Cambiar estado de línea/orden | Valida estado y actualiza la entidad. |
| Marcar entregada | Solicita número de factura y cambia estado si está presente. |
| Convertir a factura | Crea factura relacionada. |
| Clonar | Crea copia de la orden. |
| Eliminar | Confirma; extensión de detalle agrega confirmación escrita reforzada. |
| Volver/paginación | Regresa al listado o cambia página. |

Edición en línea presenta cálculos de venta, ISV, inversión de productos, otros gastos, inversión total y totales.

## Settings bajo Ajustes

Settings específicos no están identificados como tales en la aplicación actual. Sección del módulo bajo Ajustes: sí. Contenido (defaults de estados/destinos/proveedores/términos y numeración): `Pendiente de decisión`; no inferirlo sin revisar configuración existente.

## Datos, API, permisos

API actual: CRUD parcial, listado/detalle, documentos, estados, entrega, clonación y borrado. Datos principales: `orders`, `orders_detail`, estados, comentarios y gastos vinculados. Aplicar la matriz Core: `ver` para lectura/listados; `editar` para crear, modificar, adjuntar, cambiar estado, marcar entrega y clonar; `eliminar` para borrar órdenes/documentos. Cada operación verifica el registro y estado de dominio; las acciones destructivas requieren confirmación y auditoría.

## Pendientes

Campos completos, fórmulas/redondeo, transición de estados, tipos/tamaño de adjunto, alcance del borrado, relación exacta de gastos y comportamiento de conversión deben confirmarse con UI/esquema.

## Implementación local (2026-10-23)

Primer corte de listado/filtros, detalle, alta/edición, cálculo servidor, estados derivados y gastos. El usuario confirma agregar el catálogo de destinos en Ajustes > Órdenes de compra; sus campos son identificador y descripción, con alta/renombrado versionados. Es una ampliación autorizada, no un CRUD confirmado del Legacy. Los demás ajustes quedan pendientes. Ver [alcance, diferencias y pruebas](ordenes-de-compra-implementacion.md) y [OpenAPI](ordenes-de-compra.openapi.yaml). La publicación técnica fue verificada en AWS/Bluehost desde 59a8057, con el módulo disponible sin activar. La paridad sigue abierta.

## Acciones de documentos y conversión

Implementadas clonación, archivos privados por producto y conversión única a borrador de factura. El alcance, límites y diferencias fiscales están en [acciones de órdenes](ordenes-de-compra-acciones.md). La eliminación completa de órdenes y la emisión fiscal permanecen pendientes. Disponibilidad publicada y despliegue deben verificarse por separado.

El 2026-10-27 se desplegaron clonación, archivo privado por producto y conversión a borrador de factura con utilidad sin ISV desde 50efa7e. La activación existente de Órdenes y Facturas se conservó; no se añadieron permisos ni se activaron otros módulos. Ver [acciones y evidencia](ordenes-de-compra-acciones.md).
