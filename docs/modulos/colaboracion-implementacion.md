# Colaboración: implementación local

Fecha: 2026-10-23. No implica publicación, migración productiva ni activación.

## Alcance y decisiones

Se conservan empresa/personas, título, archivo, notas, búsqueda literal, filtros, paginación, actualizar, subir, vista previa y descarga. La papelera configurable es una ampliación confirmada por el usuario en esta tarea. No se introducen chat, tareas, comentarios ni reemplazo de archivos no documentados.

La política inicial es 50 MiB por archivo, 30 días en papelera, ofimática/imágenes/ZIP y bloqueo de ejecutables/scripts. `files.policy`, versión 1, reside en el almacén común de settings del Core, alcance `empresa`. Ajustes > Colaboración permite editar `max_file_mb` (1..100), `trash_retention_days` (1..365), `allowed_extensions` y `blocked_extensions`. Estos rangos son topes técnicos del primer corte, no reglas detectadas del Legacy. Las extensiones permitidas se limitan a formatos que el servidor identifica. El bloqueo base de ejecutables, scripts, HTML/SVG activos y documentos con extensión de macros siempre se mantiene; la lista editable permite restricciones adicionales. Los cambios rigen nuevas cargas y nuevas entradas en papelera, sin acortar plazos ya registrados.

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

Pruebas incorporadas: validación de MIME/extensión, archivo/ruta/alcance, papelera/restauración/purga, settings, navegación y permisos visibles. Las suites MySQL 5.7 de servicio y HTTP están configuradas en CI con bases aisladas; en Unix usan un AWS CLI sintético para probar reintentos, integridad y limpieza sin credenciales ni objetos reales. La ejecución local sin esas bases omite las pruebas de integración; no equivale a validarlas. La compilación general del checkout tiene errores preexistentes en Procesos (`ReadyDelivery.deferred`/`source_total`); para comprobar integración API/servidor se usó adicionalmente una copia temporal con esos tres accesos neutralizados, sin modificar Procesos en el proyecto. La revisión visual local no pudo ejecutarse porque el control de permisos del navegador rechazó el origen localhost.

Fuentes técnicas: [OWASP File Upload Cheat Sheet](https://cheatsheetseries.owasp.org/cheatsheets/File_Upload_Cheat_Sheet.html), [formato CFB de Microsoft](https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-cfb/53989ce4-7b05-4f8d-829b-d08d6148375b).
