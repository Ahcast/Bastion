# Excel autorizado de órdenes

Complementa [Reportes](reports.md); define unidad 2 antes de su implementación.

POST `/api/v1/reports/orders/export`, JSON objeto obligatorio con `date_from`, `date_to`, `currency`, `status` (cadenas; pueden estar vacías), sin query ni campos adicionales/null. Fechas/estado/moneda validados como listado. Usuario humano y sesión MFA Core; CSRF; reauth vigente; intersección por registro de `reportes.view`, `reportes.export`, `ordenes.view`. Datos/totales/controles de precisión comparten snapshot REPEATABLE READ. Permisos revalidados por nueva descarga. No acceso técnico ni enlace reutilizable. Sin persistencia, URL pública, job, efectos sobre fuentes ni clave idempotente.

200 stream `application/vnd.openxmlformats-officedocument.spreadsheetml.sheet`, `Content-Disposition: attachment; filename=ordenes.xlsx`, no-store y nosniff. 401/403/404/422/500 Core JSON sanitizados. Si más de 10000 filas: 422 `export_too_large`, pedir intervalo más estrecho; nunca truncar. Presupuesto del request Core limita el trabajo. Límite adicional de 24 MiB XML renderizado: mismo error explícito. Auditoría `reports.orders.export` transaccional antes de enviar bytes: filtros, conteo y timestamp, sin nombres ni valores comerciales; una auditoría no confirma recepción de bytes por cliente.

## OOXML acotado y determinista

ZIP XLSX ECMA-376 con `archive/zip`/`encoding/xml` de Go estándar; no nuevas dependencias runtime. Solo dos hojas, sin macros, objetos, vínculos externos ni cadenas fórmulas aportadas por usuario.

- **Órdenes**: título/fecha UTC/filtros/nota precisión; encabezados españoles en fila 6; filas desde 7; columnas ID, Número de orden, Cliente, Destino, Fecha, Moneda, Estado, Subtotal, ISV, Total, Inversión productos, Gastos adicionales, Inversión total, Utilidad. AutoFilter, panel congelado y anchuras. Estados españoles; fechas DATE como texto ISO sin conversión de zona. IDs siempre texto. Todos los importes visibles son texto exacto con dos decimales para preservar incluso DECIMAL(18,2); el libro explica este formato.
- **Totales**: por moneda/count, para cada monto columna exacta SQL y columna auxiliar de fórmula. Nunca total global HNL+USD. Sin filas: encabezados y explicación, sin valores inventados.

Columnas auxiliares ocultas de Órdenes almacenan centavos enteros solo para moneda/campo cuya `SUM(ABS(valor)*100)` calculada en SQL sea <=999999999999999; esta cota conserva 15 dígitos decimales y queda debajo de 2^53. Fórmulas controladas `ROUND(SUMIF('Órdenes'!$F$7:$F$n,"HNL",'Órdenes'!$HIDDEN$7:$HIDDEN$n)/100,2)` y equivalente USD, cache del resultado SQL. La acumulación suma enteros y no mezcla monedas. Si supera cota: columna auxiliar vacía y aviso explícito en Totales; total SQL permanece texto exacto. Ningún monto financiero se construye pasando por float64. El cálculo de SUM/ABS y criterio de seguridad es SQL, no agregación financiera en memoria.

Todo texto comercial se codifica `inlineStr` escapado XML, incluso prefijos =,+,-,@, tabs/CR; nunca se interpreta como fórmula ni hipervínculo. Solo el renderer crea fórmulas usando nombres/referencias/monedas constantes. ZIP entradas ordenadas, fechas internas fijas; misma proyección/timestamp produce mismos bytes.

## UI y aceptación

Botón Excel solo para Órdenes con permiso exportar; usa filtros del reporte efectivamente consultado (no campos editados sin consultar), exporta conjunto completo, sin paginación. Componente común de reauth. Descarga Blob mediante cliente Angular autenticado/CSRF, URL local revocada después de descarga; errores Blob JSON recuperan mensaje Core sin exponer contenido. Busy impide doble clic y cambio de consulta durante descarga. Error no descarga contenido ni muestra éxito; cancelación/salida de vista ignora respuesta tardía.

Tests de ZIP/XML y lector independiente: nombre hojas/encabezados/formatos/filtros/fecha, formulas cache/monedas/centavos y montos grandes/negativos, inyección texto, unicode, sin macros/enlaces, bytes deterministas. MySQL 5.7: permiso export independiente/scopes/reauth/revocación/CSRF, inclusión de todo conjunto filtrado, límites y auditoría con rollback. Navegador real: reauth y descarga XLSX, mismo período y totales, reload y sin URLs persistidas.

Fuentes técnicas primarias consultadas 2026-10-12: [ECMA-376](https://ecma-international.org/publications-and-standards/standards/ecma-376/), [celdas/inlineStr](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.spreadsheet.cell), [fórmulas SpreadsheetML](https://learn.microsoft.com/en-us/office/open-xml/spreadsheet/working-with-formulas), [precisión de Excel](https://support.microsoft.com/en-us/excel/xml-schema-definition-xsd-data-type-support).
