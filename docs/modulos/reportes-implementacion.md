# Reportes · implementación local

## Incluido

- Módulo `reportes` en el catálogo de release, configurable por ambiente y activable desde Ajustes del Core.
- Pantalla con pestañas para órdenes de compra, facturas y gastos, rango de fechas, estado de órdenes, resumen, listado paginado y exportación Excel.
- Búsqueda por cliente/número de orden, cliente/número de orden de origen, o proveedor/número de factura, según el dominio.
- El Core autoriza `reportes/{dominio}/ver`; cada dominio vuelve a autorizar su lectura antes de consultar su propia tabla.
- Los módulos fuente calculan agregados dentro de su dominio. Gastos agrupa los totales por moneda.
- Eventos de auditoría para consultar/exportar, cuota de 30 solicitudes por actor por minuto y máximo de dos reportes simultáneos por proceso.
- Exportación como SpreadsheetML 2003 (`.xls`), con resumen y detalle, máximo de 1,000 registros por archivo.

## Decisiones de este corte, pendientes de validar

- Rango máximo de 366 días calendario, con ambas fechas incluidas.
- Exportación limitada a 1,000 registros. Si existen más, el usuario debe reducir el rango.
- Cuota de 30 consultas/exportaciones por actor por minuto.
- La lectura de un reporte requiere tanto permiso de Reportes como permiso `ver` del módulo fuente. Esto conserva el límite de acceso que usa cada dominio.
- El estado solo filtra órdenes. La versión de facturas guarda borradores y no tiene estados adicionales; gastos aún no tiene estado.

Estos valores son controles de implementación elegidos para consultas acotadas. La ficha de paridad de CastilloIT no documenta esos topes, por lo que deben cotejarse antes de considerarlos paridad cerrada.

## Pendiente

- Confirmar rango, límites y período de fechas con una sesión Legacy.
- Reconciliar los agregados con registros anonimizados de Legacy.
- Verificar si la exportación original debe coincidir en nombres de hoja/columnas o extensión.
- Definir reportes de facturas emitidas y pagos cuando el módulo Facturas implemente esos flujos.
- MCP, filtros adicionales y ajustes específicos del módulo.

El corte es local y no está publicado ni activado en ambiente.
