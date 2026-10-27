# Archivos legales y técnicos de Licitaciones

Decisión del usuario del 2026-10-27: Documentación legal y Especificaciones técnicas deben permitir seleccionar PDF/imágenes de Colaboración o subirlos desde el PC para incorporarlos al expediente. Cada archivo es una copia independiente, privada y visible sólo dentro de su expediente y bloque. No se introducen lotes/ítems ni campos adicionales por inferencia. El bloque económico conserva sus formularios, sin carga de archivos en esta entrega.

Se aceptan PDF y las imágenes rasterizadas reconocidas por la política existente: PNG, JPEG, GIF, WebP, BMP y TIFF. Se valida extensión y contenido detectado en servidor; SVG/HTML y ejecutables no se admiten. El contenido de cada archivo permanece intacto. El transporte de entrada es JSON/base64, acotado a 27.000.000 bytes; el máximo del archivo real es 20.000.000 bytes y el presupuesto conjunto del expediente es 500.000.000 bytes. Se contabilizan archivos disponibles y reservas/objetos pendientes de limpieza, sin eludir el tope mediante errores de subida. Se limita la metadata a 1.000 archivos por expediente, dos solicitudes de archivo simultáneas por proceso y 180 segundos por operación; estos son presupuestos técnicos, no un límite de páginas PDF.

## API y permisos

- `GET /api/v1/licitaciones/{id}/archivos`: lista metadatos, versión del expediente y bytes retenidos; requiere `ver`.
- `POST /api/v1/licitaciones/{id}/archivos`: recibe `section` (`legal` o `tecnica`), `filename` y `content_base64`; requiere `editar`, Borrador, `If-Match` fuerte e `Idempotency-Key`.
- `POST /api/v1/licitaciones/{id}/archivos/desde-colaboracion`: recibe `section` y `source_id`, con los mismos requisitos y lectura sobre el archivo exacto de Colaboración activo. El propietario entrega bytes autorizados y hash; Licitaciones no consulta sus tablas ni recibe claves S3 del cliente. Se vuelve a comprobar el origen antes de confirmar la copia.
- `GET /api/v1/licitaciones/{id}/archivos/{attachment}/preview` y `/download`: requieren `ver` y pertenencia al expediente. Comprueban hash, tamaño y autorización nuevamente después de leer el objeto privado. La vista previa sólo entrega MIME seguro; TIFF se descarga si el navegador no lo muestra.
- `DELETE /api/v1/licitaciones/{id}/archivos/{attachment}`: exige `eliminar`, Borrador, `If-Match` y `confirm_id` igual al ID del archivo. Retira el vínculo y programa eliminación física de la copia; nunca elimina el origen.

Todas las mutaciones conservan sesión, origen exacto/CSRF, límites por actor y auditoría sin contenido privado. Versión obsoleta: 412. Otra petición para la misma clave: 409. Un reintento confirmado devuelve el expediente vigente sin duplicar el archivo. Desactivar Colaboración impide nuevas copias desde biblioteca; las copias propias y las cargas desde PC dependen sólo de Licitaciones.

## Persistencia y recuperación

La migración propia `3` añade metadatos, recibos y una cola persistente de limpieza. Una reserva contabilizada y un objeto de nombre aleatorio se registran antes de escribir en S3; metadata/recibo/versión se confirman juntos después. Si falla almacenamiento, MySQL o la autorización, la reserva conserva la referencia para limpieza y libera presupuesto sólo al retirar el objeto. Las reservas no confirmadas tienen 15 minutos de margen, superior al plazo del transporte, antes de que el trabajador reclame su limpieza. Al borrar un expediente Borrador se programan también sus copias. El trabajador funciona con módulo desactivado, nunca elimina un objeto vivo y retira las versiones físicas sólo de la clave exacta conocida.

Se usa el prefijo privado `licitaciones/`, validado por el Core. Antes de habilitar la capacidad operativa se verifican permisos S3 de escritura, lectura, enumeración y retirada de versiones limitados a ese prefijo, los tamaños efectivos y las rutas autorizadas; no se amplían otros prefijos. La entrega no ensambla todavía el PDF final.
