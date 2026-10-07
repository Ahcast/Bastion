# Contrato de integración Bluehost — pendiente de implementación en Laravel

Base HTTPS: /api/internal/v1/crawler. Identidad crawler-lightsail, bearer token
con scopes crawler.runs.create, crawler.processes.import, crawler.runs.complete.
No abrir MySQL a Internet. Versiones de datos en MySQL, nunca como maestro en SQLite.

POST /runs: run_id, started_at, source. Idempotency-Key=run_id.
POST /runs/{run_id}/processes: batch_id, schema_version=1.0, processes (máximo 100).
Idempotency-Key=batch_id. Validar que reutilizar una clave con distinto cuerpo falla.
POST /runs/{run_id}/complete: batch_id, extraction, import.
Un run parcial puede tener varios lotes inmutables; complete registra cada cierre,
no debe impedir nuevas observaciones posteriores del mismo run.

## Importación transaccional
1. Bloquear/crear registro de import_batch con índice único por Idempotency-Key.
2. Si ya está importado y hash de solicitud coincide, retornar ACK persistido.
3. UNIQUE(source, external_id). source_url permite auditoría; identity fallback
   usa URL canónica. No cambiar algoritmo de identidad sin migración explícita.
4. Sólo combinar fields_present; ausencia NO significa vacío.
5. Aplicar orden por observed_at por campo: un lote antiguo NO debe revertir
   información más reciente. Conservar observaciones atrasadas como evidencia.
6. documents_complete=false nunca permite DOCUMENT_REMOVED. sha256 ausente o
   download_status=PENDING no reemplaza el hash confirmado de un documento.
7. Normalizar el estado combinado en el servidor; calcular hash del contenido de
   negocio, excluyendo observed_at, download_status y demás estados técnicos.
   El content_hash recibido describe la observación, no sustituye este cálculo.
8. Si cambió contenido relevante: insertar process_versions, diff y tipos de
   cambio; actualizar processes y crear evento en outbox para AI en la misma
   transacción. Si no cambió: actualizar last_seen (MAX), no crear versión.
9. first_seen=MIN observaciones, last_seen=MAX. No eliminar por ausencias.
10. Guardar ACK y confirmar transacción antes de responder. El ACK de recepción
    HTTP 202 no equivale a importación: el cliente de esta versión exige 200/201
    con IMPORTED. Un importador asíncrono necesita añadir polling al cliente.

ACK del endpoint de procesos:
```json
{"batch_id":"ID-0","status":"IMPORTED","received":2,"new":1,"updated":1,"unchanged":0,"errors":0}
```
ACK complete:
```json
{"batch_id":"ID","status":"COMPLETED"}
```
ACK FTPS: mismo esquema IMPORTED, pero batch_id del lote completo.
Una falla por registro revierte el sublote; devolver rechazo sin ACK IMPORTED.
Registrar los rechazos en backend. El cliente conserva y reintenta ese sublote.

## Datos externos
Tratar descripción/HTML/documentos como contenido no confiable. No ejecutar HTML,
macros ni instrucciones AI. Escapar CSV al abrir en hojas de cálculo; processes.csv
preserva texto fuente, no es una hoja sanitizada. Entregar a AI mediante contexto
no confiable y permisos delimitados en Bastion.
