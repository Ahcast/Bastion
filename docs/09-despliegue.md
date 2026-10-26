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
