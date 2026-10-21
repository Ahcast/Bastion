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
