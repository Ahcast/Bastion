# Despliegue del Core

## Seguridad y rendimiento: despliegue verificado del 2026-11-10

El 2026-11-10, a las 23:37 de Honduras (2026-11-11 05:37 UTC), se activó en AWS el backend `e27376a677004b2e138324ac44bc9be5f6e5d2bc`, sobre `d121e80b5236188501384c14b28018c879a36430`, con [Core checks aprobados](https://github.com/Ahcast/Bastion/actions/runs/42964090945), MCP MySQL y Scout exitosos. Se comprobaron el ZIP de GitHub, el paquete, BUILD, el manifiesto y el ejecutable del proceso activo. SHA-256 del ZIP: `0af5fa23d84960c1739a16e91d4af79d300cb1c3868bbb343c1d4cbff44986d7`. El frontend Bluehost conserva su versión anterior, compatible; se verificaron BUILD y la configuración pública sin reemplazarlos.

Se aplicó únicamente **Cotizaciones 5**. Antes se ensayaron migración y reversión sobre una copia temporal con las 19 cotizaciones. Durante la pausa de la API se generó un respaldo completo de MySQL y se restauraron sus datos en 105 tablas temporales, comparándolos con el original. El respaldo definitivo contiene 14 017 536 bytes, SHA-256 `598ebc678ea581b3fe0b2eeb6c4899030924563dfc682c3e9341d7c6cfdb28a9`; permanece privado en AWS junto con la configuración, la versión anterior y la evidencia de restauración. Se verificaron los checksums de todos los registros de migración, la igualdad de los campos comerciales anteriores, las fechas derivadas, el índice fecha/ID, el orden de las cotizaciones y los estados de los módulos. La credencial existente se suministró al migrador puntual; no se persistieron variables `MYSQL_MIGRATOR_*` en el servicio.

Un intento inicial que alcanzó la migración revirtió el índice y su registro por un error del script de comprobación de arranque, y restableció el backend anterior con readiness 200. Tras corregir el script se repitió el respaldo y la restauración; el despliegue definitivo terminó con código 0 y una pausa de API de 255,68 segundos. No se detectaron cambios en los datos comerciales durante esas operaciones. La reversión conserva el procedimiento de retirar exclusivamente la columna derivada y su índice, restaurar el registro de migraciones anterior y volver al backend previo, siempre con la API detenida y comprobando antes la integridad de los datos.

La verificación posterior confirmó readiness HTTPS 200, CORS/CSRF y nueve rutas protegidas con 401 sin sesión. Una pestaña independiente reutilizó la sesión existente para consultar Inicio, Procesos, listado/detalle/resumen de Cotizaciones, Órdenes, Facturas, Reportes, Clientes, Proveedores, Gastos, Licitaciones y Colaboración: todas cargaron y la consola no registró errores. La pestaña con el expediente abierto del usuario no se modificó. Los servicios de API, respaldo, Scout, protección de salida MCP y socket de PDF están activos, sin errores nuevos del backend en la ventana revisada. No se ejercitaron cambios de contraseña/TOTP, envíos de correo, conversiones ni escrituras comerciales en producción; esos flujos están cubiertos por la CI del commit instalado. No se atribuye todavía una mejora porcentual de latencia a este despliegue.

## Preparación de seguridad y rendimiento — 2026-11-10

El cambio actualiza Tokio, autenticación, autorización y bloqueos de lectura. Incluye la migración **Cotizaciones 5**, que añade `quote_date_search` generada e índice `cotizaciones_date_id`. No modifica datos comerciales, estados de módulos, numeración, permisos ni configuración del frontend.

Antes de instalar el backend: verificar CI y hashes del commit, respaldar MySQL y comprobar restauración, aplicar únicamente la migración pendiente con identidad de migración y verificar la columna derivada contra `data_json`. Añadir una columna almacenada puede reconstruir la tabla en MySQL 5.7: planificar la pausa según tamaño y probar primero sobre una copia. Conservar el backend anterior; el esquema añadido es compatible con sus consultas, pero su comprobación estricta de migraciones requiere un procedimiento de reversión ensayado para mantener la disponibilidad del módulo. No retirar el índice/columna durante uso concurrente.

Validar acceso, recuperación, TOTP, lecturas concurrentes, desactivación, edición/versiones, conversiones y reportes. Medir p50/p95 y esperas SQL con datos de prueba representativos, sin realizar pruebas de carga sobre producción. El frontend no necesita cambios. Esta sección registra preparación de código; no acredita instalación ni ejecución de migraciones en AWS/Bluehost.

## Arquitectura acordada

- Frontend React + TypeScript estricto y Vite, compilado como archivos estáticos para Bluehost (ADR-0002).
- Backend/API/MCP Rust en AWS Lightsail, detrás de HTTPS y un reverse proxy. El backend escucha en loopback o red privada.
- MySQL en Bluehost, conexión directa desde la salida autorizada de AWS, con TLS y validación de identidad del certificado.
- Cloudflare Free es el borde previsto; su configuración y restricciones del origen siguen pendientes de validación. El ambiente de pruebas actual llega directamente a Caddy.

Las IPs, cuentas, nombres de bases, rutas reales, credenciales y respaldos operativos se conservan fuera del repositorio público. Los archivos de este repositorio usan configuración pública o ejemplos sin secretos.

## Frontend React en Bluehost

El código fuente está en `web/src/`; los archivos públicos en `web/public/`. Configurar `web/public/config.js` únicamente con el origen público HTTPS del API. Mantener fuentes locales y licencias. Node se usa en desarrollo y CI; Bluehost sirve archivos y no ejecuta un proceso Node.

Dentro de `web/`, ejecutar:

```sh
npm ci --ignore-scripts
npm run typecheck
npm test
npm run build
```

Publicar únicamente el contenido de `web/dist/`. No subir fuentes, tests, `node_modules`, archivos de entorno, dumps ni credenciales. El frontend conserva los flujos existentes y no implementa por sí mismo endpoints pendientes del Core.

## GitHub y publicación

Trabajar directamente en `main`, sin ramas de desarrollo. El workflow `Core checks` verifica frontend y Rust. El frontend usa Node y dependencias fijadas por `package-lock.json`; Rust usa el toolchain del repositorio y `Cargo.lock` con `--locked`.

GitHub genera los artifacts `bastion-web-static` y `bastion-server-linux-x86_64`, con procedencia y SHA-256; se conservan 30 días. El paquete del backend se compila en Ubuntu 24.04 para Linux x86_64. Construir artifacts no ejecuta migraciones ni despliega servicios.

Orden de publicación: actualizar documentos y código, publicar en GitHub, esperar verificaciones, descargar el artifact de ese commit, verificar ZIP/tar/archivos y después instalarlo en el hosting. Conservar respaldo privado antes de reemplazar la versión activa y mantener una reversión disponible.

Para el frontend, instalar assets antes de cambiar atómicamente el HTML. Conservar las cabeceras y el fallback SPA del `.htaccess` principal. Revalidar `index.html` y `config.js`; los JS/CSS con hash reciben caché inmutable mediante `assets/.htaccess`. Retirar los scripts de la implementación anterior a respaldo privado, fuera del directorio público. Preservar los assets con hash anteriores mientras puedan usarlos clientes con HTML en caché. Verificar por HTTPS los hashes y que las rutas SPA, fuentes, QR y carga de pantallas funcionen sin errores.

## API, cookies y proxy

Configurar CORS con el origen HTTPS exacto del frontend y permitir credenciales únicamente para ese origen. Toda mutación web exige origen exacto y CSRF; las operaciones sobre sesiones conservan su vinculación en servidor. No almacenar credenciales en almacenamiento web ni insertar secretos en variables Vite.

Con Caddy directo, seguir `deploy/Caddyfile.example`: sobrescribir `X-Forwarded-For` con la IP del peer, eliminar cabeceras alternativas y configurar en el backend únicamente los CIDR reales del proxy. Recorrer la cadena de derecha a izquierda; no confiar en cabeceras aportadas directamente por clientes ni en redes generales. Si se añade Cloudflare u otro proxy, validar la cadena antes de habilitarla. Mantener HTTPS y no exponer el puerto interno del backend.

Para MCP/OAuth, el ejemplo de Caddy elimina los parámetros de consulta completos y cabeceras sensibles de sus logs de acceso y de error, incluidas las redirecciones con códigos. Antes de instalarlo, validar el archivo con la versión real de Caddy y verificar con valores sintéticos que los logs de acceso, errores, CDN y cualquier proxy adicional no conserven códigos, estado, cookies ni tokens. El ejemplo se validó con la versión instalada de Caddy y pruebas temporales en loopback comprobaron que códigos, cookies, cabeceras y redirecciones sintéticos no aparecen en los logs de acceso ni de error. La configuración activa de Caddy incorpora la redacción validada en el despliegue MCP del 2026-11-01.
El servicio del backend debe usar una cuenta restringida, secretos entregados por el gestor del ambiente y permisos mínimos. No imprimir secretos en logs, comandos ni respuestas. No mezclar credenciales de migración con las del proceso normal. Las actualizaciones de frontend y backend son independientes; no reiniciar ni migrar el backend cuando solo cambia el frontend.

## MySQL, correo y archivos

La compatibilidad inicial es MySQL 5.7; las migraciones existentes se verificaron en el ambiente de pruebas. Revisar con Bluehost la actualización a una versión con mantenimiento vigente antes de producción. Mantener cuenta de ejecución con privilegios mínimos y otra credencial para migraciones, lista de IP de salida autorizada, pool acotado y certificado/hostname verificados. No degradar TLS para resolver conectividad.

Respaldar antes de migrar y ensayar restauración. El comando `check-db` verifica TLS y `SELECT 1` sin crear tablas; `migrate` aplica migraciones con su credencial exclusiva. La retención de seguridad y auditoría sigue `08-seguridad-y-permisos.md`.

SMTP usa configuración global autorizada y credenciales personales cifradas. TLS es obligatorio; resolver y fijar IPs públicas válidas, rechazar destinos internos/reservados y restringir egreso según la política del Core. PDFs y adjuntos privados requieren almacenamiento privado, autorización por descarga y respaldo; no se publican junto al frontend.

## Proveedores: incorporación verificada

El 2026-10-24 se publicó el commit de código `2ef93c5` (workflow `40151489770`, CI exitoso) y se desplegaron sus artifacts verificados en AWS y Bluehost. En AWS se respaldó MySQL, se aplicó la migración propia 1 y se preservaron las 53 tablas previas y la activación de los demás módulos. Las cuatro tablas de Proveedores están vacías y readiness responde 200. Se agregó `proveedores` a `BASTION_ALLOWED_MODULES`; el registro queda disponible con `enabled=0`, pendiente de activación administrativa. No se importaron datos Legacy.

Bluehost publica 40 archivos verificados desde el mismo commit. Se conservaron `config.js` y `.htaccess`, se mantuvieron assets anteriores para cachés y se creó un respaldo privado. Ver [alcance, evidencia y pendientes](modulos/proveedores-implementacion.md).
## Estado y verificaciones pendientes

El ambiente de pruebas tiene frontend, backend HTTPS y conectividad MySQL TLS comprobados. Esto no constituye aprobación de producción ni cierra los criterios funcionales. Faltan pruebas integrales con usuarios, funciones pendientes del Core, configuración y validación de Cloudflare, privilegios mínimos de MySQL, separación de ambientes, almacenamiento privado, restauraciones, observabilidad, límites y objetivos operativos. Mantener cada pendiente explícito y no crear datos o estadísticas ficticias para aparentar implementación.

## Órdenes de compra: incorporación verificada

El corte local incorpora migraciones propias con `migrate` y sincroniza disponibilidad al arrancar, sin DDL en `serve`. Requiere `ordenes_compra` en `BASTION_ALLOWED_MODULES`, migraciones exactas y habilitación administrativa; destinos se carga explícitamente desde Ajustes. CI prepara bases MySQL 5.7 aisladas para servicio/HTTP. El 2026-10-23 se publicaron en GitHub y se instalaron en AWS/Bluehost los artifacts verificados de 59a8057 tras CI exitoso y respaldo privado; migración 1 aplicada. El módulo permanece desactivado y los módulos existentes se conservan. Se mantiene Bluehost/AWS/MySQL. Ver `modulos/ordenes-de-compra-implementacion.md`.

## Facturas: incorporación verificada

El 2026-10-23 se instalaron en AWS y Bluehost los artifacts verificados de `f6c5a1d` (workflow 40038962819), después de respaldo privado. Se aplicó la migración propia 1 y se comprobó que las 43 tablas existentes y la activación de los módulos previos se conservaron. Facturas queda disponible en el ambiente y desactivado hasta su activación administrativa. Salud HTTPS 200, acceso sin sesión 401 y 35 archivos públicos verificados. Ver `modulos/facturas-implementacion.md` para alcance y límites.

## Colaboración

El release incluye la migración propia del módulo y el detector `file`/libmagic en la imagen. Antes de activarlo en AWS, verificar su migración, el IAM limitado a `colaboracion/*`, versionado/Object Lock del bucket, carga MIME y timeout del proxy. La activación requiere una comprobación administrativa. Pasos, estado vigente y límites en `modulos/colaboracion-implementacion.md`.

El 2026-10-24 se publicó el commit `14b1bf6` con `Core checks` y `Scout control checks` exitosos, y se desplegaron sus artifacts verificados en AWS/Bluehost. Se aplicó la migración `202610230001`, se respaldó la base y se cotejaron las 50 tablas previas; salud pública 200 y 36 hashes de assets correctos. En ese corte, Colaboración quedó fuera de `BASTION_ALLOWED_MODULES` y desactivado porque la identidad del servicio no podía enumerar versiones del prefijo S3. La activación y la prueba real se completaron el 2026-10-26, según el registro siguiente.

## Gastos: preparación de activación

El usuario autoriza activar el registro general sólo para el Administrador del sistema. El estado inicial comprobado por SSH ejecuta `ca383c7` en AWS/Bluehost, con migración 1 aplicada, readiness 200 y Gastos desactivado/fuera de allowlist. Las correcciones de paridad y edición añaden migración 2 (borrado lógico), cálculo previo y resumen filtrado. Esperar CI de ese commit, verificar el artifact, respaldar MySQL/frontend, aplicar `migrate`, agregar `gastos` a `BASTION_ALLOWED_MODULES` y publicar en ambos hosts. Activar mediante el Core sólo tras verificar; no conceder acceso a otros roles ni importar registros Legacy. Ver [alcance y evidencia](modulos/gastos-implementacion.md).

El código `a7757d3` pasó CI (`40169034037`) y ambos artifacts están verificados en staging privado (servidor: 5 archivos; web: 41). El 2026-10-26 las correcciones reemplazaron la versión activa y se completaron migración 2 y allowlist; permanece pendiente únicamente la activación administrativa. El registro de gastos está vacío y no hay grants a roles no administradores. No se crearon datos de negocio sintéticos.

- 2026-10-26: el usuario autoriza expresamente desplegar Gastos usando el acceso MySQL existente, igual que en Facturas y Cotizaciones. Para este despliegue se permite reutilizar la credencial del operador únicamente en el proceso puntual de migrate, con respaldo privado previo y comparación de los datos existentes. No se crean cuentas ni se cambian privilegios, y MYSQL_MIGRATOR_* no se persiste en el servicio. Esta excepción operativa no cierra el pendiente general de separar la credencial de ejecución de la credencial de migraciones.

El 2026-10-26 se instalaron los artifacts de `a7757d3` (CI exitoso `40169034037`) en AWS y Bluehost. La versión previa inmediata era `d5e7ac5`, incluida en este release. Se verificaron respaldo privado de MySQL y frontend, conservación de las 61 tablas existentes y sus datos, y conservación de la activación de los demás módulos. Migraciones de Gastos 1 y 2 correctas; `deleted_at` incorporado, `gastos` permitido en el ambiente y disponible con `enabled=0`, pendiente de activación desde Ajustes por un Administrador. Readiness HTTPS 200, API anónima 401, 41 archivos frontend verificados en disco y 39 recursos públicos comprobados por HTTPS (se excluyen los dos .htaccess). Cero registros de gastos y cero permisos del módulo para otros roles; no se importaron datos Legacy ni se crearon registros de negocio sintéticos. Se preservaron config.js, .htaccess y assets anteriores para caché. Se utilizó el acceso MySQL existente autorizado por el usuario sólo para el proceso puntual de migración, sin crear cuentas, cambiar privilegios ni persistir variables de migrador.


## Colaboración: activación verificada

El 2026-10-26 se permitió y activó Colaboración sobre la release vigente `a7757d3`, con respaldo privado de configuración y las 61 tablas, sin modificar binarios, esquema ni estados de los otros ocho módulos. La identidad del backend pasó escritura/lectura con integridad, AES256, rechazo anónimo 403 y eliminación de dos versiones y un marcador del objeto sintético exacto bajo `colaboracion/empresa/`, sin residuos. Se añadió `colaboracion` a la allowlist, se verificó la migración ya instalada y se registró la activación operativa con actor de sistema en auditoría. Estado final `1/1/1`; salud HTTPS 200, frontend y chunk de Colaboración verificados y API anónima 401. Se conserva el pendiente de prueba integral autenticada y de carga máxima por el proxy. Ver `modulos/colaboracion-implementacion.md`.

## Mantenimiento del repositorio antes de publicar

Sincronizar las referencias de `origin/main` antes de implementar o preparar un release. Revisar divergencias y equivalencia de commits con `git cherry`; conservar un respaldo recuperable del historial y de los archivos locales antes de reconciliar. Las copias antiguas de módulos ya publicados se comparan con su historial para incorporar sólo los cambios nuevos sobre la versión vigente. Los respaldos operativos quedan fuera de Git y no se suben con el código.

## Licitaciones: infraestructura local sin despliegue

El 2026-10-27 comienza la [infraestructura local](modulos/licitaciones-infraestructura.md), autorizada sin campos ni formularios precargados. `migrate` incorpora `_licitaciones_migrations` con versión 1 y la nueva migración `202610270001` de Colaboración para esquemas publicados. El arranque verifica checksums y registra Licitaciones inicialmente deshabilitado. No se cambia la allowlist de ningún host; `licitaciones` requiere inclusión explícita y activación administrativa independiente tras respaldo, CI y migraciones verificadas. La entrega de metadatos no requiere cambiar IAM/S3/Typst. No instala archivos ni plantillas ejecutables ni modifica el origen Facturas. Los límites de 20 MB por entrada y 500 MB independientes por expediente/PDF están en validadores puros; transporte binario, capacidad superior a 100 páginas y presupuestos operativos siguen pendientes de implementación y medición. No acredita despliegue.

## Reportes: despliegue en producción verificado

El módulo añade su migración de cuota y consultas dentro de Órdenes, Facturas y Gastos. Para desplegar, usar artifacts del mismo commit con CI exitoso, respaldar MySQL y frontend, ejecutar `migrate` con la identidad de migración autorizada, agregar `reportes` a `BASTION_ALLOWED_MODULES` y verificar salud y estados. La activación administrativa y los permisos por dominio se revisan por separado. Mantener frontend en Bluehost y backend en AWS. Ver [alcance y estado](modulos/reportes-implementacion.md).


El 2026-10-26, con autorización del usuario para producción, se instalaron en AWS y Bluehost los artifacts del commit `02c1d89`, después de completar [Core checks](https://github.com/Ahcast/Bastion/actions/runs/40430689605) y Scout control checks. Se verificaron ZIP, tar, procedencia del commit y hashes de archivos antes de publicar. Se conservaron respaldos privados de MySQL, configuración y frontend, además de la versión anterior para reversión.

La migración propia 1 añadió `_reportes_migrations` y `reportes_actor_limits`: el esquema y los datos de las 61 tablas previas se cotejaron sin diferencias inmediatamente antes y después de migrar. El total pasó a 63 tablas. Se conservaron la activación de los demás módulos y los permisos existentes; no se importaron datos Legacy ni se crearon registros de negocio sintéticos.

Reportes está incluido en el release y permitido en el ambiente, con estado `1/1/0`: disponible y desactivado, pendiente de activación administrativa desde Ajustes. No se añadieron permisos a roles. Cada reporte requiere su permiso de Reportes y la lectura del dominio fuente.

El frontend verificó los 43 archivos del paquete en disco y 41 recursos públicos por HTTPS contra sus hashes. Se preservaron `config.js`, ambos `.htaccess` y los assets anteriores para cachés. La ruta SPA respondió 200, readiness HTTPS respondió 200 y los tres endpoints de Reportes sin sesión respondieron 401.

El usuario autorizó reutilizar la credencial MySQL existente sólo durante el proceso puntual de `migrate` de Reportes. No se crearon cuentas, no se cambiaron privilegios y `MYSQL_MIGRATOR_*` no se persistió en la configuración ni en el proceso del servicio. Esta excepción no cierra el pendiente general de separar las identidades de ejecución y migración.

## Órdenes de compra: autorización de migraciones

El 2026-10-27 el usuario autoriza usar la credencial MySQL existente para completar las acciones de Órdenes y la corrección de utilidad. Se permite reutilizarla únicamente en el proceso puntual de migrate, con respaldo privado previo y verificación de datos. No crear cuentas, modificar privilegios ni persistir MYSQL_MIGRATOR_* en el servicio. Esta excepción operativa no cierra el pendiente de separar identidades. Utilidad autorizada: total de venta menos ISV menos inversión total, incluidos productos y gastos.

## Órdenes: acciones y utilidad desplegadas

El 2026-10-27 se instalaron en AWS y Bluehost los artifacts del commit 50efa7ebda634fb1c7dcd7d8baa26179522be9ca, con [CI aprobado](https://github.com/Ahcast/Bastion/actions/runs/40540732266). Se habilitan técnicamente clonación, archivos privados por producto y conversión única a borrador de factura, sin emisión fiscal. La utilidad en Órdenes y Facturas es total de venta menos ISV menos inversión total.

Se verificó el respaldo privado y se ensayó su restauración íntegra en 63 tablas temporales, comparando todas sus filas con las originales. El ensayo usa hexadecimal para campos binarios y UTC para timestamps; las tablas temporales se retiraron sin sustituir datos de producción. Las migraciones propias de Órdenes y Facturas quedaron en 4 y document-flows en 2; el total de tablas pasó de 63 a 69. Las seis nuevas tablas quedaron vacías, sin datos de negocio sintéticos.

La comparación comprobó que sólo se corrigió profit en la orden existente; no había borradores de factura ni recibos anteriores que corregir. Se conservaron precios, impuestos, inversión, versiones, origen, permisos, configuración y estados de todos los módulos. Se usó la credencial MySQL autorizada sólo para migrate; MYSQL_MIGRATOR_* no quedó en el archivo de entorno ni en el proceso del servicio.

Frontend: 44 archivos verificados en disco y 42 recursos por HTTPS, conservando config.js, ambos .htaccess y assets previos. SPA y readiness respondieron 200; Órdenes, documentos, disponibilidad de archivos, Facturas y Reportes sin sesión respondieron 401. Los artifacts y respaldos conservan reversión privada. La prueba integral de interfaz autenticada en producción queda pendiente; los flujos de dominio, HTTP, permisos, archivos y migraciones pasaron CI con MySQL 5.7.


## Licitaciones: backend desplegado en AWS

El 2026-10-27 se instaló el backend de 0d951d5 con CI aprobado, artefacto verificado, respaldo privado y restauración ensayada de las 69 tablas previas. Migraciones de Licitaciones 1 y Colaboración 202610270001; total de 77 tablas y conservación de datos y estados de los demás módulos. Licitaciones queda disponible y desactivado (1/1/0). Readiness HTTPS 200 y rutas privadas anónimas 401. El frontend Bluehost no se publicó en esta operación. Ver [evidencia, recuperación y pendientes](modulos/licitaciones-despliegue.md).


## Facturas: autorización de migración 5

El usuario autoriza el 2026-10-27 reutilizar la credencial MySQL existente únicamente para la migración 5 de Facturas, después de respaldo privado verificado. Usar el comando acotado `migrate-facturas`; no migrar otros módulos, crear cuentas, cambiar privilegios ni persistir `MYSQL_MIGRATOR_*` en el servicio. El runtime de documentos necesita acceso al prefijo privado `facturas/` en el bucket existente. Mantener las activaciones y configuración existentes; no generar documentos comerciales sintéticos ni consumir números SAR durante la verificación. Ver `modulos/facturas-acciones.md`.

## Facturas: acciones desplegadas

El 2026-10-27 se instalaron en AWS y Bluehost los artifacts verificados de `e5c8d9f`, después de [CI aprobado](https://github.com/Ahcast/Bastion/actions/runs/40587735942). Respaldo privado y restauración ensayada de 77 tablas antes de aplicar exclusivamente Facturas 5; total posterior 81 tablas. Se preservaron la factura existente, todos sus campos anteriores, otras tablas, configuración y activaciones. Sin números SAR ni operaciones financieras de prueba; credencial migradora temporal conforme a la autorización. Frontend: 45 archivos verificados y 43 hashes públicos, configuración y assets anteriores conservados. Readiness 200 y rutas privadas anónimas 401. Emisión, pagos, clonación, PDF/entrega/recibo y archivo lógico están desplegados; la purga de versiones S3 requiere los permisos mínimos pendientes del prefijo `facturas/`. Ver [evidencia, respaldos y límites](modulos/facturas-acciones.md#publicación-verificada-del-2026-10-27).

## Facturas: permisos S3 verificados

El 2026-10-27, después de que el usuario agregó los permisos acotados a `facturas/*`, se verificaron con la identidad del servicio la enumeración y eliminación permanente de versiones, escritura/lectura con hash, cifrado AES256 y acceso anónimo 403. Se eliminaron todos los remanentes sintéticos de ambas comprobaciones y se confirmó que los prefijos de prueba quedaron vacíos. Se cierra el pendiente IAM registrado arriba; sin cambios en datos financieros, numeración, credenciales, activaciones o binarios. Ver [cierre y alcance](modulos/facturas-acciones.md#cierre-de-permisos-s3--2026-10-27).

## Facturas: recibo independiente del pago previo

El 2026-10-27 se desplegó en AWS y Bluehost `204f90e`, con [CI aprobado](https://github.com/Ahcast/Bastion/actions/runs/40621961030): Emitir Recibo ya no requiere pagos registrados. El documento refleja el saldo real y no registra pagos ni cambia la factura. Actualización de código sin migraciones, con respaldos privados y comprobación de facturas, pagos y activaciones. Se verificaron 47 archivos del frontend y 45 hashes públicos; configuración y assets anteriores conservados, readiness 200 y rutas privadas anónimas 401. Ver [regla y validación](modulos/facturas-acciones.md#recibo-sin-pago-previo--2026-10-27).

## Facturas: campos comerciales y fiscales

El 2026-10-27 se desplegó en AWS y Bluehost `c6d2124`, con [CI aprobado](https://github.com/Ahcast/Bastion/actions/runs/40623822103). Se muestran en el editor los tres números fiscales ya almacenados, y el PDF de factura incluye esos números, términos y descuento. El editor informa que cualquiera de los tres números aplica la exoneración existente a todos los ítems. Se preservaron facturas, pagos y activaciones, sin migraciones; respaldos privados y verificación de 47 archivos locales, 45 hashes públicos, readiness 200 y rutas privadas anónimas 401. Ver [alcance y pruebas](modulos/facturas.md#campos-comerciales-y-fiscales--2026-10-27).


El 2026-10-28 se desplegó en AWS el backend `43e65be2ec16b712b310abd3a47f683715149bc2` tras [Core checks](https://github.com/Ahcast/Bastion/actions/runs/40624114593) y Scout control checks exitosos. La extensión `.typ` se valida como `text/plain`; `.png` y `.jpeg` siguen habilitadas por defecto. Se verificaron procedencia y hashes del artifact, release previa preservada, comparación sin cambios del registro de módulos, ledger de migraciones y `files.policy`, readiness HTTPS 200 y endpoint anónimo de ajustes 401. No se aplicaron migraciones ni se cambió la configuración/activación. La política persistida no tenía fila y aplica defaults. No se hizo carga autenticada de archivo en producción.

## Facturas: corrección SAR, proforma y recibo

El 2026-10-28 se desplegó en AWS y Bluehost `cac1fa4`, con [CI aprobado](https://github.com/Ahcast/Bastion/actions/runs/40677322367). El rango SAR admite espacios alrededor de sus guiones sin alterar la configuración y mantiene las validaciones de secuencia, prefijo, límites y fecha. El PDF de borrador se presenta como Proforma con totales compactos a la izquierda; el recibo requiere factura emitida y usa el texto y total en letras solicitados, sin registrar pagos. Respaldo privado previo, sin migraciones ni consumo de números SAR; 47 archivos frontend, 45 hashes públicos, readiness 200 y rutas privadas anónimas 401. No se emitió una factura real durante la verificación. Ver [detalle y diagnóstico](modulos/facturas-acciones.md#emisión-sar-y-presentación--2026-10-28).

## Facturas: edición de enviadas

El 2026-10-28 se desplegó en AWS y Bluehost `ae475e8`, con [CI aprobado](https://github.com/Ahcast/Bastion/actions/runs/40679375678). Las facturas Enviadas admiten edición; conservan número SAR, pagos y snapshots, y su saldo se recalcula sin permitir un total inferior a lo pagado. Pagado continúa sin edición y se mantienen las reglas de eliminación. Sin migraciones, con respaldos privados y verificación de datos/activaciones, 47 archivos frontend, 45 hashes públicos y readiness 200. Ver [evidencia](modulos/facturas-acciones.md#edición-de-facturas-enviadas--despliegue-2026-10-28).

## Facturas: ajuste visual de PDF

El 2026-10-28 se desplegó en AWS `07c1877`, con [CI aprobado](https://github.com/Ahcast/Bastion/actions/runs/40682083139): totales compactos en el lado derecho con contenido alineado a la izquierda y título «Factura Proforma». Revisión visual de ambos PDF sintéticos, respaldo previo, sin migraciones; datos y activaciones preservados, readiness HTTPS 200 y documentos privados anónimos 401. El frontend no requirió cambios. Ver [detalle](modulos/facturas-acciones.md#diseño-de-factura-y-proforma--2026-10-28).

El 2026-10-28 se publicaron y desplegaron los lotes y el orden de Licitaciones desde df7ed34c8e04c040ac3330e06c6bd8aa5b22c78a. El workflow 40814759068 terminó correctamente; los artefactos se verificaron antes de instalarlos. AWS conservó las 84 tablas anteriores, verificó una restauración de prueba y aplicó la migración 6 (85 tablas), sin cambiar los expedientes existentes ni la activación de módulos. Bluehost publicó el frontend con respaldo y configuración preservada; se verificaron 45 archivos, las rutas HTTPS y los controles de autenticación. La generación del PDF, las plantillas originales, su servicio aislado y el almacenamiento temporal de gran tamaño siguen pendientes. El trabajador todavía no se instaló ni activó.

### Preparación del PDF de Licitaciones

Antes de habilitar el PDF: verificar CI y hashes del commit, respaldar la base y la versión activa, aplicar únicamente la migración de Licitaciones, instalar worker, conversor, socket y recuperación con propietarios y permisos restringidos, preparar el directorio de trabajo y el punto de montaje al arranque, y activar el ajuste del servicio. El backend debe conservar `NoNewPrivileges` y usar el socket local autenticado; el trabajador no recibe credenciales ni red. Publicar el frontend solo con la API compatible. La activación y los controles posteriores deben registrarse con evidencia; los ejemplos de `deploy/bastion-tender-*` no equivalen a una instalación.

El 2026-10-29 se desplegó el PDF de Licitaciones desde `d6e82df`, con [CI aprobado](https://github.com/Ahcast/Bastion/actions/runs/40986354983) y artefactos verificados. Antes de migrar se respaldó la base y se comprobó la restauración de las 85 tablas existentes; la migración 7 añadió una tabla, preservando el estado de los módulos. Licitaciones permanece incluido, permitido y habilitado (`1/1/1`). El backend responde readiness 200, conserva `NoNewPrivileges` y tiene activos el socket y el temporizador de recuperación. Bluehost publicó el frontend con respaldo, configuración preservada y 45 recursos verificados por HTTPS. La prueba integral de CI generó y descargó un PDF con TIFF multipágina; también se verificó la composición aislada con los formularios originales. Quedan pendientes una prueba autenticada del flujo real y una medición de archivos cercanos al límite de 1 GB.


## Respaldo integral de datos

El Core centraliza la administración de copias y recuperación mediante un trabajador privado persistente en AWS. Se conserva frontend Bluehost y MySQL Bluehost. Véase [alcance, recuperación y límites](18-respaldo-mysql.md).

## Colaboración: vista previa PDF corregida

El 2026-11-01 se publicó en GitHub y Bluehost el commit [fe13c90](https://github.com/Ahcast/Bastion/commit/fe13c90c7274f6aef5039b7e26a19cb8437d3345), con [Core checks aprobados](https://github.com/Ahcast/Bastion/actions/runs/41351824281). La vista de PDF deja de usar un iframe con sandbox que bloqueaba el visor integrado del navegador. Reserva una pestaña durante el gesto del usuario, separa `opener`, valida MIME/tamaño/firma PDF antes de mostrarlo y revoca el URL temporal al cerrar o al vencer el plazo. No se relajó CSP ni se publicaron documentos privados.

Bluehost activó el artifact estático del commit después de verificar SHA-256, `BUILD.txt` y sus 48 archivos del manifiesto. Se creó un respaldo privado previo (SHA-256 `5f87d38b65484e44ec735fae23709d59404cbfef625214ecb739ddb3a3bf4e67`); se conservaron `config.js`, los dos `.htaccess` y assets con hash previos. Cinco recursos HTTPS respondieron 200: SPA, `BUILD.txt`, `config.js`, ruta `/colaboracion` y el chunk actualizado del módulo. No hubo cambios en AWS, API, datos, permisos, allowlist ni activación.

## Licitaciones: tamaño de tabla por lote

El 2026-11-01 se desplegaron el backend `3337fc2a4856` y el frontend `25f068945b93`, con las compilaciones de GitHub Actions `41367839449` y `41367860221` aprobadas y los paquetes verificados. La migración 10 agregó el tamaño de tabla a cada lote; los cinco lotes existentes quedaron en 7 pt. Antes de migrar se respaldó la base y se verificó la restauración de 87 tablas. La API y la ruta web respondieron por HTTPS; no se hizo una prueba autenticada en un expediente real.

## Colaboración: edición de títulos y publicación de formatos

El 2026-11-03 se publicó el commit e1e502723b22ca73d0074ac5db2702eaf289e98f, con [Core checks aprobados](https://github.com/Ahcast/Bastion/actions/runs/41609726208): frontend, formato, compilación y pruebas. La integración conserva además los cambios publicados del selector de archivos de Licitaciones. La edición de título modifica únicamente metadatos, requiere permiso editar en la carpeta del archivo y queda auditada; el objeto y su clave S3 no cambian. El backend acepta .typ como texto UTF-8 normalizado a text/plain, además de los formatos de imagen ya permitidos.

Los artifacts del mismo commit se verificaron y desplegaron en AWS y Bluehost. AWS usa actualización sólo de código: no se ejecutaron migraciones ni se cambiaron IAM, configuración, datos o estado del registro. Readiness respondió 200, el manifiesto instalado coincidió, la migración de Colaboración siguió en 202610230001, y el registro permaneció 1/1/1. Bluehost publicó los 49 archivos del paquete; se conservaron config.js, .htaccess, assets/.htaccess y los assets antiguos. Se creó respaldo privado del sitio (SHA-256 2796348c3333064ed486e33399c0eb3d50471f27caa07d5bd6cde991867e6661). La verificación HTTPS confirmó cuatro recursos de Colaboración con hash correcto, la ruta SPA /colaboracion, readiness 200 y API sin sesión 401.

La causa conocida de rechazo MIME para algunos .typ con #import era una clasificación errónea de libmagic como Objective-C; el backend actual normaliza archivos de texto UTF-8 al MIME text/plain. El mensaje genérico de carga reportado para dj-derechosautor.typ no se pudo correlacionar con una solicitud autenticada, por lo que queda pendiente validar la carga de ese archivo concreto con el usuario; no se hizo una carga de prueba en producción.

## Cotizaciones: descripciones contenidas en PDF

El 2026-11-03 se desplegó únicamente en AWS el backend `a214a751b1ce877d2d1d7922d6822ca6153346b7`, con [Core checks aprobados](https://github.com/Ahcast/Bastion/actions/runs/41708769364). Concepto conserva el ancho ampliado; las columnas numéricas tienen anchos acotados y los códigos o enlaces largos admiten saltos dentro de la descripción, sin modificar los datos almacenados. La reproducción visual comparó un código sin espacios que desbordaba con su versión corregida, descripciones extensas, enlaces y paginación.

Se verificaron ZIP, paquete, BUILD.txt y manifiesto del artifact antes de instalarlo; SHA-256 del ZIP `5d1a88d737f4022e80b10ddb0f6cda12d5ca507025af953f001ba87786aeb64d`. Respaldo privado MySQL completo y comprobado (9 943 373 bytes, SHA-256 `9718f93dfd93cda0e8b7dcb6cdc5a801bf9260eae2174b118845c9e3087fdd87`), configuración y release anterior conservadas para reversión. Se cotejaron los hashes de los datos/cálculos de cotizaciones, su ledger y el registro de módulos antes y después. Sin migraciones, cambios de configuración ni activaciones. El proceso activo contiene la plantilla corregida; readiness HTTPS 200 y PDF anónimo 401. Se conservaron los cambios activos de las PR #35 y #36; Bluehost y el worker de Licitaciones no requirieron publicación. No se generaron cotizaciones sintéticas en producción.

## Licitaciones: JICA y desglose de ISV incluido

El 2026-11-04 se fusionó la [PR #39](https://github.com/Ahcast/Bastion/pull/39) en `3ccb391010d352363f17261a376a43273892ffd7`, con [Core checks aprobados](https://github.com/Ahcast/Bastion/actions/runs/41813285427). AWS y Bluehost activaron los artifacts del commit de integración probado `971519f38301763f6ea9e88160d1c1be33343650`, cuyo árbol coincide con la fusión. Se verificaron los hashes ZIP de GitHub, paquetes, BUILD y manifiestos. Sin migraciones, cambios de IAM, configuración ni activaciones de módulos. La versión anterior del backend permanece disponible para reversión; Bluehost creó y comprobó un respaldo privado del sitio (SHA-256 `34126fda1835d181e8677f85319e9b5724fdfddf94ab60d2630bdd36105c306b`).

JICA permite destino global DDP, país de origen y precio unitario con ISV y transporte incluidos. La presentación muestra el total de la oferta en cifras y letras, bienes = total / 1,15 redondeado a centavos e impuestos = total menos bienes. El cambio hacia o desde JICA requiere confirmar el descarte de ítems. Se conserva tamaño de tabla por lote, inicialmente 7 pt. Las dos plantillas privadas aprobadas se instalaron sin publicar su contenido en Git. La compilación y revisión visual de presentación (dos páginas) y lista de precios (una página) con datos sintéticos verificaron total 250,00, bienes 217,39 e impuestos 32,61. CI comprobó también la vista previa JICA por el trabajador aislado. No se modificaron expedientes reales.

La comprobación posterior verificó 47 hashes públicos, tres rutas SPA, readiness HTTPS 200 y siete rutas privadas con respuesta anónima 401. Se conservaron config.js y reglas Apache. El worker instalado permanece en SHA-256 `175e358a0688a5e64501fe3293afe3b41c7e023c3e71af633ea57fbc4b173fa4`, con la numeración elevada de la PR #38. También se verificó que continúa instalada la lista STD con empresa y fecha. Queda pendiente la validación del usuario sobre un expediente real.


## Autenticación por petición: optimización desplegada

El 2026-11-04 se instaló en AWS `d6a697593e9246d696bd7139132d16dfe6bfccfc`, después de [Core checks](https://github.com/Ahcast/Bastion/actions/runs/41871641000), MCP MySQL checks y Scout control checks exitosos. Se verificaron el SHA-256 del artifact ZIP, del tar y del ejecutable, junto con la procedencia del commit. La versión previa se conserva para reversión.

La consulta de sesión incorpora el nombre de la persona y la API reutiliza la identidad únicamente dentro de la misma petición vinculada a su token. En rutas con autenticación repetida entre filtro y controlador, como el detalle de Cotizaciones, esa parte pasa de seis operaciones SQL a dos. No se almacena autorización entre peticiones ni se modifican cuotas, CSRF, permisos o reglas de dominio. No cambia la arquitectura ni se ejecutan migraciones; el frontend Bluehost y los servicios de Scout, documentos y respaldos se conservan.

La verificación posterior confirmó readiness HTTPS 200, rutas privadas anónimas 401, identidad falsificada 401 y mutación sin CSRF 403. Se retiró el directorio temporal de instalación; no se crearon cuentas ni registros comerciales de prueba en producción. Las pruebas locales cubrieron cinco casos de aislamiento de identidad y tres regresiones de cookies, origen y CSRF; CI ejecutó las suites de integración con bases desechables. La reducción de consultas está verificada por el flujo de código y las pruebas; falta una medición autenticada de extremo a extremo de los botones concretos para cuantificar la mejora percibida.

## Licitaciones: registrar una oferta enviada

El 2026-11-05 se publicó la [PR #40](https://github.com/Ahcast/Bastion/pull/40) en AWS y Bluehost con los paquetes verificados de `933f44110f760ef7e115bde6496c17846fe0e037`. El árbol coincide con la fusión `5f26e7c3bd00db30ab19ea3c448dfc537902cf00`. La [validación de GitHub](https://github.com/Ahcast/Bastion/actions/runs/41977280464) comprobó la transición con versión vigente, el bloqueo de edición y eliminación, y la descarga del PDF después de marcar como enviada.

Se conservó la versión anterior del backend y se verificó un respaldo del sitio. La actualización fue de código y no requirió migraciones. Readiness respondió 200, se cotejaron 47 hashes públicos y la nueva acción rechazó una solicitud anónima con 403. La prueba integral utilizó un expediente sintético.

## Cotizaciones: envíos programados y campos decimales

El 2026-11-06 se publicaron en AWS y Bluehost los paquetes verificados de `fd77345e908b95ebbbd1fd263c3ea6e4134cc5bd` tras [Core checks aprobados](https://github.com/Ahcast/Bastion/actions/runs/42341148347). El estado del listado muestra el próximo envío pendiente en hora de Honduras. Los campos decimales rechazan caracteres inválidos y el descuento por ítem tiene opciones visibles HNL y %. Las 164 pruebas de frontend y las suites Rust/MySQL pasaron; el navegador confirmó ambos controles sobre una cotización sintética local.

Se verificaron los respaldos privados de MySQL (SHA-256 `1ad4c555fe604538ce8c10b8680983825be22e1991fc608bd5a799095202b07c`) y del sitio (SHA-256 `9b7b442333e9f197b3b4e1d0f09e9c9a2cfbe2b63098b10f5213ad43df8dfa12`). Se conservaron los datos de cotizaciones, las activaciones, el registro de migraciones, el `config.js` operativo con su opción de MCP, las reglas de Apache y los recursos previos. No se ejecutaron migraciones ni se enviaron correos de prueba en producción. API readiness 200, acceso anónimo 401, ruta Cotizaciones 200 y 47 recursos públicos verificados por HTTPS.

## Cambio de autenticador TOTP: despliegue del 2026-11-10

Se activó en AWS y Bluehost el commit `d121e80b5236188501384c14b28018c879a36430`, sobre la versión anterior `4206409839fabbbc7cf6f0cbe027a027e2fbbcf7`, con [Core checks aprobados](https://github.com/Ahcast/Bastion/actions/runs/42728668810), MCP MySQL checks y Scout control checks exitosos. El frontend ya ofrecía cambiar el autenticador, pero la API publicada devolvía 404. El backend actualizado incorpora el inicio y la confirmación del cambio; exige contraseña y TOTP actuales, verifica el nuevo autenticador y al completar regenera los códigos de recuperación y revoca accesos.

Se verificaron los ZIP de GitHub, los paquetes y sus manifiestos antes de publicar. SHA-256 del ZIP del backend: `356a0926b8facdad0fdf8070d361233a8c331db05d1513acb771d7331881bf56`; frontend: `b8d632b0e63afd59ab2cf9b797a8217171a2a36c3776d6767033f32878cfe6a0`. El paquete incluye también las correcciones de correo, validación Office y bajas de ese commit. Se aplicaron únicamente las migraciones pendientes Clientes 2, Proveedores 2 y Órdenes de compra 5, que añaden `archived_at` inicialmente nulo. Se conservó un respaldo privado de MySQL y se ensayó su restauración mediante tablas temporales; las 105 tablas, sus datos anteriores y los estados de activación se comprobaron antes y después. La credencial de migración se suministró sólo al proceso puntual, sin persistirla en el servicio.

La versión anterior del backend y la configuración se conservaron para reversión. Bluehost respaldó los archivos reemplazados y preservó `config.js`, ambas reglas `.htaccess` y los assets previos. Se comprobaron 46 archivos por HTTPS, la configuración pública conservada y readiness 200. Ambos POST de TOTP responden 401 a solicitudes sin sesión con origen y CSRF válidos; ya no devuelven 404. No se cambió el autenticador ni se crearon cuentas de prueba en producción: la confirmación completa con el autenticador del usuario queda como verificación interactiva pendiente. No se actualizó el servicio independiente de Scout ni los trabajadores de documentos o respaldos.
