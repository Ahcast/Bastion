# Contrato de Colaboración

Versión 0.1.0. Colaboración organiza archivos privados y da un espacio común para comentarios, menciones y tareas. No sustituye Core.Files, Workflow ni los módulos comerciales.

## Propiedad y fases

El módulo depende de `core`; no escribe tablas de Clientes, Proveedores, Procesos, Cotizaciones, Órdenes, Facturas o Gastos. Composición lo habilita con `BASTION_COLLABORATION_ENABLED`; al deshabilitarlo se bloquean rutas/políticas y se conservan las filas y bytes.

| Unidad | Entrega | Migración prevista |
| --- | --- | --- |
| 1. Biblioteca | Carpetas empresa/persona, metadata, búsqueda, versiones y papelera; upload, preview y descarga por Core.Files | 0034 |
| 2. Conversación | Comentarios, menciones y actividad sobre recursos cuya política de lectura esté registrada | 0035 |
| 3. Trabajo | Tareas asignables, estados, vencimientos y vistas «Mi trabajo»/«Vencidas» | 0036 |
| 4. Integración | Relaciones documentales con entidades y bandeja de aprobaciones de solo lectura sobre Workflow | 0037+ |
| Aceptación | Regresión transversal de biblioteca, conversación, tareas, accesos, UI y CI | Sin migración adicional salvo hallazgo contractual |

Cada unidad se implementa y certifica antes de empezar la siguiente. Workflow conserva la propiedad de aprobaciones y sus decisiones; Colaboración solo ofrece una bandeja con enlaces a la pantalla propietaria. El uso de IA/OCR no forma parte de estas unidades.

## Unidad 1 — biblioteca

### Datos

`collaboration_folders` es propietaria de `id`, `scope` (`company` o `person`), `person_id` nullable, `parent_id` nullable, `name`, `created_by`, `version`, timestamps y `deleted_at`. El scope `person` requiere persona; el scope `company` prohíbe `person_id`. Un padre debe estar activo y compartir scope/persona. No se mueve un folder entre scopes; la creación solo puede apuntar a una carpeta ya existente, lo que evita ciclos. La profundidad máxima es 8.

`collaboration_entries` es propietaria de `id`, `document_id` único, `folder_id` nullable, `scope`, `person_id` nullable, `team_id` nullable, `title`, `notes`, `uploaded_by`, `version` y timestamps. El documento, equipo, persona, carpeta y creador usan FKs. Su scope/persona/equipo debe coincidir con la carpeta. En scope persona, quien carga el archivo debe ser esa persona; no se suplanta al dueño. El scope del registro y la carpeta son inmutables.

Los bytes, hash, MIME real, versiones físicas, cuarentena, análisis, claves S3, equipo y URLs temporales siguen siendo propiedad exclusiva de Core.Files (`documents`/`document_versions`). La UI primero carga por Core.Files y luego registra el documento en la biblioteca. Core.Files admite PDF, DOC/DOCX, XLS/XLSX, CSV, TXT, JPG/JPEG y PNG, hasta 20 MiB; Office/CSV siempre se descargan como adjunto y solo PDF/imagen/TXT pueden tener preview. La carga valida firma de formato, checksum y MIME; el análisis externo sigue siendo necesario para salir de cuarentena. El registro llama `Core.BindDocument` a `entity_type='collaboration'`/ID de entrada dentro de la misma transacción que crea `collaboration_entries`. No copia ni modifica adjuntos de otros dueños.

Las extensiones y límites son exactamente los que Core.Files valide. Preview usa solo los tipos permitidos por Core; los demás se descargan como adjunto. No se devuelve `object_key`, URL persistente ni bytes en la metadata. El documento conserva sus estados Core `pending`, `quarantined`, `available` o `rejected`; estar en la biblioteca no omite análisis ni cuarentena. Solo se puede vincular un archivo cuyo upload fue finalizado y cuyo estado está `quarantined` o `available`; cargas `pending`, rechazadas, borradas o generadas no se registran.

### Permisos y autorización por fila

Permisos de la unidad: `colaboracion.view`, `colaboracion.upload`, `colaboracion.organize` y `colaboracion.delete`; migración sin asignarlos a roles. Las operaciones de archivo también exigen el permiso Core correspondiente (`core.documents.read`/`core.documents.write`).

Cada decisión exige permiso de Colaboración **y** acceso Core al documento. Scope `all` ve todos los registros; `self` ve los de la persona indicada en folder personal y los creados por el actor en empresa; `team` solo ve archivos cuyo `team_id` coincide con un equipo vigente del actor. Una carpeta compartida por equipo requiere que quien sube pertenezca a ese equipo y que Core.Files registre el mismo `team_id`. Scope persona nunca es un filtro cosmético. Todos los endpoints —incluyendo preview, descarga, metadata, historial, papelera, restore y replays idempotentes— vuelven a comprobar fila y permisos; conocer el ID no autoriza acceso. Core delega políticas separadas a Colaboración para vincular, crear versiones y enviar a papelera, así que los endpoints genéricos de Files no pueden saltarse `colaboracion.organize`, `colaboracion.upload` o `colaboracion.delete`.

Toda mutación requiere sesión humana MFA, scope y versión vigente. Upload usa el flujo Core prefirmado; crear/editar folder y enlazar documento usa `Idempotency-Key`; actualización usa versión optimista y devuelve 409 si quedó obsoleta. Vincular, enviar a papelera/restaurar archivos y borrar/restaurar folders requieren reautenticación. Papelera/restauración delega a Core.TrashFile: conserva metadata/bytes y su versión; un folder se elimina lógicamente solo si no contiene entradas ni carpetas activas.

### API REST

```http
GET    /api/v1/collaboration?scope=&person_id=&folder_id=&search=&page=&per_page=
GET    /api/v1/collaboration/folders?scope=&person_id=&parent_id=
GET    /api/v1/collaboration/teams
POST   /api/v1/collaboration/folders
PATCH  /api/v1/collaboration/folders/{id}
DELETE /api/v1/collaboration/folders/{id}
POST   /api/v1/collaboration/folders/{id}/restore
POST   /api/v1/collaboration
GET    /api/v1/collaboration/{id}
PATCH  /api/v1/collaboration/{id}
DELETE /api/v1/collaboration/{id}
POST   /api/v1/collaboration/{id}/restore
GET    /api/v1/collaboration/{id}/versions
GET    /api/v1/collaboration/{id}/preview
GET    /api/v1/collaboration/{id}/download
```

IDs y versiones viajan como cadenas JSON. Paginación es `page` 1–100000, `per_page` 1–100; búsqueda tiene máximo 100 caracteres y se escapan comodines SQL. Listas aplican scope antes de count/página y ordenan por `updated_at DESC, id DESC`. `GET /collaboration` usa `deleted=true` para papelera; `folder_id=root` consulta archivos sin carpeta. `GET /folders` devuelve `{data:[...]}` de una ubicación; `parent_id=root` selecciona raíz; `GET /teams` devuelve solo equipos del actor. DTOs son estrictos. `POST /collaboration` vincula un documento Core existente: `{document_id, document_version, scope, person_id, folder_id, title, notes}`. `PATCH` de entrada acepta `{version, folder_id, title, notes}`; no cambia `document_id`, scope ni persona. Los endpoints de upload/finalize/version/preview/download de Core siguen siendo los únicos que mueven bytes.

Errores conservan el catálogo Core: 401 sesión inválida; 403 permiso/reauth requerido; 404 registro ajeno o inexistente sin enumeración; 409 versión/uso/estado incompatible; 422 DTO o transición inválida; 503 Core.Files/S3 no disponible. La respuesta nunca distingue un ID ajeno de uno inexistente.

### Aceptación de unidad 1

- Empresa y carpetas personales conservan navegación, conteo, filtros/búsqueda, título/notas, tamaño, preview, descarga y papelera reversible.
- Folder padre/scope/propietario no se pueden alterar por payload; carpetas no se cruzan ni se eliminan ocupadas; paginación no revela conteos fuera del ACL.
- Upload incompleto, hash/firma/MIME incorrectos, estado pendiente/rechazado, tipo activo, documento generado, ID ajeno o vínculo duplicado se rechazan de forma explícita.
- La versión física se conserva en Core; versión obsoleta, replay cambiado, rollback y dos actores concurrentes no pisan metadata ni reubican documentos.
- Pruebas MySQL 5.7 verifican all/self/team, persona/empresa, módulo deshabilitado, reauth, revocación, carrera, auditoría y vínculo Core transaccional. Navegador verifica carga sintética privada, estado/cuarentena, búsqueda, folder, preview/descarga autorizados, papelera/restauración y usuario ajeno.

## Unidades siguientes — contratos previos

Unidad 2 añade comentarios como texto plano, append-only salvo papelera por autor/moderador, anclados a una entrada o recurso con `entity_type`/`entity_id`. Solo tipos registrados en composición se admiten. La política propietaria de lectura se valida antes de listar, comentar o mencionar. `mention_user_ids` se validan contra usuarios activos que puedan ver el mismo recurso; notificaciones no llevan extracto privado y cada lectura vuelve a validar acceso. El feed de actividad publica actor/acción/recurso/fecha sin copiar `audit_events.details`.

Unidad 3 añade tareas independientes de Workflow: asignado, creador, título, nota, fecha límite DATE Honduras, estado `open|in_progress|done|cancelled`, versión y timestamps. Para recursos vinculados, asignado debe tener permiso de lectura vigente; tareas personales sin recurso solo las ven creador/asignado. El asignado puede cambiar estado; editar destino/vencimiento requiere `colaboracion.task`; una tarea cerrada no se reactiva sin acción explícita y auditoría. «Mi trabajo», «Hoy» y «Vencidas» son proyecciones sobre estas tareas, sin copiar tareas de otros dueños.

Unidad 4 introduce relaciones documentales sin duplicar bytes y una vista de aprobaciones pendientes ya registradas por Workflow. La política del módulo dueño se comprueba en cada acceso; Workflow decide/aprueba/revoca y conserva su versión/hash. Colaboración no crea aprobaciones ni ejecuta cambios comerciales.
