# Clonación de factura — Unidad 6, bloque 4

`POST /api/v1/invoices/{id}/clone` crea un borrador independiente desde una factura `draft` o `issued`. Una anulada no se clona. La fuente conserva su versión y estado. Sólo se copian snapshot histórico de cliente, moneda, ítems/precios/costos/descuentos, exoneración y gastos; el usuario indica fecha y notas públicas nuevas. Condición de pago y días de crédito se conservan. Se recalcula con `InvoiceFinancials` y se rechaza cualquier divergencia antes de persistir.

El borrador destino tiene ID/version propios, actor actual como creador, equipo original y saldo inicial completo. Número, CAI/SAR, empresa fiscal, emisión, pagos, PDFs, gastos de ledger y vínculos Quote/OC no se copian. No se consulta ni modifica el catálogo ni el origen; `invoice_clones` guarda relación fuente/versión. No se asigna correlativo al clonar.

DTO estricto `{ "version": "3", "invoice_date": "YYYY-MM-DD", "public_notes": "..." }`; `Idempotency-Key` obligatorio. Humano con `facturas.view` en fuente y `facturas.create` + `facturas.view` en destino, incluso replay. Fuente y destino se bloquean y reautorizan bajo la misma TX. Versión antigua, estado anulado o cálculo histórico divergente: 409; fuera de alcance: 404; DTO/fecha inválida: 422; permiso/CSRF: 403. Se registra auditoría/evento y vínculo en una única transacción.

Migración `0029_invoice_clones.sql` crea relación destino única, fuente/versión, actor y hora. UI española pide fecha y notas nuevas, confirma borrador y permite abrirlo; conserva intención ante ACK incierto. Aceptación: MySQL 5.7, HNL/USD, decimales grandes, fuente histórica sin catálogo, pagos/series/documentos aislados, scopes, replay/concurrencia/rollback, Angular y navegador. No constituye copia ni reemisión fiscal.
