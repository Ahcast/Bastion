# Proveedores: primer corte publicado

Fecha: 2026-10-24. Estado: publicado en GitHub, AWS y Bluehost desde `2ef93c5`. El módulo está disponible en el ambiente y desactivado; la paridad completa con Legacy sigue pendiente.

## Alcance

Implementa el módulo `proveedores` con tabla y ledger de migración propios, registro progresivo en Core, API HTTP y pantalla React cargada bajo demanda. La fuente de paridad sigue siendo `proveedores-y-destinos.md` y el inventario de `01-aplicacion-actual-y-paridad.md`.

Incluye consulta/listado paginado, búsqueda, endpoint de catálogo para selectores, consulta individual, alta y edición. La tabla web muestra proveedor, contacto, correo, móvil/teléfono y acción Editar. El formulario contiene los siete campos confirmados. No incorpora borrado, reglas de duplicados, settings editables, selector dentro de Órdenes de compra, MCP, ni migración de datos Legacy.

## Decisiones técnicas explícitas

Las decisiones de esta sección son parte del contrato nuevo necesario para este corte. No se presentan como reglas observadas del Legacy y deben revisarse al cotejar una sesión autenticada:

- Permisos: `proveedores/catalogo/ver` y `proveedores/catalogo/editar`. El Administrador del sistema conserva el acceso transversal de Core. `editar` cubre altas y actualizaciones; no se agrega `eliminar`.
- Persistencia: `proveedores_records`, `proveedores_create_receipts` y `proveedores_actor_limits`, con ledger `_proveedores_migrations`. Los IDs empiezan en la secuencia nueva y no coinciden por contrato con Legacy.
- Límites técnicos de texto: proveedor 255; contacto 255; correo 254; RTN 100; teléfono 60; móvil 60; dirección 1500. Los campos se recortan en los extremos y rechazan caracteres de control salvo tabulador/salto de línea. Solo `supplier_name` es requerido. No se interpreta ni valida sintaxis de email, teléfonos o RTN.
- No se deduplica, fusiona ni impone unicidad a proveedores. La edición reemplaza los campos recibidos; los campos opcionales pueden quedar vacíos.
- Búsqueda literal en nombre, contacto, correo, RTN, teléfono, móvil y dirección; orden ascendente por nombre e ID. La cobertura y el orden observados en Legacy siguen pendientes de verificación.
- `GET /api/v1/proveedores/catalogo` devuelve únicamente ID y nombre para futuros selectores. Las operaciones web y de API comparten identidad, permisos, estado del módulo, límite por actor y bitácora.
- La creación requiere `Idempotency-Key` (16..128 caracteres ASCII alfanuméricos, guion o guion bajo); una repetición del mismo actor y cuerpo devuelve el resultado original y reutilizar la clave con otro cuerpo da conflicto. La actualización requiere `If-Match`; una versión obsoleta devuelve 412 y no sobrescribe cambios posteriores.
- Límite de cuerpo: 16 KiB; paginación: páginas 1..10000, tamaños 1..100, 25 por defecto; búsqueda: hasta 200 caracteres; cuota: 120 solicitudes por actor/minuto; timeout: 30 segundos.

## Frontend

La página aplica los lineamientos recientes de Cotizaciones mediante tokens y tabla del Core: listado con borde superior oscuro, búsqueda compacta, tipografía y espacios densos, y presentación de filas con etiquetas en pantallas estrechas. El formulario fluye en tres columnas, dos columnas y una columna según el ancho. Tras guardar, vuelve vacío para permitir otra alta; este detalle de interacción queda pendiente de revisión Legacy. No se agrega botón cancelar/limpiar. La navegación y Ajustes muestran Proveedores solo con permiso `ver`; las acciones de edición requieren `editar`. Se protegen cambios sin guardar y se conservan en pantalla ante conflicto de versión.

Se muestra sección propia en Ajustes que declara que no hay ajustes editables en este corte; no se inventan parámetros.

## Publicación y estado

El workflow `40151489770` pasó con éxito. AWS ejecuta `2ef93c5a81590d0f1b4cb577b1731845372a4f91`, readiness responde 200 y se aplicó la migración propia 1. Se creó un respaldo privado antes de migrar; se conservaron las 53 tablas previas y la activación de los demás módulos. Las cuatro tablas nuevas quedaron vacías. `proveedores` está en `BASTION_ALLOWED_MODULES` con `included_in_release=1`, `environment_allowed=1` y `enabled=0`. Bluehost publica 40 archivos verificados, preservó `config.js` y `.htaccess`, mantuvo los assets anteriores y creó un respaldo privado. No se importaron datos Legacy.

## Pendientes de cierre

- Revisar formulario y tabla con sesión Legacy autenticada para confirmar longitudes, validación de correo y el orden/cobertura de búsqueda.
- Confirmar reglas de duplicados y permisos de mantenimiento antes de considerarlos cerrados.
- Validar visualmente en escritorio, tableta y móvil; verificar el API contra MySQL 5.7 en ambiente aislado.
- Decidir la activación administrativa tras validar la ficha Legacy y configurar los permisos requeridos.

La publicación y migración no demuestran equivalencia funcional completa con Legacy. El módulo sigue desactivado hasta una decisión administrativa.
