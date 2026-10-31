# Respaldo diario de MySQL

El usuario autoriza una copia diaria a las 17:00 de Honduras en `bastion-prod-assets/backups/mysql/`. Se instala `bastion-mysql-backup.timer` en Lightsail; el servicio utiliza las credenciales MySQL y el perfil AWS existentes mediante credenciales de systemd, TLS con verificación de identidad y una exportación transaccional comprimida. El manifiesto incluye tamaño y SHA-256, y solo se publica tras una exportación y carga exitosas. Las cargas sin manifiesto no son respaldos completos.

El 2026-10-30 se activó el temporizador. Su próxima ejecución observada es 2026-10-31 23:00 UTC. El usuario solicitó no ejecutar pruebas; la primera ejecución programada y la restauración no se han comprobado. Una copia puntual anterior se subió al mismo prefijo y se confirmó la existencia del objeto y manifiesto en S3.

La retención de 30 días no está configurada por esta entrega: el usuario gestionará la política de S3. Limitar cualquier regla a `backups/`, respetando versionado y otras reglas. Esta copia no incluye documentos S3, archivos de configuración ni la clave de cifrado de Bastion. Los documentos y el respaldo de la base comparten bucket, por lo que su pérdida total afectaría a ambos. La protección de claves, restauración aislada, alertas y recuperación integral permanecen pendientes.

