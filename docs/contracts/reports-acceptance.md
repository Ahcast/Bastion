# Aceptación Reportes

Alcance contractual: tres reportes canónicos, Inicio y Excel privado de órdenes. [Consultas](reports.md), [Excel](reports-export.md), [runbook](../development/reports-runbook.md). La evidencia efectiva y HEAD certificados se registran en checkpoint/status; esta matriz no acredita ejecución por sí sola.

| Criterio | Verificación |
| --- | --- |
| Reportes no concede fuentes | MySQL `TestReportsSourceIntersectionPaginationExactTotalsAndHome` y HTTP sesión/permisos |
| Scopes independientes | MySQL all/self cruzados, team/membresía, revocación y export scope adicional |
| Filas y totales completos | Paginación 1 fila con SUM sobre conjunto; export sin límites de frontend |
| Precisión y moneda | DECIMAL(18,2) grande, HNL/USD separados; texto XLSX exacto y fórmula cents acotada |
| Fechas y exclusiones | Intervalo DATE inclusivo, inválidos, OC/Gastos papelera y Facturas anuladas |
| Utilidad/inversión propietarias | Comparación del reporte OC con valores del servicio propietario; ningún join a referencias documentales |
| Snapshot coherente | Actualizaciones SQL concurrentes, totales y filas del mismo estado |
| Inicio coincidente | Misma función backend y comparación respuesta navegador para mismo período/moneda |
| UI accesible y errores | Selector etiquetado, filtros/estados/páginas, respuesta tardía y limpieza ante revocación |
| Excel autenticado | CSRF, sesión/MFA, humano/reauth, scopes export/view/fuente, revocación y flags |
| Archivo seguro | ZIP/XML/lector independiente, Unicode, texto =/+/-/@, sin links/macros, cabezales españoles |
| Límite explícito | 10001 filas y XML sobredimensionado: error, sin libro truncado ni éxito auditado |
| Auditoría | Registro generación/filtros/conteo confirmado antes de bytes; rollback ante fallo de auditoría |
| Recuperación | Error de descarga no simula éxito; nuevo request autorizado captura nuevo snapshot |
| Desactivación | Reports/fuentes flags y rutas; no jobs ni escrituras comerciales |
| Regresión | Go fmt/vet/race/MySQL5.7/builds, Angular build/pruebas, Playwright real y paridad documental |

No se prometen análisis IA/RFQ/competitividad o estados de procesos no modelados. Estas ampliaciones siguen por contratos de sus propietarios en fases siguientes. Producción, fiscalidad e infraestructura requieren su verificación independiente; las pruebas sintéticas no la sustituyen.
