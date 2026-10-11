# Contrato propietario de Facturas/pagos

Continúa después de las seis unidades OC certificadas en a36de697a4c0d39d16f7095a76d443258deec3f5 (Core checks 38175683879). Especificación funcional: ../modules/invoices.md. Desarrollo incremental en orden: (1) InvoiceFinancials y saldos exactos; (2) borradores, persistencia, REST/UI y autorización; (3) configuración fiscal, correlativo y emisión; (4) pagos auditados y proyección comercial; (5) conversiones por propietarios; (6) documentos, anulación y clonación; (7) aceptación integral. Cada unidad se certifica antes de la siguiente.

## Unidad 1: motor financiero y proyección de pagos

Código interno de dominio, sin rutas HTTP/MCP ni acciones expuestas. No crea tablas ni migración, permisos, registros, series, estados de factura ni documentos. No simula un módulo operativo. La siguiente unidad debe definir migración/DTO/permisos/estados/errores y aceptación antes de introducir persistencia y API. Core no importa Invoices.

CalculationInput contiene currency obligatoria HNL|USD, items[] del contrato finance (1..200), general_discount_type amount|percent, general_discount, exoneration_reference y additional_expenses. Decimales como strings; cantidades/precios/costos/descuentos conservan escalas, redondeo y distribución certificados de finance.Calculate. No acepta totales calculados por cliente.

Resultado: moneda explícita, ítems normalizados y cálculo con gross, item_discount, general_discount, base_15, base_18, exempt, exonerated, isv_15, isv_18, total, products_investment, additional_expenses, investment_total, profit, calculator_version=1 y profit_version=1. Inversión total = costo de productos redondeado por línea + gastos explícitos. Utilidad = bases netas sin ISV − inversión total, mediante finance.Profit. Negativa permitida. No cambiar reglas fiscales ni sumar monedas; ningún número fiscal/SAR se obtiene de este cálculo. El PDF público futuro seleccionará campos comerciales explícitos, sin costos/utilidad privados.

PaymentProjection devuelve currency, paid_total y balance. ProjectPayments recibe total de una factura y exclusivamente importes de filas de pagos activos de su misma moneda; el servicio propietario futuro debe filtrar anulados y leer bajo la transacción adecuada. No recibe pagos desde UI como autoridad, no escribe ledger, no emite recibo y no decide invoice.status. Con cero pagos, pagado 0.00 y saldo igual al total (también para total cero).

ApplyPayment valida total/paid/amount antes de sumar. Importe estrictamente positivo; pagado actual no negativo y no mayor al total. Suma exacta, sin tolerancias, FX, retenciones automáticas ni divisiones por 1.15. Rechaza sobrepago. Devuelve nueva proyección; registrar un pago real requerirá permiso facturas.payment, humano/aprobación correspondiente, reauth, lock/versión, idempotencia, auditoría/evento y fila payments atómicos en su unidad.

Errores: Core Invalid (422 en una API futura) para moneda, decimal, escala, importe cero/negativo o formato inválido; Conflict (409 futuro) para saldo incoherente/sobrepago. Sin cambios de estado por ejecutar funciones puras.

Aceptación: paridad con ventas canónicas en impuestos mixtos, exoneración, descuentos, costo por línea y gastos; pérdidas; HNL/USD separados; límites y decimales inválidos; abonos exactos, pago completo, cero pagos, sobrepago y cantidades monetarias superiores a 2^53. Go fmt/vet/race y regresión MySQL, Angular y navegador en CI; ninguna prueba acredita emisión fiscal o integración productiva.

## Dependencias fiscales y siguientes contratos

Borradores no asignarán invoice_number ni incrementarán contador. Emisión requiere una fuente transaccional de serie/rango/SAR, validez y snapshot histórico; no inventar configuración real ni retroceder contador. Pagos son filas propietarias payments, no arrays JSON persistidos. Conversión OC/Quote comparte TX de ambos servicios y preserva origen/versión con permiso explícito. No habilitar Facturado/Cerrado en OC sin estos efectos. Anulación, vencimiento y recibo por pago real se definirán con pruebas antes de exponerlos. Configuración, credenciales y validación fiscal productiva siguen pendientes; el desarrollo sintético autorizado continúa.
