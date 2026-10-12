# Módulo Colaboración

Colaboración conserva la biblioteca privada por empresa/persona y conecta el trabajo diario entre módulos. Es un centro operativo ligero; no intenta reemplazar Google Drive o Slack.

La especificación normativa, permisos, tablas, API y criterios de aceptación están en el [contrato de Colaboración](../contracts/collaboration.md). Core.Files conserva bytes, versiones, cuarentena, análisis y acceso firmado. Los módulos comerciales conservan propiedad de sus entidades, adjuntos y decisiones.

## Experiencia prevista

- Biblioteca: carpetas, documentos, versiones, búsqueda, metadata, preview y descarga autorizada.
- Mi trabajo: tareas asignadas a la persona, pendientes, de hoy y vencidas.
- Comentarios y menciones: conversación vinculada a recursos visibles por el actor y la persona mencionada.
- Aprobaciones: bandeja de lectura que enlaza con Workflow y las pantallas propietarias; decisiones permanecen en su módulo de origen.
- Actividad: hechos mínimos de Colaboración, filtrados por el permiso vivo del recurso.

## Límites

Permisos por acción y fila se validan en backend para cada lectura y escritura. No se exponen claves S3 ni se sirve contenido activo inline. Una referencia o mención no concede acceso al recurso. La IA/OCR puede llegar mediante fase externa, pero no puede elevar permisos ni ejecutar una acción sensible.

Las unidades se implementan en orden y se certifican antes de avanzar: biblioteca, conversación/actividad, tareas y vistas transversales; luego aceptación integral. Workflow y AI no agregan entradas de menú.
