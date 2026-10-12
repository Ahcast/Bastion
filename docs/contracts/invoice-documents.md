# Documentos generados de Facturas — Unidad 6, bloque 2

Este contrato implementa persistencia, REST y UI sobre la proyección pública y renderer puro definidos en `docs/contracts/invoices.md`. No incluye anulación ni clonación de factura; esos pertenecen a bloques posteriores de la unidad 6.

## Propiedad y privacidad

Core.Files es el único propietario de metadatos descargables y de la evidencia PDF en almacenamiento privado. El módulo Facturas conserva únicamente el snapshot público que produjo los bytes, su identidad, estado y vínculo al documento Core. Nunca se publica `object_key`, `public_data`, costos, inversión, utilidad, actores/equipos, razones internas, saldos o datos de origen.

Los documentos generados usan `documents.entity_type='invoices'` y `entity_id=<invoice_id>`. La política de documento de Facturas exige alcance vigente sobre la factura; la descarga continúa pasando por Core y su permiso `core.documents.read`.

## Tipos

- `invoice`: borrador o emitida. Borrador usa empresa pública actual y versionada capturada al reservar. Emitida usa exclusivamente snapshots históricos de empresa/SAR de la factura.
- `delivery_note`: sólo factura emitida; no contiene precios, impuestos, totales ni SAR y no afirma entrega física.
- `receipt`: sólo factura emitida y un pago real de esa factura. Requiere `payment_id` y `payment_version`; usa número público `RCP-{payment_id}`. Un pago anulado sigue generando `Recibo anulado` sin publicar la razón interna.

El renderer nunca vuelve a leer factura, pago, configuración ni empresa: renderiza únicamente `public_data` reservado.

## Persistencia

Migración `0027_invoice_documents.sql` crea `invoice_document_snapshots` con: factura, versión observada, pago/version opcionales, tipo, hash SHA-256 de la proyección pública, `public_data`, `object_key`, documento Core opcional, estado `pending|available`, actor y timestamps. `UNIQUE(invoice_id,kind,input_hash)` evita duplicar la misma evidencia pública con distintas llaves idempotentes.

Un snapshot `pending` es reanudable: si la escritura inmutable ya ocurrió pero la transacción final falló, el mismo snapshot se vuelve a renderizar de forma determinista y `PutImmutable` debe aceptar exactamente los mismos bytes/checksum. `available` es terminal e inmutable.

Permiso nuevo: `facturas.documents.generate`, sin concesión automática por migración. Generar también exige `facturas.view`, alcance por registro y `core.documents.write`. Listar exige `facturas.view`; abrir/descargar el documento exige además `core.documents.read` mediante Core.

## REST

### POST `/api/v1/invoices/{id}/documents`

`Idempotency-Key` obligatorio.

DTO exacto:

```json
{
  "kind": "invoice|delivery_note|receipt",
  "version": "7",
  "payment_id": null,
  "payment_version": null
}
```

- `version` debe coincidir con la versión observada de la factura al reservar.
- Para `receipt`, `payment_id` y `payment_version` son obligatorios y coherentes.
- Para los otros tipos ambos deben ser `null`.
- La reserva ocurre en TX: reautoriza, bloquea factura, carga ítems/pago, captura empresa pública si es borrador, construye/valida la proyección y guarda `public_data` + hash.
- Fuera de TX: renderiza el snapshot reservado y hace `PutImmutable`.
- La finalización ocurre en segunda TX: reautoriza, comprueba que el snapshot sigue perteneciendo a la factura, registra el PDF con `Core.RegisterGeneratedFile`, marca `available`, audita y emite evento.
- Replays vuelven a autorizar. Un snapshot `available` devuelve el mismo resultado sin regenerar metadata.

Respuesta:

```json
{
  "id": "snapshot_id",
  "document_id": "core_document_id",
  "kind": "invoice|delivery_note|receipt",
  "status": "available"
}
```

### GET `/api/v1/invoices/{id}/documents?page=1&per_page=25`

Devuelve sólo metadata segura: `id`, `document_id`, `kind`, `status`, `invoice_version`, `payment_id`, `payment_version`, `created_at`, `available_at`. No devuelve `public_data`, `input_hash` ni `object_key`.

## Autorización y errores

- 422: DTO, tipo, paginación, IDs/versiones o combinaciones de campos inválidas.
- 404: factura/pago inexistente o fuera de alcance; snapshot ajeno.
- 403: falta permiso de generación/lectura documental o actor no humano donde corresponda.
- 409: versión antigua, pago incoherente, snapshot/cálculo histórico corrupto o cambio de estado que invalida el tipo.
- integración de archivos/store ausente: módulo deshabilitado para generación sin degradar silenciosamente.

La segunda TX reautoriza para evitar publicar metadata si permisos/scopes fueron revocados entre reserva y escritura de bytes.

## UI y aceptación

Detalle de factura muestra panel **Documentos**. Acciones:
- Factura PDF: borrador o emitida.
- Nota de entrega: sólo emitida.
- Recibo: por cada pago, incluyendo anulado con etiqueta correspondiente.

La lista muestra tipo, estado/fecha y acción de abrir/descargar sólo cuando `available`. Errores conservan la intención del usuario y nunca muestran datos internos.

Aceptación mínima: MySQL 5.7 real; HNL/USD; exactitud decimal >2^53; draft con empresa actual versionada; emitida inmune a cambios posteriores de empresa/SAR; recibo parcial/anulado; nota sin finanzas; privacidad de `public_data`; replay idempotente; concurrencia; fallo/reintento de storage; revocación antes de finalización; scopes; Core document policy; tests frontend y regresión navegador. No se inicia anulación/clonación hasta que este bloque quede verde.