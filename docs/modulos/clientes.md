# Módulo: Clientes

## Propósito

Mantener el catálogo de clientes utilizado por órdenes, cotizaciones y facturas. El backend normaliza, deduplica y combina registros coincidentes.

## Pantalla y campos

| Campo | Nombre técnico | Tipo | Requerido | Función |
|---|---|---|---:|---|
| Buscar cliente | `search` | Texto | No | Filtra la lista. |
| Cliente | `cliente_name` | Texto | Sí | Nombre del cliente. |
| Contacto | `user_contact` | Texto | No | Persona de contacto. |
| Correo | `email` | Email | No | Correo de contacto. |
| RTN | `rtn` | Texto | No | Identificación tributaria. |
| Teléfono | `phone_number` | Teléfono | No | Teléfono fijo. |
| Móvil | `mobile_number` | Teléfono | No | Teléfono móvil. |
| Dirección | `direction` | Texto multilínea/texto | No | Domicilio/dirección. |

Lista: cliente, contacto, correo, móvil/teléfono y acciones; búsqueda y paginación según componente.

## Botones y acciones

| Acción | Efecto |
|---|---|
| Buscar | Consulta clientes por texto. |
| Nuevo | Inicia formulario vacío. |
| Editar | Carga fila en formulario. |
| Guardar | Crea o actualiza; requiere nombre; limpia y recarga la lista. |
| Paginación | Cambia el grupo de resultados. |

## Settings bajo Ajustes

El informe no identifica settings propios del módulo. Mantener su sección de Ajustes dentro del espacio central; contenido y necesidad de campos: `Pendiente de decisión`.

## API, datos y permisos

API actual: listar/buscar/paginar clientes, catálogo para selectores, crear y editar. Permisos nuevos deben cubrir consultar/crear/editar. Evitar que normalización cambie identidad tributaria sin regla explícita.

## Pendientes

Confirmar todos los campos del formulario, unicidad/merge de clientes, permisos por rol y política de borrado/archivo.
