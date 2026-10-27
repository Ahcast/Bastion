# Licitaciones: despliegue de backend del 2026-10-27

Por solicitud del usuario se desplegó en AWS el primer corte de [infraestructura](licitaciones-infraestructura.md). Código publicado en `main`: `0d951d5cd4d2454e624fd1a759d0409b07c757c8`, con [Core checks aprobado](https://github.com/Ahcast/Bastion/actions/runs/40576582171). La versión anterior activa era `17b95a00c7308b8d8bf6147fe3556a69a15bdad5`, ancestro del cambio; no se incorporó el trabajo local de Facturas aún sin publicar.

## Artefacto y recuperación

Se verificaron el digest del ZIP de GitHub, el SHA-256 del paquete, su procedencia en `BUILD.txt`, el manifiesto del ejecutable y la correspondencia de todos los archivos extraídos con el archivo original. Paquete: `bastion-server-linux-x86_64.tar.gz`; SHA-256 `6d04f551b6bf4836af1134be0a2871c26a9304aef710077e037af23c14ed1375`. El ejecutable previo y la configuración se conservan para reversión; respaldos y diagnósticos permanecen privados, fuera del repositorio.

Antes de migrar se detuvo el servicio para estabilizar los datos, se verificó el respaldo de MySQL y se ensayó su restauración en 69 tablas temporales, cotejando todas sus filas con los originales mediante representación hexadecimal y UTC. Las tablas temporales se retiraron sin reemplazar datos existentes.

## Migraciones y disponibilidad

- Licitaciones: migración propia `1`, ledger `_licitaciones_migrations`.
- Colaboración: migración adicional `202610270001` para esquemas de captura versionados; se preservó la migración anterior y su checksum.
- Se añadieron ocho tablas: siete de datos/recibos y el ledger de Licitaciones. El total pasó de 69 a 77. Las tablas nuevas de datos quedaron vacías; no se crearon formularios, expedientes ni registros de negocio sintéticos.
- La comparación anterior y posterior a migrar conservó esquema y datos de las tablas previas; el único cambio previsto de su contenido fue la entrada nueva en el ledger de Colaboración. Tras arrancar se verificó por separado la conservación de los estados de los módulos existentes.
- Se añadió `licitaciones` a la allowlist del ambiente. Estado final `1/1/0`: incluido, disponible y desactivado. La activación administrativa es independiente del despliegue; no se añadieron permisos a roles.

Las migraciones utilizaron el acceso existente únicamente en el proceso puntual de `migrate`, siguiendo el mecanismo operativo previo. No se crearon cuentas, no se cambiaron privilegios y `MYSQL_MIGRATOR_*` no quedó persistido en configuración ni en el servicio. Sigue pendiente la separación general de las identidades de ejecución y migración.

## Verificación y límites de esta entrega

El proceso activo corresponde al ejecutable de ese commit y readiness local respondió 200. La verificación pública por HTTPS con TLS validado comprobó `/health/ready` y `/health/live` en 200; Licitaciones, biblioteca de formularios de Colaboración, Órdenes, Facturas y Reportes sin sesión respondieron 401.

El usuario solicitó Git y AWS. **El frontend de Bluehost no se publicó en esta operación**; su `BUILD.txt` seguía en `50efa7ebda634fb1c7dcd7d8baa26179522be9ca` durante la comprobación pública. Las pantallas nuevas requieren su propio despliegue estático antes de verificar el recorrido autenticado. No se activó Licitaciones ni se modificaron S3, IAM, Typst o las reglas de Facturas. Archivos, lotes/ítems, PDF y adjudicación siguen con los pendientes de la ficha de infraestructura.
