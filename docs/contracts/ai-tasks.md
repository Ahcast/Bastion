# IA — Unidad 1: tareas, ejecuciones y reservas

Contrato ejecutable previo a puntuación. Migración 0038, módulo `ai`, sin menú adicional. No inicia modelos, búsquedas, correo ni automatizaciones. OAuth/Work permanecen sin conectar. REST y MCP comparten servicios de aplicación.

## Datos y permisos

`ai_tasks`: identificador numérico, `process_score`, proceso, versión/hash y snapshot inmutable autorizado de entrada, skill version, solicitante, estado/version, intentos, disponibilidad y ejecución actual. Clave única `(task_type,process_id)` para puntuación inicial: cambiar skill o versión no reprocesa un proceso automáticamente. Una revisión futura exige contrato explícito. `ai_runs`: intento, actor autenticado, skill, reserva cifrada y hash, vencimiento, renovaciones, estado, código acotado y tiempos. Tokens no aparecen en auditoría, eventos, listados o logs.

Permisos sin grants automáticos: `ai.tasks.read`, `ai.tasks.create`, `ai.tasks.execute`, `ai.tasks.cancel`; intersectan `procesos.view` y scopes vigentes del proceso (all/self/team). Crear/cancelar exige humano MFA. Servicios limitados admiten read/execute y necesitan también procesos.view. Claim verifica además autorización vigente del solicitante original. No requested_by/actor libre.

Processes aporta puerto de snapshot y proyección autorizada; AI no escribe tablas Processes. ID canónico sin alias numérico. Snapshot externo es dato no confiable, máximo 256 KiB; no incorpora notas personales, secretos ni enlaces privados firmados.

## Estados y operaciones

- POST `/api/v1/ai-tasks`: schema_version `1.0`, process_id, input_version string, skill_version (identificador ASCII 1–100). Captura hash/snapshot desde propietario; rechaza versión stale. Deduplicación concurrente conserva tarea inicial, incluidas canceladas/completadas.
- GET `/api/v1/ai-tasks`: page/per_page, process_id/status opcionales, filtros y count autorizados antes de paginar; máximo 100.
- GET `/api/v1/ai-tasks/{id}`: metadata y máximo tres ejecuciones sin tokens ni snapshot. Reautoriza fuente actual; historial no modifica datos comerciales.
- POST `/api/v1/ai-tasks/claim`: schema_version `1.0`, limit 1–10. Toma pendientes elegibles y reservas expiradas; incluye rezagadas, sin filtro por día. Claim devuelve tarea, run_id, lease_token, vencimiento, snapshot y aviso `UNTRUSTED_EXTERNAL_CONTENT`. MySQL 5.7, sin SKIP LOCKED. Bloqueos actor→procesos en orden numérico→tarea. Revalida fuente, solicitante y reserva después de locks.
- POST `/api/v1/ai-tasks/{id}/renew`: sobre de ejecución; reserva cinco minutos, máximo dos renovaciones y quince minutos desde inicio. No revive reserva expirada.
- POST `/api/v1/ai-tasks/{id}/fail`: sobre más error_code (`transient`, `executor_limit`, `invalid_input`, `permission_denied`). Transitorios vuelven a pending con backoff 1/2 minutos y máximo tres intentos; agotamiento failed. Datos/permisos requieren needs_review. Conserva run fallido y limpia reserva.
- POST `/api/v1/ai-tasks/{id}/cancel`: schema_version y version string; humano autorizado. Invalida run/reserva; cancelada es terminal en U1.

Sobre renew/fail: schema_version, version (tarea), run_id, input_version, input_hash, skill_version, lease_token. Todos obligatorios; IDs/versiones strings. Idempotency-Key requerido en mutaciones; misma clave/cuerpo retorna efecto previo autorizado, cuerpo diferente 409. Claim conserva referencias, cifra token en run y revalida reservas al reconstruir replay; no entrega una reserva caducada/reemplazada. Renew/fail sólo permiten al actor de la reserva y rechazan resultado tardío, fuente cambiada o cancelación aun en replay. Una reserva expirada se registra como expired; agotamiento requiere revisión. Fuente cambiada se omite en claim y devuelve input_stale en writes; revisión/cancelación humana sigue disponible.

`pending → running → pending/failed/needs_review`; running expirada se recupera con nuevo run, hasta tres intentos. pending/running/failed/needs_review → cancelled. No endpoint de completed ni resultado genérico: completed/waiting_supplier se incorporarán exclusivamente con validadores de etapa. No puede declararse puntuación hecha sin resultado tipado persistido.

Errores: validation 422, forbidden 403, not_found 404 (registro ajeno), conflict/input_stale/lease_expired 409, integration_disabled 503. Estado+auditoría+domain_events transaccionales; no outbox/job sin ejecutor registrado ni consumer implícito.

## MCP y aceptación

Tools U1: list_ai_tasks, get_ai_task, claim_scoring_tasks, renew_ai_task, fail_ai_task. Escrituras incluyen idempotency_key. Crear/cancelar humano REST; ninguna tool comercial sensible, SQL, shell o correo. SDK oficial y transporte existente, mismas reglas de dominio. MCP request autenticada no concede permisos de task ni del proceso.

Aceptación: migración reproducible real MySQL5.7; dedupe y competencia de claims entre actores; replays/conflicto; recuperación y agotamiento; renew acotado; vencimiento/sustitución/cancelación/source stale; scope self/team, revocación usuario/servicio/solicitante; rollback auditoría/evento; input inválido/parcial, schema y transporte REST/MCP con cliente SDK. Regresión Go/race, Angular/build/navegador de módulos ya cerrados. No acredita Work ni producción.
