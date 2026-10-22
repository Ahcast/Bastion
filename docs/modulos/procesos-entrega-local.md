# Entrega local de bastion-scout a Procesos

Decisión del 22 de octubre de 2026: Scout y Bastion están en el mismo servidor. El usuario confirmó `/var/lib/bastion-scout/maestro.csv` como resultado. La recepción pertenece al backend Rust, sin HTTP entre procesos ni acceso de Scout a MySQL.

## Publicación y lectura

Scout conserva su publicación atómica actual: termina `maestro.csv` y después `main.json`. El receptor lee el manifiesto hermano `/var/lib/bastion-scout/main.json`, comprueba `status=exitoso`, `output_ready=true`, las tres etapas exitosas, `run_id`, SHA-256 y total de filas. Lee el CSV con límite de 8 MiB, verifica su huella y vuelve a leer el manifiesto para detectar cambios concurrentes. Un resultado parcial, ausente o incongruente queda pendiente sin modificar procesos.

El manifiesto no puede seleccionar otra ruta ni una URL. El backend solo abre los dos archivos configurados en el servidor; rechaza enlaces simbólicos y archivos no regulares. El usuario del servicio Rust necesita lectura/travesía sobre la carpeta de Scout, sin permiso de escritura. Los archivos originales no se renombran, borran ni marcan desde Bastion.

## Ejecución

El botón **Actualizar** solicita `POST /api/v1/procesos/refresh` con un objeto vacío. El backend lee exclusivamente el maestro configurado y su manifiesto, valida la entrega y la incorpora antes de responder con los contadores de importación. Requiere sesión, CSRF, módulo activo y `procesos/importacion/editar`; la auditoría identifica al usuario conectado. Repetir el archivo recupera su recibo sin duplicar registros. Esta actualización bajo demanda funciona aunque el trabajador periódico esté deshabilitado. No se aceptan rutas ni archivos desde el navegador. Se elimina Importar CSV y su endpoint; Exportar CSV no cambia.

El receptor se habilita explícitamente con `BASTION_SCOUT_IMPORT_ENABLED=true`, junto con la allowlist y activación normal de Procesos. `BASTION_SCOUT_CSV_PATH` permite definir el archivo local en el despliegue; su valor predeterminado es la ruta confirmada. No es un ajuste web ni un argumento API/MCP. El proceso revisa al iniciar y después cada 60 segundos, fuera del horario del crawler también, para recuperar entregas pendientes sin volver a ejecutar IA.

Se usa una identidad interna del Core `bastion-scout`, sin cuenta humana, contraseña ni token de red. El Core le concede exclusivamente `procesos/importacion/editar` cuando la integración está habilitada en el ambiente. No participa en login, API ni MCP. Cada incorporación vuelve a comprobar esta autorización y la activación del módulo. La auditoría la identifica como `bastion-scout (local)`.

La importación utiliza los mismos casos de uso, cuotas, validación y transacción que la carga manual. Su recibo e idempotencia se persisten en `procesos_imports` bajo el origen técnico y SHA-256. Si se reinicia el servidor o se pierde la respuesta después del commit, repetir el archivo recupera su recibo. Los fallos se reintentan en la siguiente revisión; solo una importación confirmada actualiza la memoria del trabajador. Las marcas/notas compartidas y todos los históricos se conservan.

El receptor incorpora la versión del maestro disponible en cada revisión. Si Scout reemplaza el archivo varias veces mientras Bastion está detenido, no hay una cola de las versiones intermedias: solo se podrá incorporar la última disponible. Los procesos ya guardados en Bastion se conservan aunque desaparezcan del maestro. Una garantía de recepción de cada ciclo requeriría acordar un archivo por entrega o una cola persistente.

## Compatibilidad del CSV

Scout actual publica `resumen` y `items` como texto numerado, un ítem por línea. El lector también conserva compatibilidad con `resumen_corto`/JSON de la entrega anterior. Ambos formatos se validan explícitamente; se mantienen los valores fuente y los ID/hash originales. Los enlaces HTTP originales de `sicc.honducompras.gob.hn` son referencias del proceso y no se descargan desde el backend.

La publicación del código y su validación se hacen primero en GitHub. Configurar permisos de carpeta, aplicar migraciones y habilitar el receptor en el servidor son pasos de despliegue posteriores.
