# Catálogo de módulos

El catálogo inicial se deriva del apartado 4 de `../01-aplicacion-actual-y-paridad.md`. Las fichas no agregan módulos de negocio nuevos. Cada módulo se registra con el core y presenta su configuración bajo `Ajustes`.

| Módulo | Pantallas/áreas conocidas | Ficha |
|---|---|---|
| Clientes | Búsqueda, listado, alta y edición | `clientes.md` |
| Proveedores y destinos | Gestión de proveedores; catálogo de destinos | `proveedores-y-destinos.md` |
| Órdenes de compra | Listado, detalle/edición, documentos, entrega y conversión | `ordenes-de-compra.md` |
| Gastos | Registro, filtros, edición y eliminación; gastos vinculados a órdenes | `gastos.md` |
| Cotizaciones | Listado, editor, PDF, adjuntos y correo | `cotizaciones.md` |
| Facturas | Listado, editor, pagos, PDF, nota de entrega y recibo | `facturas.md` |
| Reportes | Órdenes, facturas y gastos; exportación | `reportes.md` |
| Procesos | Importar, revisar, marcar, anotar y exportar CSV | `procesos.md` |
| Colaboración y archivos | Archivos comunes y de personas | `colaboracion-y-archivos.md` |

## Settings

Cada ficha tiene una sección de settings que aparecerá en Ajustes. Cuando no existe configuración de módulo en el sistema actual, la ficha lo marca como `Pendiente de decisión` en vez de inventar campos. La conexión al registro, permisos y almacenamiento sigue `../04-contrato-de-modulos.md`.

## Certeza

Los campos y acciones observados se basan en frontend compilado, scripts y backend estático. La verificación visual con una sesión autenticada está pendiente. Ver `../01-aplicacion-actual-y-paridad.md` para el inventario fuente completo.

Decisión del 2026-10-23: el catálogo nuevo de destinos se administra bajo Ajustes > Órdenes de compra por confirmación del usuario y pertenece a `ordenes_compra`. La fila Proveedores y destinos conserva el inventario Legacy, no asigna la propiedad de la implementación nueva. Ver `ordenes-de-compra-implementacion.md`.

Colaboración incorpora su primer corte con límites y retención confirmados por el usuario. Ver `colaboracion-implementacion.md`.

Proveedores comienza su implementación independiente con el contrato de paridad y las decisiones nuevas pendientes de cotejo Legacy identificadas. Ver [detalle](proveedores-implementacion.md) y [OpenAPI](proveedores.openapi.yaml).

Reportes consulta los tres dominios mediante sus servicios y publica exportaciones Excel. Ver [implementación](reportes-implementacion.md) y [OpenAPI](reportes.openapi.yaml); los límites propuestos conservan su cotejo pendiente.
