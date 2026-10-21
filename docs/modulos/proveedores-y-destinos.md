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
