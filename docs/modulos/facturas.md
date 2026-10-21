# Módulo: Facturas

## Propósito

Emitir y administrar facturas, pagos/saldos y documentos derivados (PDF, nota de entrega y recibo), con origen desde cotización u orden.

## Campos

| Campo | Nombre técnico | Función/validación conocida |
|---|---|---|
| Buscar factura o cliente | `search` | Filtra listado. |
| Número de factura/orden | `invoice_number`, `order_number` | Número según configuración y referencia relacionada. |
| Cliente/manual | `id_client`, `client_name` | Selección o ingreso manual. |
| Fecha | `invoice_date` | Fecha de emisión. |
| Condición | `payment_term_type` | Contado o crédito. |
| Días de crédito | `credit_days` | Plazo del crédito. |
| Descuento | `discount_type`, `discount_value` | Monto o porcentaje. |
| Estado | `status` | Estados acordados: Borrador, Enviado y Pagado. Borrador es el estado inicial. |
| Referencias | `purchase_order_exempt_number`, `registration_certificate_number`, `sag_registration_number`, `client_request_id` | Referencias de orden/exoneración/registro/solicitud. |
| Ítem | descripción, cantidad, precio venta, costo, proveedor, tracking, comentarios, descuento e impuesto | Calcula venta, costo e impuestos (exento/ISV 15/18). |
| Gastos | `amount`, `notes`, categoría pendiente | Gastos adicionales/inversión. |
| Notas | `notes`, `public_notes` | Notas privadas y públicas. |
| Pago | `amount`, método, referencia, fecha | Actualiza pagado/saldo; un pago parcial conserva el estado Enviado y el saldo cero lleva a Pagado. Método permitido por confirmar. |

## Ciclo de vida y pagos

Estados acordados: `Borrador`, `Enviado` y `Pagado`. Los pagos parciales no cambian `Enviado`; la factura pasa a `Pagado` solo cuando el saldo llega a cero. La condición exacta que marca una factura como `Enviado` sigue pendiente: el informe distingue la acción manual “Marcar como enviado” del envío por correo. También falta definir cómo se corrige una factura después de enviarla; no asumir que puede editarse libremente ni inventar una acción de anulación.

## Botones y acciones

Listado: Nueva factura, Ingresar pago (fecha/monto/método/referencia; Guardar pago/Cancelar), Marcar como enviado, Clonar, PDF, Nota de entrega, Emitir recibo, Eliminar, abrir/editar y paginación.

Editor: Guardar; Editar/Cancelar; Agregar/Quitar ítem; Agregar/Quitar gasto; Agregar/Quitar pago; confirmar pago de contado; PDF; volver. Si la factura es de contado, exige pago total antes de guardar.

Resumen recalculado: importes exonerado/exento/gravado, descuentos, ISV, pagado, saldo, inversión de productos/otros gastos, inversión total y utilidad estimada.

El PDF de factura, la nota de entrega y el recibo se generan con Typst mediante el servicio compartido del core. Se conservan las acciones y el flujo actuales; el módulo provee los datos de cada documento y define el permiso correspondiente. Las plantillas y su versionado quedan por definir.

## Settings bajo Ajustes

La numeración fiscal/datos de empresa y SMTP son settings del Core identificados en el informe. Cada factura tendrá una sección propia bajo Ajustes; cualquier setting extra (defaults de términos, pagos, impresión) queda `Pendiente de decisión`.

## API, datos y permisos

API actual: CRUD, estado, pago, clonar, borrar, crear desde cotización/orden. Entidades factura, ítems y pagos. Aplicar la matriz Core: `ver` para consultar, listar, exportar y generar/descargar documentos autorizados; `editar` para crear, emitir, modificar, cambiar estado, registrar pago y clonar; `eliminar` para borrar. Cada comando además valida estado, saldo y relaciones; los pagos deben actualizar saldo y estado de forma atómica, con idempotencia y auditoría.

## Pendientes

Reglas fiscales, métodos de pago, condición que marca una factura como Enviado, corrección después del envío, numeración, correlación de conversiones, borrado y conciliación contable deben verificarse antes de implementar.
