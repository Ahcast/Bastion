# Acciones de órdenes de compra

Paridad verificada contra los controladores locales de CastilloIT: clonar conserva cliente, destino, fecha, productos y gastos; agrega el sufijo -COPIA-HHMMSS (máximo 50 caracteres), reinicia productos a Pendiente y limpia factura, entrega y archivos. La copia es independiente y no hereda el origen de cotización.

Documentos conserva un archivo privado por producto, máximo 10 MiB. Ver/descargar requiere documentos.ver; cargar requiere editar; sustituir requiere también eliminar. Retirar requiere eliminar. Cada producto recibe una identidad interna estable; retirar una línea con archivo exige retirar antes su archivo. No se exponen claves S3 ni URLs públicas.

Convertir a factura conserva la instantánea comercial: cliente/RTN, número de orden, cantidades, precios de venta, costos, ISV 15% o exento y gastos. Crédito a 30 días y fecha actual de Honduras, como Legacy. Bastion crea un borrador vinculado, sin emisión ni número fiscal; por ello no escribe un número ficticio en n_bill ni marca Entregado. Requiere ver/editar en ambos módulos habilitados. Una orden tiene una sola conversión a factura; reintentos recuperan el borrador existente. La factura conserva su origen incluso después de editarse. Se rechazan precios con precisión que Facturas no puede conservar y diferencias de redondeo.

API: POST /api/v1/ordenes-compra/{id}/clone y /convert/invoice usan If-Match fuerte e Idempotency-Key (16–128 caracteres ASCII alfanuméricos, guion o guion bajo). Clonar devuelve la orden nueva; convertir devuelve target_module, target_id, order_id, order_version, reused. GET /{id}/documents devuelve items y order_version; POST /{id}/documents/{line_id} recibe filename/content_base64 con ambas cabeceras; GET /{id}/documents/{attachment_id}/content devuelve contenido base64; DELETE /{id}/documents/{attachment_id} usa If-Match. GET /document-readiness informa almacenamiento disponible. Tamaño HTTP máximo 14 MiB, cuatro operaciones de archivos simultáneas. Conflictos de versión: 412; clave reutilizada con otro contenido: 409. Autenticación, CSRF, límites y auditoría del core siguen vigentes.

Los objetos se cargan fuera de la transacción, con cola de limpieza durable previa. Cargas fallidas, reemplazos y retiros quedan en cola; el worker elimina objetos privados. La retención de versiones S3 depende de la política del bucket. El prefijo ordenes_compra/ requiere acceso del rol AWS antes de despliegue. Construir o publicar código no confirma despliegue.

Corrección autorizada el 2026-10-26: utilidad = total de venta − ISV − inversión total, en la orden y en su borrador de factura. Migraciones propias 4 corrigen el campo derivado profit de documentos y recibos existentes sin cambiar importes fuente, versiones, origen ni auditoría histórica. Los reportes y Excel consumen esa utilidad corregida.

## Publicación y despliegue verificados

El 2026-10-27 se desplegó 50efa7e, CI 40540732266 aprobado. Órdenes 4, Facturas 4 y document-flows 2 aplicadas; seis tablas nuevas vacías, utilidad de la orden existente corregida sin otros cambios de negocio. Se conservaron activaciones y permisos. Se verificaron respaldo, restauración temporal de 63 tablas, hashes del frontend, readiness 200 y protección anónima 401. La interfaz autenticada en producción no se ejercitó con datos sintéticos; las pruebas completas de acciones y migraciones pasaron en CI MySQL 5.7.

## Proveedor por producto y comentario compacto (2026-11-11)

Se integra id_supplier opcional por producto, observado en CastilloIT. supplier_name es una instantánea resuelta por el servidor mediante un puerto de Proveedores. Seleccionar o cambiar requiere catalogo/ver y Proveedores activo, además de editar la orden. Conservar la selección existente preserva la instantánea aunque el proveedor sea renombrado o archivado; clonar conserva la referencia histórica. Quitar limpia ambos campos. Órdenes antiguas sin referencia siguen siendo válidas. Los campos se guardan en los JSON propios, sin migración SQL.

Descripción, Proveedor y Comentario tienen columnas separadas. Ambos textos empiezan con una línea de alto y pueden ampliarse; se conservan textos y límites anteriores. El selector ofrece búsqueda paginada del catálogo, sin crear proveedores implícitos.

Despliegue verificado el 2026-11-11 desde e7b60e3 (CI 43156439314): selector del catálogo existente de Proveedores y comentario compacto publicados en AWS/Bluehost. Sin migraciones ni altas implícitas; respaldo de 106 tablas, 53 archivos frontend y 51 recursos HTTPS verificados. Véase docs/09-despliegue.md.

## Búsqueda por productos

El buscador de Órdenes y el filtro equivalente de reportes también revisan las descripciones de todos los productos. Admiten coincidencias parciales, sin distinguir mayúsculas; %, _ y ! se tratan literalmente. Cada orden aparece una sola vez, con filtros, paginación y permisos existentes. No se buscan comentarios, gastos ni otros campos del JSON. No requiere migración de datos ni cambios de esquema.

Despliegue verificado el 2026-11-11 desde `654b724eae859eed887150181880ac70bda707d3` en AWS y Bluehost, con CI 43158351672 aprobado. Regresiones MySQL 5.7 verifican coincidencias parciales, caracteres literales, filtros, paginación sin duplicados y consistencia con reportes. Véase docs/09-despliegue.md.
