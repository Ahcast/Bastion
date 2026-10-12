# Anulación de factura — Unidad 6, bloque 3

La acción conserva el correlativo, CAI, snapshots, ítems y montos. Nunca borra ni reutiliza un número emitido. `cancelled` es terminal y su estado comercial es nulo. Motivo y actor quedan en MySQL/auditoría; el PDF público posterior muestra «Factura anulada» sin motivo interno.

## Alcance inicial seguro

Sólo factura `issued` independiente o derivada de Cotización, con **cero filas de pago** y **cero documentos generados**, puede anularse. Una factura vinculada a OC requiere una transición coordinada con el propietario de OC y queda rechazada con 409 en este bloque. La presencia de pagos anulados o PDFs históricos también exige un flujo de conservación/corrección específico y se rechaza. No se modifica la Cotización de origen.

Un humano autorizado declara explícitamente que marcó «Anulado» y conserva original y copia conforme al procedimiento de la empresa. La declaración se registra como evidencia interna; Bastion no verifica físicamente esos papeles ni representa una notificación al SAR. La habilitación productiva depende de revisión fiscal y de los casos con pagos/OC/documentos ya difundidos.

Referencia pública consultada: [pregunta frecuente oficial del SAR sobre documentos fiscales con errores](https://www.sar.gob.hn/helpie_faq/que-hacer-en-caso-de-emitir-un-documento-fiscal-con-errores-en-el-llenado/), que remite al artículo 41 del Acuerdo 481-2017. La implementación no deduce de esa respuesta una regla universal para todos los casos comerciales.

## Datos y acción

Migración `0028_invoice_cancellation.sql`: `cancelled_at`, `cancelled_by`, `cancel_reason`, `cancellation_attested` y permiso `facturas.cancel`, sin concesión automática. DTO exacto `POST /api/v1/invoices/{id}/cancel` con `Idempotency-Key` y `{ "version": "2", "reason": "...", "original_and_copy_marked_and_archived": true }`. Versión positiva; motivo de 1..1000 caracteres. Sólo humano, `facturas.view` y `facturas.cancel` sobre el registro, reautenticación, CSRF, bloqueo, CAS, auditoría, evento e idempotencia dentro de una transacción. Replay vuelve a autorizar y no cambia contador ni versión. Respuesta `id`, `version`, `status`, `invoice_number`, `cancelled_at`.

422 para DTO/versión/motivo/declaración inválidos; 404 fuera de scope; 403 sin permisos/reauth; 409 para versión/estado/pago/documento/OC incompatible. La factura anulada se consulta y filtra, pero no se edita, emite, cobra ni clona. Documentos nuevos sólo permiten la factura marcada anulada, no nota/recibo. El PDF usa snapshots fiscales/empresa históricos y leyenda visible.

## Aceptación

MySQL 5.7, versión/replays/concurrencia, scopes/revocación/reauth/CSRF, rollback de auditoría, cero pagos y número/CAI/counter inmutables, visibilidad en listado y PDF marcado; rechazo de pagos incluso anulados, PDFs previos y OC vinculada; Angular y navegador real. Sin certificación fiscal productiva.
