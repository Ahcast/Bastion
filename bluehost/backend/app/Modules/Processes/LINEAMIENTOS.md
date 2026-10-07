# Módulo Procesos

## Objetivo

Convertir Procesos en el punto de entrada de oportunidades de HonduCompras, preservando filtros, marcas, notas y exportación legacy, pero sustituyendo CSV/FTPS/manual por ingestión API, versionado y automatización.

## Funcionalidad legacy a conservar

### Visualización y filtros

- tarjetas/bloques visuales de 10;
- entidad;
- keyword;
- búsqueda sobre especificaciones y expediente;
- orden asc/desc por fecha;
- vigencia;
- solo marcados;
- ocultar municipalidades;
- resumen de solicitud con fallback a especificaciones;
- fecha de recepción;
- expediente;
- especificaciones.

### Usuario

- marcar/desmarcar interés;
- nota personal;
- conservar marca/nota al actualizar la fuente.

### Administración

- status de fuente/importación;
- actualizar/importar;
- exportar CSV;
- resumen de leídas/nuevas/actualizadas/inactivas/errores;
- administración/depuración restringida.

## Debilidades legacy a eliminar

- crawler fuera de integración formal;
- FTPS + CSV como acoplamiento principal;
- importación manual;
- identidad hash ambigua;
- sin historial durable por ejecución;
- `active` confundido con vigencia;
- filtros/paginación en memoria;
- frontend limitado por primera página grande;
- shapes inconsistentes de marks/notas;
- purge físico destructivo;
- formula injection en CSV.

## Nueva ingestión

```mermaid
flowchart LR
    GH[GitHub Actions crawler] --> HC[HonduCompras]
    GH --> S3[(S3 RAW)]
    GH --> API[POST /api/v1/ingestion/processes]
    API --> DB[(MySQL)]
    DB --> EV[Eventos]
    EV --> AI[IA / automatizaciones]
```

El crawler reporta `crawler_run_id` y nunca escribe directo a MySQL.

## Modelo objetivo

```text
crawler_runs
processes
process_versions
process_documents
process_user_marks
process_user_notes
```

### crawler_runs

```text
id
source
started_at
finished_at
status
records_found
new_records
updated_records
unchanged_records
errors_count
raw_snapshot_key
```

### processes

```text
id
source
external_id
expediente
entity
title
received_at
closing_at
is_source_active
first_seen_at
last_seen_at
last_changed_at
current_version_id
created_at
updated_at
```

Restricción:

```text
UNIQUE(source, external_id)
```

Si la fuente no entrega ID confiable, la estrategia de identidad debe ser explícita y versionada.

### process_versions

```text
id
process_id
crawler_run_id
content_hash
normalized_data
raw_data
created_at
```

Solo se crea versión cuando cambia contenido significativo.

## Active vs vigencia

```text
is_source_active = apareció en la fuente esperada
is_current       = closing_at >= ahora según reglas de negocio
```

No son equivalentes.

## Marcas y notas

Contrato uniforme:

```json
{
  "is_interesting": true,
  "note": "..."
}
```

Son por usuario y sobreviven a nuevas versiones.

## API objetivo

```http
GET   /api/v1/processes
GET   /api/v1/processes/{id}
GET   /api/v1/processes/{id}/versions
PATCH /api/v1/processes/{id}/interest
PATCH /api/v1/processes/{id}/note

GET   /api/v1/processes/status
POST  /api/v1/processes/export

POST  /api/v1/ingestion/processes
POST  /api/v1/ingestion/runs
```

Filtros server-side:

```text
search
entity
interesting
source_active
current
hide_municipalities
direction
date_from
date_to
page
per_page
```

## Importar/Actualizar en UI

Para conservar la funcionalidad percibida:

- `Actualizar` refresca datos y estado;
- admin puede solicitar reingestión/reprocesamiento;
- puede existir importador CSV de emergencia separado;
- UI muestra última ejecución, éxito, encontrados, nuevos, cambiados y errores.

## Export CSV

Se conserva y debe:

- aplicar filtros server-side;
- exportar UTF-8;
- incluir metadata;
- neutralizar celdas que inicien con `=`, `+`, `-`, `@`;
- exigir autorización.

## Conexiones

```mermaid
flowchart LR
    P[Procesos] -->|crear borrador| Q[Cotizaciones]
    P --> I[Inicio]
    P --> AI[IA]
    AI -->|score/resumen/requisitos| P
    COL[Documentos/S3] --> P
```

`source_process_id` permite trazabilidad hacia Cotizaciones.

## IA

Jobs:

```text
classify_process
summarize_process
extract_requirements
calculate_relevance
match_internal_history
search_suppliers
suggest_bid_or_no_bid
```

Resultados IA se guardan por versión e `input_hash`. Nunca modifican el raw original.

## Seguridad

```text
procesos.view
procesos.mark
procesos.note
procesos.export
procesos.ingest
procesos.admin
```

Ingestión usa identidad machine-to-machine del crawler.

## Criterios de aceptación

- Filtros, marcas, notas y export continúan.
- Sin tope de 1000 filas en frontend.
- Reintentos no duplican procesos.
- Cambios generan versiones.
- Marks/notas sobreviven.
- `active` y `vigente` separados.
- CSV deja de ser mecanismo principal.
