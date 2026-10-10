# Contrato de Procesos — unidad 3

Fuente maestra MySQL 5.7 en Bluehost. Crawler Python conservado sin cambios. Contrato wire 1.0 definido en backend/crawler/docs/IMPORT_CONTRACT.md. El código implementado y la evidencia de aceptación se registran en status.md; este contrato no acredita despliegue.

## Ingestión

POST /api/internal/v1/crawler/runs recibe run_id (1–100 caracteres ASCII alfanuméricos, guion o guion bajo), started_at RFC3339 y source=honducompras. Clave igual run_id. Identidad bearer y scopes crawler.runs.create, crawler.processes.import, crawler.runs.complete; usuario propietario activo con TOTP y grants all correspondientes. Sesión humana no ingiere. Tokens revocados o permisos retirados se rechazan incluso para replays. Estas capacidades no autorizan escrituras MCP comerciales.

POST /runs/{run_id}/processes recibe batch_id, schema_version=1.0, processes (1–100). Clave igual batch_id. Límite HTTP 24 MiB y cuatro importaciones concurrentes; 429 se reintenta sin repetir scraping. Cada observación contiene source, external_id, source_url, observed_at, fields_present, data, documents_complete, completeness, content_hash, raw_sha256. Campos data admitidos: entity, reference, deadline, description, documents. Documents: url, name, sha256 nullable, download_status opcional PENDING/DOWNLOADED. Campos desconocidos rechazados. URLs http/https sin credenciales; no se descargan. Checksums SHA256 hex; content_hash recibido verifica el JSON canónico Python de la observación, distinto del hash servidor del estado combinado. Se conserva el cuerpo de evidencia en MySQL; raw bytes permanecen S3, sin afirmar validación del bucket.

Fusión por timestamp UTC por campo, empate con diferente contenido => 409; campo ausente no borra. Una observación explícita vacía más nueva sí reemplaza. Documentos parciales no eliminan; hash confirmado no se degrada por PENDING ni SHA ausente. Completo puede retirar documentos si su observación es posterior al catálogo y a cada documento conocido. No hay inactivación por ausencia. first_seen MIN, last_seen MAX. Nueva versión/diff únicamente ante cambio significativo del estado combinado; no por estados técnicos. Evidencia atrasada permanece durable. Procesos existentes conservan propietario/equipo/estado interno y marcas/notas.

Identidad UNIQUE(source,external_id), algoritmo oficial/URL del crawler conservado. El backend no vuelve a interpretar external_id ni crea Clientes. Creación inicial propiedad del usuario vinculado al token; reasignación futura exige administración explícita. Ingestión de fuente requiere scope all, no limita por propietario comercial.

Run, lote, observaciones, estado combinado, versiones/cambios, auditoría y hechos domain_events se confirman transaccionalmente. No se encolan jobs AI sin consumidor implementado. Clave repetida/cuerpo igual => ACK guardado después de autorizar; cuerpo distinto => 409. Un rechazo de registro revierte todo el lote. No ACK 202. ACK IMPORTED: batch_id/status/received/new/updated/unchanged/errors=0; contadores enteros. Complete conserva cada cierre inmutable y permite nuevos lotes del mismo run; nunca marca ausencia como borrado. Complete valida totales contra los lotes importados de su prefijo y metadatos de extracción, sin confiar en contadores libres.

## Lectura y usuario

Procesos expone GET /api/v1/processes, /{id}, /{id}/versions; PATCH /{id}/interest y /{id}/note con version string e Idempotency-Key. Marcas/notas personales separadas y versión propia; permisos procesos.view/mark/note all/self/team se reevaluan por registro. IDs BIGINT string. Listados SQL paginados 1–100 con total, filtros search/entity/interesting/source_active/current/hide_municipalities/direction/date_from/date_to; fechas límite válidas se comparan UTC, inválidas quedan fuente textual y no se inventa vigencia. CSV servidor autorizado y neutralizado, máximo explícito 10000 resultados y 8 MiB por exportación; un exceso devuelve 422 y exige filtros, sin recortar; estado de ingestión protegido procesos.admin. Raw externo no habilita acciones ni se renderiza como HTML.

## Dependencias conservadas

Quotes implementará conversión determinista cliente/expediente/source_process_id sin copiar ítems; AI/Workflow y Colaboración implementarán investigación, resultados, tareas/comentarios/aprobaciones. No introducir endpoints simulados ni activar envíos. Esta unidad no acredita esas integraciones ni S3/Bluehost productivos.

## Aceptación

MySQL 5.7 real: replay/concurrencia/rollback, lotes inválidos, orden por campo, documentos parciales/hash confirmado, cierres múltiples, ausencia, revocación/scopes/IDOR y cadenas >2^53. Pruebas REST autenticación y cliente Python wire sin cambios, Angular/navegador y regresión Core/Clientes/Proveedores/crawler. No comenzar Cotizaciones hasta cerrar todas las pruebas y CI.
