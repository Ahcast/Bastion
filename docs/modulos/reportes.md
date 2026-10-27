# Módulo: Reportes

## Propósito

Consultar agregados de órdenes, facturas y gastos por periodo y exportar reportes.

## Filtros y presentación

| Campo/control | Requerido | Función |
|---|---:|---|
| Fecha inicio (`date_from`) | Sí para consultar | Inicio del rango. |
| Fecha fin (`date_to`) | Sí para consultar | Fin del rango. |
| Tipo de reporte | Selección | Facturas, gastos u órdenes. |
| Estado | Opcional | Filtra estados aplicables. |
| Búsqueda/paginación | Opcional | Navega registros. |

Resumen de órdenes: venta, ISV, inversión de productos, otros gastos, inversión total y utilidad. Facturas: importes, pagos y saldo. Gastos: bases gravadas/exentas e ISV. Valores observados en HNL con dos decimales.

## Botones y acciones

Consultar/Aplicar (valida fechas y carga informe); pestañas Reportes de facturas, Reportes de gastos, Reportes de órdenes de compra; exportar Excel; paginación. Etiqueta exacta del botón Excel requiere revisión visual.

## Settings bajo Ajustes

No se identifican settings propios. El módulo aparecerá bajo Ajustes; preferencias de exportación o formatos son `Pendientes de decisión`.

## API, datos y permisos

API actual: resumen de órdenes, reportes de facturas/gastos y Excel. Reportes agregan dominios, por lo que deben aplicar los mismos filtros y alcance de datos autorizados por usuario. Aplicar `ver` para consultar y exportar; validar alcance por dominio y los límites de rango, filas y concurrencia.

## Pendientes

Definir zonas de fecha, inclusión de extremos, filtros avanzados, límites de exportación y reconciliar fórmulas con ejemplos reales anonimizados.

## Primer corte local de Bastion

Implementa consultas por fecha para los datos disponibles en los módulos actuales:

- Órdenes de compra: venta, ISV, inversión en productos, otros gastos, inversión total y utilidad; filtros por estado `Pendiente`, `Listo` o `Entregado`.
- Facturas: borradores, venta, pagado y saldo. En este corte no hay pagos ni números fiscales; los importes pagados reflejan el valor guardado por el dominio.
- Gastos: bases gravadas/exentas, ISV y total, con totales separados por moneda.
- Exportación Excel legible por Excel, en formato SpreadsheetML `.xls`, con hojas de resumen y detalle; límite de 1,000 filas por archivo.

La consulta requiere `reportes/{ordenes|facturas|gastos}/ver` y también permiso `ver` del dominio fuente (`ordenes_compra/documentos`, `facturas/documentos` o `gastos/registros`). El servidor filtra de nuevo; ocultar una pestaña en React no concede acceso. Las consultas están limitadas a 366 días inclusive, 30 solicitudes por usuario por minuto y dos consultas simultáneas por proceso. Los eventos `report.view` y `report.export` guardan tipo, rango y cantidad de registros, sin copiar importes o datos personales a la auditoría.

Los topes de rango, exportación y solicitudes son controles propuestos para esta implementación; no se presentan como reglas observadas en Legacy y quedan pendientes de verificación. Fechas inclusivas usan los campos DATE locales guardados por cada dominio. Todavía no se implementan filtros de gastos por proveedor/estado, pagos de facturas ni los reportes legacy de facturas emitidas. Ver el [contrato OpenAPI](reportes.openapi.yaml) y el [detalle de implementación](reportes-implementacion.md).
