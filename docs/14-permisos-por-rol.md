# Permisos por rol

## Contrato y compatibilidad

La API lee y escribe una lista JSON de objetos con `module_id`, `scope_key` y `capability`. MySQL persiste cada entrada en `role_permissions`; no existe un documento JSON por rol en la base. Se conservan los endpoints `GET /api/v1/admin/roles`, `GET /api/v1/admin/roles/{id}/permissions` y `PATCH /api/v1/admin/roles/{id}` con cuerpo `{ "permissions": [...] }`.

Las capacidades efectivas siguen siendo `ver`, `editar` y `eliminar`. El usuario confirmó mantener este contrato y mostrar tres columnas: **Ver**, **Crear y editar** y **Eliminar**. Crear y editar son una sola casilla que representa `editar`, que ya incluye altas y modificaciones. Eliminar tiene su propia casilla independiente. Nunca se envía una capacidad nueva `crear`.

## Interfaz

- `Ajustes > Roles y permisos`: título **Permisos por rol**, subtítulo **Controla el acceso por módulo: ver, crear y editar, o eliminar.** y selector con todos los roles, incluido el Administrador protegido.
- Una única fuente, `web/src/permissionCatalog.ts`, ordena Inicio, Clientes, Proveedores, Órdenes de Compra, Cotizaciones, Facturas, Gastos, Reportes, Procesos, Colaboración y Configuración. El registro del servidor determina cuáles pueden configurarse y agrega automáticamente módulos registrados nuevos. Los identificadores de módulos pendientes siguen la arquitectura propuesta, pero no generan permisos hasta coincidir con un módulo incluido y permitido por el servidor.
- Inicio es accesible por autenticación; no inventa un permiso de dashboard. Configuración representa la administración global, exclusiva del Administrador del sistema. No se convierte un permiso genérico del Core en autorización para administrar roles. Los demás permisos del Core, por ejemplo Personas, se muestran como adicionales.
- La matriz tiene casillas por celda, fila y columna. Los controles masivos afectan solamente módulos configurables. Marcar Crear y editar o Eliminar marca también Ver, sin marcar la otra acción. Desmarcar Ver retira Crear y editar y Eliminar. Desmarcar una acción conserva Ver y la otra acción. Marcar todo por fila concede las tres capacidades.
- Los permisos específicos aparecen como estado parcial. Conceder acceso a todo un módulo agrega `scope_key: "*"` y conserva las entradas anteriores. Los módulos pendientes o excluidos se muestran deshabilitados; un módulo incluido pero desactivado puede preparar sus permisos sin activarse.
- Se conservan creación y eliminación de roles con las mismas restricciones existentes. El Administrador integrado muestra sus accesos implícitos en modo de solo lectura.

## Conservación y guardado

- Cargar, cambiar de rol o guardar sin modificar no transforma las entradas existentes. Los permisos de eliminación del módulo completo se reflejan en su propia columna. Permisos del Core, módulos desconocidos, acciones antiguas y alcances específicos se conservan en una sección adicional legible, sin editor JSON.
- Antes de retirar alcances específicos mediante una casilla, se identifican los permisos afectados y se solicita confirmación. **Limpiar todos los permisos** también identifica los adicionales que retirará; modifica el borrador y requiere **Guardar cambios** para persistir.
- El servidor permite conservar entradas existentes exactas aunque su módulo ya no esté disponible. Esta excepción no permite crear ni ampliar entradas para módulos no incluidos o no permitidos. Los duplicados se rechazan y una solicitud inválida revierte toda la transacción. Las acciones no reconocidas conservadas como datos históricos siguen sin autorizar operaciones.
- Éxito y errores son visibles. Un error de guardado conserva el borrador. La carga cancela respuestas obsoletas al cambiar de rol. Durante el guardado se bloquean los controles de edición.
- Se advierte antes de cambiar de rol, sección, página, cerrar sesión o abandonar el documento con cambios pendientes. Una reautenticación TOTP conserva la pantalla y su borrador para repetir el guardado al terminar.

## Autorización y pruebas

La interfaz y todas las lecturas/mutaciones de roles siguen restringidas al Administrador del sistema. El servidor valida identidad, rol protegido, formato, módulos, duplicados y auditoría; las casillas no conceden autoridad por sí mismas. No se amplía la delegación administrativa.

Probar con datos aislados: guardar y volver a cargar un rol; dependencias y controles masivos; conservación de eliminación/alcances/módulos desconocidos; confirmación de limpieza y salida; cancelación de cargas y errores; rol integrado; rechazo de solicitudes directas sin administración; y anchos de 320, 390, 768 y 1440 px.
