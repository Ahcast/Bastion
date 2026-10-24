# Cotización aprobada → orden de compra

Confirmación del usuario (2026-10-23): la orden representa la cotización aprobada por el cliente. Se conservan productos, cantidades decimales, descuentos, ISV 15/18%, exoneraciones, notas, términos e importes. El precio se copia inicialmente a costo y venta según el mapeo Legacy; el costo sigue siendo operativo y editable.

La conversión requiere estado Aprobada. Preparar/cancelar no cambia documentos. Guardar registra origen y revisión, crea la orden y marca la cotización Convertido atómicamente. Se representa una cotización mediante una orden; los reintentos devuelven la operación original. Dividir una cotización en varias órdenes queda fuera de este flujo.

La instantánea comercial aprobada es inmutable en la orden; se pueden editar datos operativos, costos, gastos, tracking y estado de productos. El origen no se borra si tiene una conversión registrada. Los documentos anteriores y su cálculo Legacy continúan siendo legibles.

La composición vive en bastion-workflows. Cotizaciones y Órdenes exponen puertos internos sobre una transacción compartida y conservan sus tablas. El adaptador HTTP no escribe SQL de dominio. Se bloquean los registros de módulos en orden Cotizaciones→Órdenes; se comprueban habilitación, permisos, revisión e idempotencia. Cancelar o fallar revierte toda la operación.

El límite HTTP de Órdenes y de las rutas de conversión es 3 MiB para alojar productos y su instantánea; Cotizaciones ordinarias conserva 1 MiB, y adjuntos su límite separado. Se conservan 200 productos, 200 gastos, cuota 120/minuto/actor por módulo y plazo de 30 segundos.

Ver contrato OpenAPI cotizacion-a-orden.openapi.yaml. No se copian archivos, ni se implementa emisión de factura o MCP en este avance.
