# Módulo: Facturas

## Propósito

Emitir y administrar facturas, pagos/saldos y documentos derivados (PDF, nota de entrega y recibo), con origen desde cotización u orden.

## Estado de implementación

Corte de acciones del 2026-10-27: emisión numerada al marcar como enviado, pagos/saldos, clonación, documentos privados y eliminación limitada a borradores sin pagos. Ver [decisiones, reglas y validación](facturas-acciones.md); ver [alcance, diferencias y pendientes](facturas-implementacion.md) y [OpenAPI](facturas.openapi.yaml). La ficha siguiente conserva el alcance objetivo y no implica que esté todo implementado.

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
| Orden de Compra / Exenta No., Constancia de Registro No. y Registro SAG No. | `purchase_order_exempt_number`, `registration_certificate_number`, `sag_registration_number` | Referencias opcionales capturadas en Generales y mostradas en el PDF de factura. |
| Estado | `status` | Estados acordados: Borrador, Enviado y Pagado. Borrador es el estado inicial. |
| Referencias | `purchase_order_exempt_number`, `registration_certificate_number`, `sag_registration_number`, `client_request_id` | Referencias de orden/exoneración/registro/solicitud. |
| Ítem | descripción, cantidad, precio venta, costo, proveedor, descuento e impuesto | Calcula venta, costo e impuestos (exento/ISV 15/18). Sin tracking ni comentarios por confirmación del usuario del 2026-10-23. |
| Gastos | `amount`, `notes`, categoría pendiente | Gastos adicionales/inversión. |
| Notas | `notes`, `public_notes` | Notas privadas y públicas. |
| Pago | `amount`, método, referencia, fecha | Actualiza pagado/saldo; un pago parcial conserva el estado Enviado y el saldo cero lleva a Pagado. Método permitido por confirmar. |

## Ciclo de vida y pagos

Estados acordados: `Borrador`, `Enviado` y `Pagado`. Los pagos parciales no cambian `Enviado`; la factura pasa a `Pagado` solo cuando el saldo llega a cero. Confirmado el 2026-10-27: Marcar como enviado asigna número SAR y bloquea edición/eliminación; no envía correo. Si ya está liquidada, queda Pagado. Solo se eliminan borradores sin pagos, preservando origen. No se añade anulación ni se reutiliza numeración.

## Botones y acciones

Confirmación del usuario del 2026-10-23: cada fila ofrece un único botón desplegable con este orden exacto: **Ver, Editar, Marcar como enviado, Ingresar pago, Clonar, PDF, Nota de entrega, Emitir Recibo, Eliminar**. Las nueve opciones se habilitan por permisos, estado, saldo y disponibilidad de documentos; ver `facturas-acciones.md`.

Listado: Nueva factura, Ingresar pago (fecha/monto/método/referencia; Guardar pago/Cancelar), Marcar como enviado, Clonar, PDF, Nota de entrega, Emitir recibo, Eliminar, abrir/editar y paginación.

Editor: Guardar; Editar/Cancelar; Agregar/Quitar ítem; Agregar/Quitar gasto; Agregar/Quitar pago; confirmar pago de contado; PDF; volver. Si la factura es de contado, exige pago total antes de guardar.

Resumen recalculado: importes exonerado/exento/gravado, descuentos, ISV, pagado, saldo, inversión de productos/otros gastos, inversión total y utilidad estimada.

El PDF de factura, la nota de entrega y el recibo se generan con Typst mediante el servicio compartido del core. Se conservan las acciones y el flujo actuales; el módulo provee los datos de cada documento y define el permiso correspondiente. La plantilla `factura-v1.typ` diferencia factura, entrega y recibo; conserva empresa y SAR del momento de emisión.

Identidad visual confirmada el 2026-10-23: factura y nota de entrega usan el logo de Castillo IT suministrado como recurso compartido del Core, conforme a `../15-identidad-documental.md`. No se agrega firma/sello a estos documentos.

## Settings bajo Ajustes

La numeración fiscal/datos de empresa y SMTP son settings del Core identificados en el informe. Cada factura tendrá una sección propia bajo Ajustes; cualquier setting extra (defaults de términos, pagos, impresión) queda `Pendiente de decisión`.

## API, datos y permisos

API actual: CRUD, estado, pago, clonar, borrar, crear desde cotización/orden. Entidades factura, ítems y pagos. Aplicar la matriz Core: `ver` para consultar, listar, exportar y generar/descargar documentos autorizados; `editar` para crear, emitir, modificar, cambiar estado, registrar pago y clonar; `eliminar` para borrar. Cada comando además valida estado, saldo y relaciones; los pagos deben actualizar saldo y estado de forma atómica, con idempotencia y auditoría.

## Pendientes

Persisten la validación fiscal/contable externa, anulación/reembolsos y los contratos futuros de origen de Licitaciones/MCP. Métodos de pago admiten texto opcional como Legacy; no se inventa un catálogo. Emisión, bloqueo, numeración y borrado quedan definidos en `facturas-acciones.md`.

## Origen futuro: adjudicación de Licitaciones

La [incorporación documental de Licitaciones](licitaciones.md) propone crear borradores vinculados mediante un puerto propio de Facturas y el coordinador `document-flows`. No existe ese puerto ni un origen de adjudicación en el contrato vigente. El usuario confirma el 2026-10-27 que solo una adjudicación registrada habilita la creación y que admite un único borrador; la relación necesita unicidad persistente además de idempotencia de la petición. Deben acordarse cliente/instantánea y líneas/importes/impuestos antes de fijar DTO y migraciones; la política de corrección/borrado de ese borrador de Facturas sigue pendiente. La regla de Licitaciones que permite eliminar solo ofertas Borrador y bloquea las Enviadas no modifica la política global de este módulo. Requiere autorización de ambos dominios y respeta el corte actual sin emisión fiscal ni pagos; registrar una adjudicación no crea una factura automáticamente.

## Campos comerciales y fiscales — 2026-10-27

A solicitud del usuario, el editor muestra Fecha, Términos (Contado/Crédito), tipo y valor del descuento general, Orden de Compra / Exenta No., Constancia de Registro No. y Registro SAG No. Los tres números opcionales ya estaban en el contrato y almacenamiento: ahora se pueden capturar y se incluyen en el PDF de factura junto a los términos y el descuento. El editor advierte que, conforme al calculador existente, llenar cualquiera de esos tres números aplica exoneración a todos los ítems. El PDF ajusta los valores largos sin recortarlos.

Desplegado en AWS y Bluehost como `c6d2124a2b720e13d75eb7ac287cfd8b0aa60293`, con [CI aprobado](https://github.com/Ahcast/Bastion/actions/runs/40623822103), prueba de captura/guardado y cálculo exonerado con MySQL aislado, y revisión visual de PDF con valores normales y de 150 caracteres. Sin migraciones; se verificaron las facturas, pagos, ledger y activaciones antes y después del cambio. Respaldo privado previo en ambos hosts; frontend con 47 archivos y 45 hashes públicos verificados; readiness 200 y rutas privadas sin sesión 401. No se crearon datos financieros de prueba en producción.
