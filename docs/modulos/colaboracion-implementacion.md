# Colaboración: implementación y operación

Implementada y publicada desde `14b1bf67cd1df545df84ac0ea6ddd4dde18a1ece`. El 2026-10-26 quedó permitida y activada operativamente sobre la release existente `a7757d3` de AWS/Bluehost, después de comprobar S3 real. No se sustituyeron los artifacts de los módulos publicados posteriormente. Ver la evidencia de activación al final; la prueba integral con una sesión administrativa sigue pendiente.

## Alcance y decisiones

Se conservan empresa/personas, título, archivo, notas, búsqueda literal, filtros, paginación, actualizar, subir, vista previa y descarga. La papelera configurable es una ampliación confirmada por el usuario en esta tarea. No se introducen chat, tareas, comentarios ni reemplazo de archivos no documentados.

La política inicial es 50 MiB por archivo, 30 días en papelera, ofimática/imágenes/ZIP y bloqueo de ejecutables/scripts. Por decisión del usuario del 2026-10-27, `.typ` se añade como texto plano UTF-8 (`text/plain`); `.png` y `.jpeg` ya están permitidos en la lista inicial. `files.policy`, versión 1, reside en el almacén común de settings del Core, alcance `empresa`. Ajustes > Colaboración permite editar `max_file_mb` (1..100), `trash_retention_days` (1..365), `allowed_extensions` y `blocked_extensions`. Estos rangos son topes técnicos del primer corte, no reglas detectadas del Legacy. Las extensiones permitidas se limitan a formatos que el servidor identifica. El bloqueo base de ejecutables, scripts, HTML/SVG activos y documentos con extensión de macros siempre se mantiene; la lista editable permite restricciones adicionales. Los cambios rigen nuevas cargas y nuevas entradas en papelera, sin acortar plazos ya registrados.

Despliegue de formatos, 2026-10-28: backend `43e65be2ec16b712b310abd3a47f683715149bc2`, con CI completo exitoso (Core checks run `40624114593`, Scout control checks exitoso). Artifact verificado por SHA-256; release anterior `c6d2124a2b72` conservada y respaldo privado de configuración/metadata. Sin migraciones ni cambios de ajustes/allowlist; la fila `files.policy` estaba ausente, así que aplican defaults. Readiness HTTPS 200 y ajustes anónimos 401. La carga autenticada de `.typ`, `.png` y `.jpeg` no se ejercitó en producción.

## Propiedad e integración

- `crates/modules/colaboracion` es dueño de metadatos y casos de uso. Migración MySQL 5.7 propia, con ledger `_colaboracion_migrations`. `migrate` aplica DDL; `serve` solo verifica checksums y registra disponibilidad.
- Core conserva identidad, autorización, selector mínimo de personas, almacenamiento S3, settings y auditoría. El módulo no modifica personas, usuarios ni roles. Las FK impiden borrar una persona con archivos activos/en papelera.
- Registro `colaboracion`, allowlist `BASTION_ALLOWED_MODULES` y activación administrativa. Sin migraciones exactas, exclusión por ambiente o desactivación: no hay navegación ni operaciones disponibles. Desactivar conserva datos. El worker puede terminar eliminaciones ya autorizadas y retención vencida si la migración es compatible.
- Se mantiene frontend Bluehost, backend/API AWS y MySQL Bluehost. No se expone MCP; futuros adaptadores deben usar estos casos de uso con autorización delegada.

## Permisos y carpetas

`ver`: listar, vista previa, descargar y leer política. `editar`: subir en la carpeta autorizada. `eliminar`: mover a papelera, restaurar y solicitar borrado definitivo. Guardar política exige `editar` en alcance `settings`. Los alcances de archivos son `empresa`, `persona:{id}` o `*`; el Administrador del sistema conserva acceso implícito. La matriz común prepara permisos de módulo completo con `*` y conserva alcances específicos existentes.

La lista se filtra antes de paginar usando los alcances efectivos del Core. Cada lectura/mutación verifica la carpeta del objeto concreto; acceder por ID a una carpeta ajena devuelve 404. El selector expone solo ID y nombre de personas autorizadas para ver/editar archivos. No otorga acceso a DNI, usuarios ni administración de Personas. No se infiere que un usuario pueda leer archivos de su persona por estar vinculado a ella. Esta aplicación tiene una empresa por base de datos, como el Core actual; no se añade multitenancy implícito.

## Archivos privados y validación

Se reutilizan `BASTION_DOCUMENTS_*` y `BASTION_AWS_CLI`. Objetos bajo `colaboracion/empresa/{aleatorio}` o `colaboracion/personas/{id}/{aleatorio}`. El nombre original nunca forma la clave ni una ruta local. Las respuestas no exponen claves S3, hashes ni rutas. Cifrado S3 AES256, bucket privado, sin URL pública ni caducidad de archivos vigentes. La disponibilidad de archivos no depende de instalar Typst.

El Core ejecuta `file --brief --mime-type` (libmagic) en un temporal privado y con plazo de 5 segundos, usando `BASTION_FILE_BIN` (por defecto `/usr/bin/file`). Se coteja el MIME detectado contra la extensión y la política; el MIME declarado por el navegador no se recibe como autoridad. Se bloquean dobles extensiones peligrosas, rutas, NTFS streams y controles. Office moderno/ODF/ZIP inspeccionan su directorio central sin extraer: hasta 2000 entradas y 500 MiB descomprimidos declarados; se rechazan cifrado, multidisco, ZIP64, rutas de escape, archivos comprimidos anidados, macros identificadas y nombres de ejecutables/scripts. La inspección de nombres y MIME no es antivirus ni análisis profundo del contenido comprimido/OLE/PDF. Los formatos ambiguos o no identificados se rechazan.

Vista previa: PDF, imágenes raster compatibles y texto inerte. HTML/SVG nunca adquieren un origen activo. Documentos Office y ZIP se descargan, sin conversión inventada. Los bytes recuperados se comparan con SHA-256 antes de entregarlos; tras S3 se vuelven a comprobar estado del registro, módulo y permiso. El frontend revoca sus URLs de objetos al cerrar la vista previa.

## Papelera y recuperación de fallos

Mover a papelera guarda `trashed_at` y `purge_after` en UTC. La descarga deja de estar disponible. Restaurar, antes del vencimiento, devuelve el archivo a su carpeta original. La purga manual solo acepta archivos en papelera, con `confirm_id` igual al ID del objeto y permiso independiente. La interfaz solicita confirmación explícita.

Un worker revisa una operación cada 10 segundos. Purga retención vencida, solicitudes de borrado definitivo y cargas abandonadas tras una hora. Nunca elimina archivos vigentes por edad. El borrado definitivo enumera y elimina las versiones y los marcadores del **objeto exacto** en el mismo bucket; un prefijo vecino nunca se elimina. Trabaja en lotes acotados y reintenta si hay más versiones o falla S3/IAM. Solo tras confirmar el lote final se vacían título/notas/nombre/persona y se marca el recibo `deleted`. Se conserva un recibo mínimo de idempotencia y claves/hashes para evitar replay, sin contenido de negocio. Los eventos identifican carga, papelera, restauración, purga solicitada, eliminación física y motivo de limpieza; los emite el Core. La auditoría conserva la retención común de un año.

La subida reserva metadatos antes de escribir S3. Un error confirmado de almacenamiento deja estado reintentable; repetir la misma clave/contenido reutiliza el objeto. Una subida simultánea aún en curso produce 409. Un proceso interrumpido deja una reserva recuperable por el worker. Misma clave con contenido distinto o tras purgar produce 409. Se revalida autorización/activación antes de publicar la carga. No hay archivos huérfanos sin recibo por una caída entre S3 y MySQL.

## Límites y contrato

[OpenAPI del módulo](colaboracion.openapi.yaml). JSON con base64, hasta 140 MiB de cuerpo para el máximo técnico de 100 MiB; el límite efectivo se aplica al archivo decodificado según settings. HTTP: 120 segundos, 120 solicitudes/actor/minuto y dos operaciones de archivos concurrentes por proceso antes de extraer el cuerpo. S3: cuatro slots compartidos, un segundo de espera, hasta 90 segundos para transferencias grandes; purga por lotes con plazo acotado. Paginación 1..10000 y 1..100 filas, 25 por defecto; búsqueda hasta 200 caracteres. Cookie, TOTP, CSRF y Origin exacto se heredan del adaptador común. El transporte base64 tiene sobrecosto de memoria/red; el proxy debe admitir el cuerpo y timeout efectivos. Sin cambio de arquitectura a enlaces públicos.

## Preparación operativa y validación

Antes de activar: instalar `file`/libmagic (incluido en Dockerfile), comprobar AWS CLI y bucket privado existentes; conceder IAM `GetObject`, `PutObject`, `DeleteObject`, `DeleteObjectVersion` para `colaboracion/*` y `ListBucketVersions` acotado a ese prefijo; revisar Object Lock/retenciones legales, que pueden mantener una purga pendiente. No configurar lifecycle que elimine archivos vigentes. Las copias de respaldo independientes se rigen por su propia política y no se borran desde este módulo. Confirmar los límites del proxy para 50 MiB y el sobrecosto base64; no asumir que todos los máximos técnicos pasan el perímetro. Aplicar migración, allowlist y activar desde Ajustes > Core > Módulos.

Pruebas incorporadas: validación de MIME/extensión, archivo/ruta/alcance, papelera/restauración/purga, settings, navegación y permisos visibles. `Core checks` ejecuta compilación y pruebas completas con MySQL 5.7 y bases aisladas; el job específico valida también transporte S3 sintético, reintentos, integridad y limpieza sin credenciales ni objetos reales. Una ejecución local sin esas bases omite las pruebas de integración y no equivale a validarlas.

Fuentes técnicas: [OWASP File Upload Cheat Sheet](https://cheatsheetseries.owasp.org/cheatsheets/File_Upload_Cheat_Sheet.html), [formato CFB de Microsoft](https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-cfb/53989ce4-7b05-4f8d-829b-d08d6148375b).

## Release verificado

El 2026-10-24 se publicó `14b1bf6` en `main`. `Core checks` (workflow `40150644138`) y `Scout control checks` finalizaron correctamente. El artifact Linux y el frontend se verificaron contra los SHA-256 de GitHub, los checksums incluidos y el `BUILD.txt` antes de instalarlos.

AWS aplicó la migración `202610230001`; se respaldó la base en ubicación privada y se cotejaron esquemas y filas lógicas de las 50 tablas preexistentes. El backend quedó en la nueva release, `/health/live` y `/health/ready` respondieron 200 y los demás módulos conservaron su estado. Bluehost publicó el `BUILD.txt` del mismo commit, pasó el fallback SPA y se comprobaron los hashes de 36 recursos públicos. La configuración del API y las reglas `.htaccess` se conservaron.

En el corte del 2026-10-24, Colaboración quedó `included_in_release=1`, `environment_allowed=0` y `enabled=0`. La allowlist no se modificó. La identidad AWS del servicio denegó `ListBucketVersions` en `colaboracion/*`; tampoco fue posible consultar la configuración de versionado del bucket con esa identidad. Antes de ofrecer el módulo, infraestructura debe conceder las acciones S3 mínimas acotadas al prefijo y verificar privacidad, versionado/Object Lock y timeout del proxy. Se confirmó acceso anónimo denegado en la API; no se probó carga ni purga contra el bucket real.

La migración usó el operador temporal disponible en el host; las variables de migración solo existieron en ese proceso y no se guardaron en el entorno del servicio. Provisionar una identidad MySQL exclusiva para migraciones sigue siendo una mejora operativa pendiente antes de los siguientes cambios de esquema.

## Activación operativa del 2026-10-26

Tras la confirmación del usuario de que actualizó la política S3 existente para los documentos de Cotizaciones, se comprobó la identidad IAM real del proceso restringido del backend (usuario, no rol de instancia). Se reutilizaron sus credenciales y el bucket compartido. No se modificaron políticas IAM ni credenciales durante esta activación.

La prueba bajo el usuario y el espacio de montajes del servicio creó un único objeto sintético bajo `colaboracion/empresa/`, escribió dos versiones, recuperó el contenido con integridad comprobada y verificó cifrado AES256 y rechazo anónimo HTTP 403. Tras crear un marcador de borrado, se enumeraron las dos versiones y el marcador, se eliminaron por su ID y se comprobó el listado vacío. El objeto de prueba y sus versiones se limpiaron completamente, sin crear registros de negocio. Esta prueba acredita el acceso operativo al prefijo y el versionado observado de ese objeto; la identidad de ejecución continúa sin permisos para consultar globalmente versionado, bloqueo público y Object Lock del bucket.

Se mantuvo la release vigente `a7757d38a077d51909c514102121a7e9be00d906`, cuyos binarios instalados se verificaron contra su manifiesto. Con bloqueo exclusivo de despliegue se respaldaron privadamente la configuración y las 61 tablas existentes; el dump completo y su SHA-256 quedaron registrados. Se añadió únicamente `colaboracion` a `BASTION_ALLOWED_MODULES` y se reinició el backend. El arranque validó la migración instalada `202610230001`; no se ejecutaron migraciones ni se cambió el esquema.

Por petición expresa del usuario se activó el registro de Colaboración mediante una operación administrativa de despliegue. El evento `core / module.deploy_activation` usa actor nulo para identificar la operación del sistema, sin atribuir una sesión a un usuario. El estado final es `included_in_release=1`, `environment_allowed=1`, `enabled=1`. Se comprobaron los estados de los otros ocho módulos y los registros de Colaboración antes y después: se conservaron. No se concedieron permisos a roles ni se alteraron cuentas o sesiones.

Bluehost ya servía el frontend de `a7757d3`; no se volvió a instalar una release anterior. Se verificaron por HTTPS cuatro recursos del manifiesto (incluido el chunk de Colaboración), `BUILD.txt`, el origen público de API y el fallback de `/colaboracion`. Después del reinicio, `/health/live` y `/health/ready` respondieron 200 y las rutas de Colaboración/settings mantuvieron 401 sin sesión.

Pendiente: prueba integral desde UI/API con una sesión administrativa existente, incluida carga máxima de 50 MiB a través del proxy, restauración y purga por el worker. La prueba directa S3 y los checks anteriores de CI no sustituyen esa comprobación. La inspección de Caddy no mostró una restricción de cuerpo adicional; se conserva el límite de 140 MiB y el timeout de 120 segundos del adaptador API.
