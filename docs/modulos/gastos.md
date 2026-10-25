# Módulo: Gastos

## Propósito

Registrar gastos de compra/factura y gastos adicionales vinculados a órdenes, reflejando los totales e inversión en reportes.

## Registro y filtros

| Campo | Nombre técnico | Tipo | Regla observada |
|---|---|---|---|
| Buscar factura o proveedor | `search` | Texto | Opcional. |
| Fecha | `expense_date` | Fecha | Requerida. |
| Número de factura | `invoice_number` | Texto | Requerido. |
| Proveedor | `provider` | Texto | Requerido. |
| Moneda | `currency` | Selector | HNL observado. |
| Gravado | `taxable_amount` | Decimal | Debe ser >0 o monto exento >0. |
| Exento | `exempt_amount` | Decimal | Debe ser >0 o monto gravado >0. |
| Desde/hasta | `date_from`, `date_to` | Fecha | Filtros opcionales. |

La tabla muestra factura, fecha, proveedor, moneda, base gravada, base exenta, ISV 15%, total y acciones.

## Botones y acciones

Nuevo gasto limpia formulario; Guardar/Actualizar valida requisitos, calcula ISV15/total y recarga; Editar carga la fila; Eliminar solicita confirmación; Buscar/Filtrar, Limpiar filtros y paginación administran la lista. En orden, agregar/eliminar gastos actualiza la inversión.

## Settings bajo Ajustes

No hay settings específicos confirmados. Sección del módulo bajo Ajustes: sí; categorías, monedas y valores predeterminados quedan `Pendientes de decisión`.

## API, datos y permisos

API actual: CRUD de gastos, filtros por periodo/orden/usuario y almacenamiento JSON en partes del flujo. Permisos por consulta, creación, edición y eliminación. Definir el origen de verdad y consistencia de gastos ligados a órdenes/facturas.

## Pendientes

Conciliar campos de gasto general y gasto en línea; reglas fiscales/base exenta; moneda; asociación a entidad; auditoría/retención.

## Estado de implementación

El primer corte cubre el registro general independiente y conserva explícitamente los pendientes de asociación y paridad. Ver [alcance y decisiones provisionales](gastos-implementacion.md) y [contrato HTTP](gastos.openapi.yaml). No está desplegado ni activado.
