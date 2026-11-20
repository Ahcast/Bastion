# Cierre operativo del alcance publicado — 2026-11-20

Marco autoriza actualizar el backend, verificar la restauración en un entorno aislado y cerrar la revisión de los flujos prioritarios sin crear cuentas ni datos de prueba en producción. La migración del sistema anterior continúa fuera de alcance. Este documento reúne la evidencia vigente; las notas históricas de preparación de otros documentos no sustituyen esta comprobación ni acreditan ampliaciones futuras.

## Resultado

Los cuatro pendientes acordados quedan cerrados para la operación del alcance comercial publicado: backend actualizado, correo autenticado recibido, restauración integral de datos ensayada y comprobaciones funcionales de Cotizaciones, Facturas y Órdenes de compra. No se modificó la arquitectura ni se aplicaron migraciones de base de datos.

| Control | Evidencia verificada |
|---|---|
| Backend | `71d210c44926d906287eefe78a69a85e8246efc9` instalado en Lightsail a las 11:03 de Honduras; hash del ejecutable activo cotejado. Integra `lettre 0.11.24` y `email-encoding 0.4.3`. |
| Frontend | Bluehost ya publica `71d210c`; navegación con rutas, historial y enlaces para nuevas pestañas. |
| CI | [Core checks 44751212082](https://github.com/Ahcast/Bastion/actions/runs/44751212082) aprobado: formato, auditoría Rust, frontend con 199 pruebas, compilación y regresiones Rust/MySQL. |
| Servicio externo | HTTPS readiness 200 con MySQL; perfil, cotizaciones, facturas y órdenes responden 401 sin sesión. CORS permite el origen exacto de Bastion. |
| Correo | `mail.castilloit.net` resuelve directamente al servidor de Bluehost; TLS 1.3 con certificado válido. Un único mensaje autorizado se autenticó, fue aceptado por SMTP y Marco confirmó recepción en Recibidos. No se reenviaron trabajos fallidos ni se enviaron documentos comerciales. |
| Respaldo | Copia integral nueva verificada en S3 antes del cambio; 80,006,485 bytes, base completa, 10 archivos locales y 104 objetos operativos. |
| Recuperación | Motor de restauración instalado ejecutado contra MySQL 5.7.44 y API S3 aislados: 112 tablas, datos, esquemas, archivos y hashes coincidentes. Clave recuperada desde S3, cinco valores cifrados descifrables, sesiones y dispositivos de la copia revocados. |
| Funciones prioritarias | Backend nuevo sobre la copia recuperada: 41 documentos consultados y 41 recálculos coincidentes; PDF de cotización y factura generados con nombre e inline; CSRF y falta de permisos rechazados en los tres módulos. |
| Continuidad | API, controlador y temporizador de respaldos, Scout, protección de salida MCP y socket de PDF activos. Respaldo diario a las 17:00 de Honduras conservado. |
| Limpieza | Procesos aislados terminados; copias restauradas, claves temporales, MySQL, emulador S3 e instaladores retirados de AWS. Sólo se conservan respaldos reales, versiones para reversión y comprobantes privados. |

## Alcance de las comprobaciones funcionales

La comparación de recuperación se hizo antes de arrancar los trabajadores de la copia. Se cotejó el esquema y la huella ordenada de todas las filas de las 112 tablas. Sólo se excluyó de la igualdad el campo `revoked_at` de sesiones y dispositivos: la recuperación debe cambiarlo, y se verificó que ninguno quedara vigente. Las dos bases SQLite locales pasaron `integrity_check`.

La comprobación posterior inició el backend actualizado sobre esa base. Una sesión temporal y un rol sin permisos se crearon exclusivamente dentro de la base aislada para comprobar lectura autorizada, CSRF y denegación; no se crearon usuarios nuevos ni se tocaron roles o sesiones de producción. Los 41 registros comerciales recuperados se leyeron por API y sus totales se recalcularon sin guardar cambios. Los PDF se generaron únicamente en el almacenamiento aislado.

CI complementa esta evidencia con numeración fiscal, pagos y saldos, documentos, permisos por dominio, concurrencia, versiones, idempotencia, conversiones y adjudicaciones. Entre las regresiones aprobadas están `mysql57_invoice_actions_numbering_payments_documents_and_preserved_origins`, `mysql57_order_actions_private_files_clone_and_invoice_are_atomic`, `mysql57_conversions_are_atomic_authorized_lossless_and_idempotent` y `mysql57_partial_awards_and_reviewed_invoices_preserve_permissions_and_uniqueness`. No se emitieron facturas ni pagos sintéticos en producción para repetirlas.

La verificación SMTP usó las credenciales existentes y el transporte seguro hacia un destinatario autorizado, sin insertar un envío de prueba en la cola productiva. La integración de la cola, MIME e idempotencia conserva sus pruebas de CI; esta comprobación no atribuye entrega a los trabajos fallidos anteriores.

## Método y límites del ensayo de recuperación

Se descargaron desde S3 el ZIP real y su clave de recuperación independiente dentro de AWS. Se cotejaron la metadata SHA-256, el inventario, los hashes y la firma. El entorno temporal usó `chroot` y namespaces independientes de red, montajes y procesos; no tenía credenciales AWS/MySQL de producción, rutas de red externas ni acceso al administrador de servicios del host. CPU limitada al 75 % y RAM a 850 MiB.

El mismo motor de restauración recuperó una base nueva con `fresh=True`, archivos locales y objetos mediante el protocolo S3 contra Moto 5.1.17 en loopback. Sólo se sustituyó la coordinación de mantenimiento porque en ese entorno nuevo los servicios de aplicación estaban detenidos y no existía systemd del host. Se comprobaron también la eliminación de un objeto adicional del entorno aislado y la conservación del espacio de respaldos. No se sobrescribieron objetos del bucket operativo para ensayar la recuperación.

El proceso duró 197.7 segundos y alcanzó 572.3 MiB de memoria. Ese tiempo corresponde al ensayo con el entorno y el ZIP preparados; **no es una promesa de recuperación completa de infraestructura en ese plazo**. Se probó el motor de datos y la aplicación recuperada; la contratación/configuración de un servidor nuevo, DNS e IAM siguen los pasos operativos de [respaldos](18-respaldo-mysql.md).

MySQL de Bluehost reporta `5.7.44-48`; el ensayo aislado utilizó MySQL Community `5.7.44`. Se confirmó compatibilidad de esquema/datos y TLS con identidad verificada. No se cambió la versión ni la política de mantenimiento del motor administrado por Bluehost. El respaldo consistente pausa brevemente API y trabajadores; la copia previa comenzó a las 10:56 y terminó a las 11:00 de Honduras.

La protección frente a pérdida de todo el bucket o cuenta AWS sigue dependiendo de la copia independiente y política que el propietario decidió administrar en AWS. El ensayo no acredita esa política externa. Inventario físico, activos, planilla, migración histórica y distribución Android estable conservan su alcance pendiente; no son parte de este cierre de los módulos comerciales publicados.

## Comprobantes y reversión

- Backend ZIP: SHA-256 `e57aa005f4b881d79d0a746f4c6dabd007c6e9063cef580308d6c7695640c384`; tar.gz `2c4ac6580a5ab6b482a56396e3d38948c71970c8fea2a9987021f0759447752d`; ejecutable activo `d4dfe43e58381c1a1980503eb7b553de38cdc135f4a6a3baeb739c41485b91ac`.
- Respaldo: `backups/bastion-backup-2026-11-20-105759.zip`, SHA-256 `84278edb5d13fcf5b558d7f2264db1b5dd329521aa53c52a419e0c22165b9871`. Manifiesto del backend anterior `4d6e7a00c031acf4d8fe3a615fd78313316d4375`; se restauró con esa versión y después se comprobó el backend nuevo sobre la copia. Los respaldos automáticos siguientes registran la versión nueva.
- Versión anterior conservada en `/opt/bastion/releases/4d6e7a00c031`; la publicación fue sólo de código y verificó readiness y rutas privadas, con reversión automática preparada.
- Comprobante del despliegue: `/etc/bastion/release.json` y `/var/lib/bastion-deploy/production-close-71d210c44926/deployment.json`.
- Comprobantes privados: `/var/lib/bastion-deploy/production-close-20261120/receipts/`. `result.json` SHA-256 `6a8327e9ab212945a5ae2588a8f68b0fb1f233fc090f8b1a0a1a98c278af7713`; `functional.json` SHA-256 `e3bb6a5d86e79399d9ee1ca8d55d707a483b8fa4c1adf927d3b77d61152fffa6`.

No se publican dumps, claves, destinatarios ni documentos privados en GitHub. Para repetir el ensayo, usar una copia real vigente, exigir aislamiento antes de ejecutar el motor, verificar todos los destinos y repetir la comparación de datos, archivos, revocaciones y arranque; nunca ejecutar la recuperación de comprobación contra el MySQL o S3 operativos.
