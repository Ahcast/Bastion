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

## Unidad 2 — conversación, menciones y actividad

### Datos y políticas

Permisos nuevos: `colaboracion.comment` y `colaboracion.moderate`, sin asignación automática a roles. `collaboration_comments` conserva cuerpo de texto plano (máximo 4000 caracteres), autor, recurso, versión y fecha; no admite edición. Borrar es una marca lógica, permitida al autor o a quien tenga `colaboracion.moderate`; requiere versión actual, clave idempotente y reautenticación. `collaboration_mentions` guarda solo las relaciones comentario/usuario. `collaboration_notifications` guarda destinatario, comentario y estado de lectura, nunca cuerpo, título ni extracto. `collaboration_activity` guarda recurso, actor, acción, comentario opcional y fecha, nunca copia `audit_events.details`.

El recurso usa `resource_type`/`resource_id`. Solo se admiten tipos con política de lectura registrada por composición; en U2 se registra únicamente `collaboration` y su ID es el de una entrada de biblioteca activa. La política vuelve a validar `colaboracion.view` y `core.documents.read` cada vez que se listan, crean o leen comentarios/actividad/notificaciones. U4 registrará recursos adicionales con la política del módulo propietario. Un ID ajeno, una carpeta eliminada o un documento en papelera se responde como inexistente.

`mention_user_ids` admite hasta 25 IDs distintos. Cada destinatario debe estar activo y, en ese momento, tener alcance de lectura de Colaboración y Core.Files sobre el mismo recurso. No se crean notificaciones para el autor. Al consultar o marcar una notificación como leída se vuelve a revisar la política; notificaciones de recursos cuyo acceso se revocó se omiten. El endpoint de sugerencias devuelve solo ID y nombre de usuarios legibles, nunca correos.

### API

```http
GET    /api/v1/collaboration/resources/{resource_type}/{resource_id}/comments?page=&per_page=
POST   /api/v1/collaboration/resources/{resource_type}/{resource_id}/comments
GET    /api/v1/collaboration/resources/{resource_type}/{resource_id}/activity?page=&per_page=
GET    /api/v1/collaboration/resources/{resource_type}/{resource_id}/mentionable-users?search=
DELETE /api/v1/collaboration/comments/{id}
GET    /api/v1/collaboration/notifications?after_id=&limit=
POST   /api/v1/collaboration/notifications/{id}/read
```

Crear comentario recibe `{body, mention_user_ids}`; borrar recibe `{version}`; marcar notificación como leída recibe `{}`. Toda escritura requiere `Idempotency-Key`. Comentarios y actividad usan paginación `page` 1–100000 y `per_page` 1–100; búsqueda de menciones tiene máximo 100 caracteres y devuelve hasta 50 resultados. Las notificaciones usan cursor `after_id` y límite 1–100, sin total global; `next_after_id` avanza por los IDs examinados aunque se hayan omitido notificaciones sin acceso vigente y `has_more` indica si queda otro tramo. La respuesta de notificación contiene solo `id`, `resource_type`, `resource_id`, `comment_id`, `created_at` y `read_at`.

La bandeja muestra únicamente la actividad mínima de U2 (`comment_added` y `comment_removed`) en orden `created_at, id`; no devuelve cuerpos, destinatarios ni el detalle de auditoría. Workflow y módulos comerciales todavía no se consultan.

### Aceptación de unidad 2

- Usuarios sin alcance actual no pueden enumerar recursos ni comentarios; se prueba revocación antes de cada lectura, creación, replay, mención y lectura/marcado de notificación.
- Texto se trata como texto plano y se limita en caracteres/cuerpo HTTP; no hay ruta de edición. Solo autor/moderador puede mandar a papelera; repetición idempotente no duplica comentario, relación, notificación ni actividad.
- Destinatario inactivo, duplicado, no registrado o sin acceso vigente no puede recibir mención. Notificaciones no almacenan extractos y se omiten tras perder acceso.
- MySQL 5.7 verifica rollback atómico, carreras de borrado/mención, aislamiento de destinatario y cursores sin conteo lateral. Navegador verifica comentario, sugerencia, mención, notificación, actividad y eliminación/revocación.

## Unidades siguientes — contratos previos

Unidad 3 añade tareas independientes de Workflow: asignado, creador, título, nota, fecha límite `DATE` en Honduras, estado `open|in_progress|done|cancelled`, versión y timestamps. Para recursos vinculados, asignado debe tener permiso de lectura vigente; tareas personales sin recurso solo las ven creador/asignado. El asignado puede cambiar estado; editar destino/vencimiento requiere `colaboracion.task`; una tarea cerrada no se reactiva sin acción explícita y auditoría. «Mi trabajo», «Hoy» y «Vencidas» son proyecciones sobre estas tareas, sin copiar tareas de otros dueños.

Unidad 4 introduce relaciones documentales sin duplicar bytes y una vista de aprobaciones pendientes ya registradas por Workflow. La política del módulo dueño se comprueba en cada acceso; Workflow decide/aprueba/revoca y conserva su versión/hash. Colaboración no crea aprobaciones ni ejecuta cambios comerciales.
