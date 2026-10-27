# Despliegue del Core

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
