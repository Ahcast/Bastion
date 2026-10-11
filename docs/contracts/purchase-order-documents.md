# Documentos privados de OC — Unidad 4

Contrato propietario. Core.Files conserva bytes S3 privados, metadatos, versiones, checksum SHA-256, validación MIME/tamaño, cuarentena, análisis, papelera y URLs autorizadas. No copia documentos de la cotización origen.

## Identidad e integridad

Cada ítem expone `id` decimal como cadena; entrada opcional (vacío para nuevo). En edición, IDs no vacíos deben pertenecer a esa OC, sin duplicados. Reordenar conserva filas/IDs y snapshots. Omitir un ID elimina la fila anterior; si tiene cualquier vínculo documental (también en papelera), conflicto y rollback completo. No reutilizar una posición como identidad. FK compuesto protege pertenencia OC/ítem; vínculos no se borran al eliminar metadata.

Migración 0018 agrega permiso `ordenes.documents.write` sin otorgamiento automático y tabla propietaria `customer_purchase_order_documents`: documento único, OC, ítem nullable, actor/fecha. `documents.entity_type='purchase_orders'`, entity_id OC. Core es único escritor de su vínculo.

## REST y DTO

`GET /api/v1/purchase-orders/{id}/documents?page=1&per_page=25&deleted=false&item_id=` devuelve data/total/page/per_page. Sin item_id incluye cabecera e ítems; cada fila expone item_id nullable, posición actual 1-based nullable y metadata Core. Nunca object_key ni bytes. deleted acepta sólo true/false, límites iguales al listado OC.

`POST /api/v1/purchase-orders/{id}/documents`, Idempotency-Key obligatorio: `{document_id:string,document_version:string,version:string,item_id:string|null}`. Todos campos presentes, IDs positivos, versiones positivas. Documento no generado, no eliminado, sin propietario comercial previo; pending/quarantined/rejected pueden vincularse pero nunca descargarse hasta available. Devuelve id/version OC. Versión OC y documento, vínculo, auditoría y hecho `purchase_order.document_linked` atómicos. Replay reautoriza y conserva resultado; otra intención/version vieja conflicto.

## Autorización y estados

Leer requiere `ordenes.view` por registro y `core.documents.read` por documento; listado filtra scopes Core. Escribir requiere `ordenes.view`, `ordenes.documents.write` por OC y `core.documents.write` por documento. Scopes all/self/team, sólo humanos MFA según Core. Política propietaria registrada sólo si módulo habilitado; denegación por defecto. Adjuntar/versionar/papelera permitido en estados operativos received..delivered, prohibido invoiced/closed. Independiente de permiso para editar finanzas.

Carga/finalize/scan/preview/download/delete/restore usan rutas Core existentes, misma política propietaria. Papelera conserva vínculo; restaurar no cambia destino. Descarga sólo available, URL firmada breve, auditoría Core. Sin escáner productivo no se elimina cuarentena.

## Errores y aceptación

422 DTO/IDs/paginación inválidos; 404 registro fuera de scope o ítem de otra OC; 403 permiso/estado protegido; 409 versiones, documentos ya vinculados, ID duplicado o eliminación de ítem vinculado. TX rollback ante fallos de vínculo/auditoría/evento. Probar reordenamiento, edición, rechazo de ID ajeno, borrado con papelera, replay/concurrencia/revocación, cuarentena/scan/descarga y UI de cabecera/ítems con intención conservada tras fallo.
