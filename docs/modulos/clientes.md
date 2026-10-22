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

El usuario confirmó el 2026-10-22 publicar esta primera versión con una sección en Ajustes que indica que los ajustes propios siguen pendientes, sin campos editables. Esto autoriza el corte inicial, no da por definidos futuros settings.

## API, datos y permisos

API actual: listar/buscar/paginar clientes, catálogo para selectores, crear y editar. Permisos nuevos deben cubrir consultar/crear/editar. Evitar que normalización cambie identidad tributaria sin regla explícita.

## Pendientes

El formulario activo y el controlador se cotejaron estáticamente el 2026-10-22. Límites comprobados: nombre/contacto 255, correo 50, RTN 30, teléfono/móvil 20 y dirección 1500 caracteres. El nombre es obligatorio. Alta completa coincidencias por nombre y luego RTN; editar reemplaza campos sin combinar registros. Ver [implementación y límites](clientes-implementacion.md) y [OpenAPI](clientes.openapi.yaml).

Pendientes: sesión visual Legacy, esquema SQL y eventual migración de clientes, equivalencia de transliteración fuera de nombres latinos, settings propios y política de borrado/archivo. No se ofrece eliminación ni MCP hasta completar identidad delegada del Core.
