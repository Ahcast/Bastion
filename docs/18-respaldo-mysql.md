# Respaldos y restauración de datos

El Core autoriza exclusivamente a administradores mediante sesión y CSRF existentes. La API delega en un controlador privado por socket Unix; los trabajos y su auditoría sobreviven a la petición web en SQLite. Configuración > Respaldos y restauración permite copia manual, historial, descarga temporal, subida por bloques, preparación y confirmación de recuperación.

## Datos

Cada ejecución produce un ZIP firmado con manifiesto, commit, inventario, tamaños y SHA-256. Incluye la base MySQL completa, SQLite y datos persistentes de Scout/control, branding local y objetos de cotizaciones/, colaboracion/, ordenes_compra/, facturas/ y licitaciones/. Las plantillas/documentos de usuario almacenados en esos prefijos están incluidos. No se enumera backups/; no se incluyen código, configuración de infraestructura, credenciales AWS/MySQL, cachés ni temporales. Un cambio futuro del catálogo de almacenamiento requiere actualizar el registro cerrado de prefijos y rutas.

El ZIP se valida en flujo antes de subirlo, con límites: 40 GiB de archivo recibido, 100 GiB expandidos, 20 GiB por entrada, 100.000 entradas y manifiesto de 4 MiB. Se rechazan rutas inseguras, duplicados, archivos especiales, compresión desproporcionada y respaldos anidados. La recuperación exige firma válida y el mismo commit del backend; para una copia antigua se despliega primero esa versión compatible.

## Programación y almacenamiento

Se reutiliza bastion-mysql-backup.timer a las 17:00 America/Tegucigalpa. Su servicio solicita la copia al controlador, espera el resultado y reintenta fallos cada 15 minutos. No hay otro temporizador de respaldo. Destino: bastion-prod-assets/backups/bastion-backup-AAAA-MM-DD-HHMMSS.zip. El prefijo backups/bastion-backup- queda reservado exclusivamente a los ZIP generados por Bastion. El ciclo de vida elimina las copias actuales a los 30 días y sus versiones no actuales después de un día adicional; S3 procesa estas acciones de forma asíncrona. Las reglas existentes se conservan; otros prefijos y los respaldos antiguos de MySQL no se eliminan por esta regla.

La clave de cifrado de datos de la aplicación se conserva fuera del ZIP en backups/recovery/<sha256>, con SSE-S3 y sin expiración automática. No es una credencial de infraestructura. Permite recuperar campos cifrados ante pérdida del servidor y verificar la firma del archivo. El ZIP y la clave requieren el acceso privado IAM al bucket; no se necesitan claves del disco del servidor perdido para descifrar SSE-S3. La cuenta AWS y sus mecanismos de recuperación deben conservarse independientemente. Compartir bucket con los originales no protege ante la pérdida total de ese bucket o de la cuenta.

## Consistencia y recuperación

Se bloquea Scout y se pausa el Core y los trabajadores de PDF durante la instantánea. Se rechazan tablas MySQL no transaccionales. SQLite usa su API de copia y S3 verifica la identidad de los objetos al descargarlos. La conexión de la interfaz puede interrumpirse durante mantenimiento; el trabajo continúa independientemente.

Una recuperación preparada muestra el alcance y caduca en una hora. La confirmación se vincula al administrador y al hash del ZIP. Se crea primero un punto de recuperación real. La importación MySQL usa modo binario para desactivar comandos locales del cliente; solo se aceptan archivos firmados por Bastion. Se restauran únicamente los espacios de datos admitidos, nunca instalación, código ni backups/. Se revocan sesiones y dispositivos confiables. Si ocurre una modificación parcial se conserva mantenimiento y la auditoría; un administrador debe resolverla por SSH antes de reactivar servicios. La recuperación total destructiva de producción no se ensaya como verificación.

## Pérdida de Lightsail

1. Descargar el código de GitHub y desplegar el commit indicado por el manifiesto. Configurar servicios, MySQL y credenciales IAM fuera de Git.
2. Recuperar el ZIP desde S3 y configurar los directorios persistentes y la clave de aplicación desde backups/recovery/<key_id>. Mantener la aplicación detenida.
3. Detener el temporizador y el controlador antes de usar su CLI exclusiva: systemctl stop bastion-mysql-backup.timer bastion-backup-control.
4. Ejecutar como root: python3 /usr/local/libexec/bastion-backup/bastion_backup_controller.py --recover /ruta/archivo.zip --confirm RESTAURAR. Solo para una base nueva completamente vacía se permite añadir --fresh, sin punto previo de base de datos.
5. Revisar resultado y mantenimiento, iniciar los servicios autorizados y verificar integridad funcional antes de dar acceso. Iniciar sesión nuevamente.

No se almacenan contraseñas, claves IAM ni secretos de infraestructura en GitHub o en el ZIP. Los permisos IAM deben limitarse a los prefijos usados. No se crean cuentas ni registros de negocio de prueba.

Los commits marcados [backup-release] ejecutan compilación Linux, formato, tipos, build web y análisis estático, sin crear fixtures, cuentas o datos de prueba. Los demás commits conservan la suite habitual.
