# Reportes · alcance, publicación y despliegue

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

## Validación y despliegue

La instantánea publicada pasó compilación y pruebas Rust, 118 pruebas frontend y build de producción. CI ejecutó las pruebas HTTP de Reportes contra MySQL 5.7 aislado: permisos de ambos módulos, filtros, totales, borrado lógico, exportación Excel, auditoría y límites. Las pruebas locales dependientes de MySQL no sustituyen esa ejecución de CI.

El 2026-10-26, con autorización del usuario para producción, se instalaron en AWS y Bluehost los artifacts del commit `02c1d89`, después de completar [Core checks](https://github.com/Ahcast/Bastion/actions/runs/40430689605) y Scout control checks. Se verificaron ZIP, tar, procedencia del commit y hashes de archivos antes de publicar. Se conservaron respaldos privados de MySQL, configuración y frontend, además de la versión anterior para reversión.

La migración propia 1 añadió `_reportes_migrations` y `reportes_actor_limits`: el esquema y los datos de las 61 tablas previas se cotejaron sin diferencias inmediatamente antes y después de migrar. El total pasó a 63 tablas. Se conservaron la activación de los demás módulos y los permisos existentes; no se importaron datos Legacy ni se crearon registros de negocio sintéticos.

Reportes está incluido en el release y permitido en el ambiente, con estado `1/1/0`: disponible y desactivado, pendiente de activación administrativa desde Ajustes. No se añadieron permisos a roles. Cada reporte requiere su permiso de Reportes y la lectura del dominio fuente.

El frontend verificó los 43 archivos del paquete en disco y 41 recursos públicos por HTTPS contra sus hashes. Se preservaron `config.js`, ambos `.htaccess` y los assets anteriores para cachés. La ruta SPA respondió 200, readiness HTTPS respondió 200 y los tres endpoints de Reportes sin sesión respondieron 401.

El usuario autorizó reutilizar la credencial MySQL existente sólo durante el proceso puntual de `migrate` de Reportes. No se crearon cuentas, no se cambiaron privilegios y `MYSQL_MIGRATOR_*` no se persistió en la configuración ni en el proceso del servicio. Esta excepción no cierra el pendiente general de separar las identidades de ejecución y migración.

Quedan pendientes la activación administrativa, una comprobación integral autenticada en producción y los puntos de paridad indicados arriba.
