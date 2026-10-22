# Módulo: Procesos

## Propósito

La entrada confirmada el 21 de octubre de 2026 es el `maestro.csv` de **bastion-scout después del análisis IA**. El contrato y los pendientes del desarrollo se describen en [procesos-implementacion.md](procesos-implementacion.md). El inventario Legacy siguiente se conserva como referencia de paridad; no define los encabezados del nuevo importador.

Importar registros desde CSV, revisar información, marcar oportunidades de interés, añadir notas y exportar o depurar históricos.

## Campos y filtros

| Campo/control | Función |
|---|---|
| Archivo CSV | Importa/actualiza procesos; formato exacto depende del archivo fuente. |
| Búsqueda | Filtra texto, expediente o resumen. |
| Entidad/municipio | Filtro de entidad/municipio. |
| Dirección | Filtro geográfico/de dirección. |
| Solo marcados | Muestra filas marcadas. |
| Ocultar municipios | Controla filas/grupos municipales. |
| Nota | Texto asociado a fila. |
| Paginación | Navega registros. |

Columnas observadas/derivadas: expediente, entidad, resumen, fecha de recepción, fecha de recepción de ofertas, dirección, especificaciones, estado De interés y nota. Confirmar encabezados exactos con CSV anonimizados.

## Botones y acciones

Actualizar (consulta estado/datos); Importar CSV (ingiere y actualiza filas); Exportar CSV (descarga); Ver todo el proceso (detalle); marcar/desmarcar (De interés); Guardar nota; anterior/siguiente; Depurar procesos antiguos (mantenimiento, acción destructiva restringida).

## Settings bajo Ajustes

No se confirman settings específicos en el módulo actual; se registra la sección central. Configuración de importación, retención, filtros por defecto y calendario son `Pendientes de decisión`.

## API y datos

API: status, rows, detalle, import, export, mark, note, purge-old. Tablas observadas `procesos_rows`, `procesos_marks`. La importación debe ser idempotente según clave de negocio que se confirme.

## Permisos

Separar consulta, importación, marca/nota, exportación y purga. Purga debe requerir privilegio específico y auditoría.

## Pendientes

CSV fuente, encabezados y normalización, clave de deduplicación, fechas, reglas de marca/notas, edad de purga y volumen/límites.
