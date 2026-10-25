# Módulo: Proveedores y destinos

## Propósito

Gestionar proveedores y ofrecer el catálogo de destinos utilizado al crear/editar órdenes.

## Proveedores: campos

| Campo | Nombre técnico | Tipo | Requerido | Función |
|---|---|---|---:|---|
| Buscar proveedor | `search` | Texto | No | Filtra la lista. |
| Proveedor | `supplier_name` | Texto | Sí | Nombre del proveedor. |
| Contacto | `user_contact` | Texto | No | Persona de contacto. |
| Correo | `email` | Email | No | Correo de contacto. |
| RTN | `rtn` | Texto | No | Identificación tributaria. |
| Teléfono | `phone_number` | Teléfono | No | Teléfono fijo. |
| Móvil | `mobile_number` | Teléfono | No | Teléfono móvil. |
| Dirección | `direction` | Texto | No | Dirección del proveedor. |

La tabla presenta proveedor, contacto, correo, móvil/teléfono y acción de editar.

## Destinos

El informe confirma un catálogo con identificador y descripción (`id_destination`, `destination`) para selectores. CRUD/pantalla de destinos no está confirmado.

## Botones y acciones

Buscar; Editar (carga la fila); Guardar proveedor (alta o edición); paginación si aplica. La etiqueta y comportamiento de cancelar/limpiar requiere revisión de pantalla. El selector de destino se usa desde el módulo de órdenes.

## Settings bajo Ajustes

No hay settings propios confirmados. El módulo contará con sección central; opciones concretas `Pendientes de decisión`.

## API y permisos

API actual: consulta/listado, catálogo para selects, alta y edición de proveedores; consulta de destinos. Permisos a definir para consulta/edición de catálogos.

## Pendientes

Confirmar gestión de destinos, campos/validaciones completos, reglas de duplicados y permisos de mantenimiento.

## Primer corte local — 2026-10-24

`proveedores` se implementa como módulo independiente con listado, búsqueda, catálogo resumido, alta y edición. La pantalla sigue los patrones visuales recientes de Cotizaciones y conserva los siete campos confirmados. No incorpora borrado ni importa IDs/datos Legacy.

Como decisiones técnicas nuevas pendientes de cotejo con Legacy, este corte usa los permisos `proveedores/catalogo/ver|editar`, búsqueda literal en los siete campos y orden por nombre/ID. Límites, idempotencia, versionado y contrato HTTP se documentan en [la ficha de implementación](proveedores-implementacion.md) y [OpenAPI](proveedores.openapi.yaml). No está desplegado ni activado.

## Decisión posterior (2026-10-23)

El usuario confirma administrar destinos bajo Ajustes > Órdenes de compra. El catálogo nuevo pertenece al dominio ordenes_compra, con contrato propio para consulta y alta/renombrado autorizados. No se incluye el módulo Proveedores en esta entrega ni se reutilizan sus futuros IDs/tablas. La gestión nueva no se atribuye al Legacy. Ver ordenes-de-compra-implementacion.md.
