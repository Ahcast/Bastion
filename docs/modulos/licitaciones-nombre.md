# Nombre editable del expediente

Decisión del usuario del 2026-10-27, posterior al primer corte: el expediente debe tener nombre editable. Este es el único campo nuevo autorizado; no se incorporan los demás campos ni formularios propuestos inicialmente.

El nombre se muestra en el listado y en la cabecera. El ID técnico se conserva como referencia secundaria. En Resumen, un usuario con `licitaciones/expedientes/editar` puede guardar el nombre de un Borrador; los demás usuarios ven el valor sin controles de edición. Se mantienen las restricciones de estado del módulo.

`PATCH /api/v1/licitaciones/{id}` recibe exclusivamente `{ "name": "..." }`, con sesión, origen/CSRF y un único `If-Match` fuerte. El servidor recorta espacios externos, exige 1–255 caracteres y rechaza controles. La edición incrementa versión y registra `expedient.rename` sin incluir el nombre en auditoría. Guardar el mismo nombre con versión vigente no cambia la versión. Un conflicto devuelve 412 y conserva el texto sin guardar en pantalla; recargar o abandonar pide confirmar su descarte.

La migración propia `2` agrega `name VARCHAR(255) NOT NULL DEFAULT ''`. Los registros anteriores conservan ID, versión, estado y contenido. El listado indica «Sin nombre» hasta asignarlo; la cabecera conserva «Expediente #ID» como alternativa. Crear sigue aceptando `{}` y no exige nombrar el expediente de antemano. Los recibos idempotentes anteriores sin `name` siguen siendo legibles.

Contrato: [OpenAPI](licitaciones.openapi.yaml). Se verifican validación y compatibilidad de recibos, permisos, estado, concurrencia, persistencia/listado y cambios sin guardar, además de la suite existente. Desplegar exige respaldo, migración 2 y artifacts del mismo commit con CI aprobado; primero backend y después frontend. El cambio no autoriza migrar, emitir o corregir datos de Facturas. [Publicación verificada en AWS y Bluehost](licitaciones-archivos-despliegue.md).
