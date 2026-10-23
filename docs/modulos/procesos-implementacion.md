# Procesos: contrato de incorporación de bastion-scout

Estado: desarrollo del primer módulo; seguimiento en [GitHub #1](https://github.com/Ahcast/Bastion/issues/1). Publicación primero en GitHub/main y validación de CI antes de instalar en servidores.

Alcance confirmado del primer corte: importar el maestro analizado, consultar/filtrar, marcar, anotar y exportar. El usuario confirmó **conservar todos los registros y dejar la depuración pendiente**. No se elimina por ausencia, vencimiento ni antigüedad. La sección de Ajustes presenta esta política y la fuente de datos; no ofrece campos editables en esta entrega.

## Fuente confirmada

El usuario confirmó el 21 de octubre de 2026 que la entrada es `maestro.csv` de **bastion-scout una vez analizado por IA**, no el CSV del crawler anterior. Contrato contrastado con [common.py](https://github.com/Ahcast/bastion-scout/blob/main/common.py) y [README](https://github.com/Ahcast/bastion-scout/blob/main/README.md). No ejecutar análisis IA ni extracción desde Bastion.

UTF-8 con BOM opcional, separador coma, comillas CSV y saltos de línea escapados. Encabezados exactos de Scout:

```text
run_id,proceso_id,url,entidad,expediente,fecha_recepcion,fecha_recepcion_original,descripcion,items_fuente,extraido_en,extraccion_estado,contenido_hash,primera_deteccion,ultima_deteccion,ultima_modificacion,puntaje,rubro,resumen,items,hash_analizado,analisis_estado,analizado_en,analisis_modelo,analisis_error
```

`items` es texto numerado consecutivamente, un ítem por línea; `items_fuente` conserva el JSON del extractor. Se admite también el formato anterior completo con `resumen_corto` e `items` JSON de textos. `puntaje` es entero 0–100. Solo se admiten filas con extracción `completo`, análisis `analizado` y `hash_analizado=contenido_hash`; el archivo entero se rechaza si contiene filas pendientes/incongruentes. Fechas normalizadas RFC 3339 con zona explícita. El contenido de IA se muestra como texto, nunca como HTML ni instrucciones ejecutables.

`proceso_id` identifica el registro, y `contenido_hash` identifica la versión analizada. Se conservan ambos valores de Scout; Bastion no sustituye su algoritmo. Un ID ausente del CSV conserva su histórico. Una versión anterior no sobrescribe una observación más reciente. Reimportar no duplica filas ni borra marcas/notas. La incorporación y su auditoría deben confirmarse en una transacción.

## Integración

Decisión del 22 de octubre: **Actualizar** incorpora el maestro local verificado y recarga la consulta. Se elimina la carga de CSV desde el navegador y su endpoint. Exportar CSV conserva filtros y contenido. La actualización requiere `procesos/importacion/editar`, sesión y CSRF; no admite rutas ni texto CSV en la solicitud y se audita al usuario conectado. Ver [entrega local](procesos-entrega-local.md).

El usuario confirmó el 21 de octubre de 2026 que **marcas y notas son compartidas por todos los usuarios**. Se guardan separadas del contenido de Scout; una reimportación no las reemplaza. Lectura con `consulta/ver`; cambios de marca con `marcas/editar` y de nota con `notas/editar`. Edición con versión obligatoria para detectar cambios de otro usuario (`412` exige recargar), nota hasta 10.000 caracteres y auditoría sin copiar su contenido. La exportación agrega `de_interes` (0/1) y `nota` después de las columnas fuente.

Dominio en `crates/modules/procesos`, adaptador HTTP común y pantalla React bajo demanda. Core controla acceso, permisos, activación y auditoría. Módulo inicialmente desactivado y excluido del ambiente por defecto. `BASTION_ALLOWED_MODULES=procesos` permite ofrecerlo al administrador, que decide su activación en Ajustes. Una migración identificada de Procesos crea sus tablas sin alterar registros Legacy. API y navegación bloquean el módulo desactivado incluso para administradores.

Consulta/detalle/exportación requieren `ver`; importación y marcas/notas requieren `editar`, con alcances separados. No se asignan permisos a roles nuevos. MCP sigue pendiente del transporte e identidad delegada del Core.

La [recepción local de Scout](procesos-entrega-local.md), acordada el 22 de octubre, lee `/var/lib/bastion-scout/maestro.csv` y su `main.json` al iniciar y cada 60 segundos. Requiere habilitación explícita del ambiente y del módulo. Usa una identidad interna limitada a importar, validación del manifiesto y recibos persistentes por origen/SHA-256; comparte la transacción con Actualizar. El trabajador no agrega rutas de recepción por red; Actualizar usa una operación autenticada sin aceptar rutas de archivos desde API/MCP. La exportación usa las columnas actuales `resumen` e `items` numerados; el DTO mantiene `resumen_corto` por compatibilidad.

## Límites técnicos del primer corte

Actualización desde el maestro local analizado, hasta 8 MiB de CSV y 5.000 filas por solicitud; el cuerpo HTTP es un objeto vacío sujeto al límite general de 1 MiB. Validar todo antes de escribir. Listados de 1–100 filas, filtros de texto de hasta 200 caracteres y orden estable. Exportación acotada a 5.000 filas: rechazar el exceso sin truncarlo. Importación/exportación serializadas y con tiempo máximo de 60 segundos; el exceso devuelve un error controlado y permite reintentar. Son topes de implementación, no campos heredados ni settings editables.

Entidades/rubros hasta 500 caracteres, expediente hasta 255 y resumen IA hasta 16.384; rechazar excesos sin recortar texto. Listados omiten los datos fuente completos, que se consultan en el detalle. Cuota general de 120 solicitudes/minuto por actor y dos importaciones nuevas confirmadas/minuto; reimportar el mismo archivo devuelve su resultado sin consumir otra importación. La identidad de idempotencia es `(actor, SHA-256 del CSV)` y se conserva en `procesos_imports`. La exportación de consulta neutraliza fórmulas de hoja de cálculo y no reemplaza el maestro original para reimportar.

Para MySQL remoto, se consultan metadatos por lote y se insertan/actualizan hasta 100 filas o 512 KiB de JSON fuente por sentencia. Una fila puede ocupar hasta 1 MiB de JSON fuente serializado; se rechaza el exceso sin recortar datos. Todos los lotes pertenecen a la misma transacción. La exportación consume un flujo de filas acotado en lugar de cargar todo el histórico.

## Primer corte implementado

Decisión de interfaz del 22 de octubre de 2026: detalle desplegado debajo de la fila, inspirado en `Bastion · Panel (1).html`. Muestra calificación, rubro, resumen, descripción/especificaciones disponibles e ítems, además de nota compartida y referencia original. Una segunda pulsación cierra el detalle; los controles de marca no abren ni cierran la fila. Corrección solicitada el 22 de octubre: marcar o guardar una nota aplica la revisión confirmada a esa fila, sin recargar la lista ni desmontar el detalle o perder el borrador de nota. Las respuestas antiguas no sustituyen revisiones confirmadas más recientes. Al desmarcar con Solo marcados se retira esa fila, actualiza el conteo y completa la página en segundo plano. Un conflicto 412 recupera únicamente la revisión afectada, conserva el borrador y exige una nueva acción explícita del usuario; no reintenta la escritura automáticamente. Se mantienen permisos y control de versión.

La barra compacta presenta, en una línea de escritorio, entidad, palabras clave, orden y los checks Solo marcados/Ocultar municipalidades; se adapta a pantallas pequeñas. Entidad usa nombres distintos del histórico importado excluyendo nombres municipales, sin depender de la página ni de otros filtros. Coincidencia exacta del nombre seleccionado (hasta 500 caracteres). Palabras clave (hasta 200) busca literalmente en expediente, descripción e `items_fuente`, el contenido fuente disponible para especificaciones; no se inventa una columna ni se busca sobre resumen IA. Búsqueda tras 350 ms sin escribir; otros filtros al cambiar.

La identificación municipal usa las palabras municipalidad(es), alcaldía(s), municipio(s) o municipal(es) en el nombre, sin distinguir mayúsculas ni el acento de alcaldía. No se infiere municipio por expediente ni se completa un catálogo geográfico. Nombres sin esas palabras no pueden reconocerse como municipalidades con esta fuente.

Órdenes: recepción más reciente/más antigua y puntuación mayor/menor, con desempate estable por fecha descendente (para puntuación) y `proceso_id` ascendente. Listado y exportación comparten filtros y orden. Los filtros de fecha del API se conservan, aunque ya no ocupan espacio en la barra.

Corrección confirmada el 22 de octubre: las estadísticas son globales de la importación y no cambian con filtros, orden, paginación ni vencimiento. Nuevos por semana cuenta la primera incorporación en Bastion, sumando `inserted` de recibos confirmados según `procesos_imports.created_at`, en seis semanas de lunes a domingo incluida la actual; una actualización, repetición o reimportación por otro actor no vuelve a contarlo. Esta fuente permite contar importaciones anteriores sin inventar fechas por registro ni usar primera detección en Scout. Entregas de hoy, mañana y pasado mañana usa `fecha_recepcion` de todo lo importado, incluidos vencidos de hoy y municipalidades. Calendario `America/Tegucigalpa` (UTC−06); ceros explícitos y última importación global. Son recepciones previstas, no ofertas enviadas. La proyección de primera detección de migración 0004 se conserva, pero no alimenta esta gráfica.

Solo se muestran vigentes: `fecha_recepcion > instante UTC actual del servidor`. La igualdad ya venció; no basta comparar el día. Listado, detalle, cambios de revisión y exportación aplican la misma condición. Vencimiento no borra procesos, marcas, notas ni recibos, y no reduce las estadísticas. Si una nueva importación extiende la recepción, el registro vuelve a poder consultarse con su revisión conservada. El listado devuelve `total` de vigentes filtrados y `server_time` del corte, separados del total global de estadísticas. La pantalla comprueba el primer vencimiento visible según el reloj del servidor y el tiempo de solicitud transcurrido. Al volver a una pestaña del navegador solo revalida si ya venció una fila; retira las filas vencidas y completa la consulta en segundo plano, conservando las demás filas y sus detalles abiertos. No depende de la zona horaria o reloj de pared del navegador.

`GET /status` aporta estadísticas sin filtros (acepta los anteriores por compatibilidad, ignorándolos) y `GET /entities` el catálogo autorizado del histórico (máximo 5.000 entidades, rechaza el exceso sin truncar). Ambos requieren `procesos/consulta/ver` y módulo activo. No se agregan permisos ni ajustes. La corrección usa los recibos existentes y no requiere una migración adicional.

Consulta de estado/estadísticas, listado paginado por expediente/contenido fuente, entidad, fechas de recepción, municipalidad y marca; detalle expandido de campos fuente y análisis; marcas/notas compartidas con control de versión; actualización desde Scout local y exportación ordenada. Contrato HTTP: [procesos.openapi.yaml](procesos.openapi.yaml). La pantalla carga bajo demanda y la sección Procesos en Ajustes explica su estado pendiente. La allowlist y el estado administrativo se comprueban también en cada operación del servidor. La compatibilidad de migraciones comprueba versiones y checksums de su ledger.

`migrate` aplica Core y el ledger independiente de Procesos con la credencial migradora. `serve` comprueba su migración antes de permitir activación; vacío en `BASTION_ALLOWED_MODULES` conserva Core solo. Deshabilitar conserva registros/importaciones. No ejecutar migraciones automáticamente al servir.

La validación de GitHub incluye una instancia desechable de MySQL 5.7, migraciones repetidas, idempotencia, actualización del mismo ID, archivos antiguos, permisos, límites y desactivación. Los ejemplos y pruebas son sintéticos. El despliegue y la equivalencia visual Legacy requieren validación posterior.

## Información de Scout en Ajustes

Decisión del 22 de octubre de 2026: `Ajustes > Procesos` presenta el último estado publicado por Scout en modo de solo lectura, con la misma entrada central que otros módulos. Sustituye la ficha estática antes situada dentro de Core. Requiere módulo activo y el permiso existente `procesos/consulta/ver`; no agrega permisos, settings editables, ejecución de comandos ni acceso directo a SQL/archivos para API/MCP.

`GET /api/v1/procesos/scout-status` lee exclusivamente `main.json`, `scout.json`, `combiner.json` y `clasificador.json` junto al maestro configurado en el backend. No lee `config.json`, CSV, SQLite ni logs; no ejecuta Scout ni IA, ni incorpora datos. Verifica autorización antes de leer y antes de responder. Mantiene los límites y cuota del módulo. `main.json` admite hasta 32 KiB; cada reporte de etapa hasta 512 KiB. Solo archivos regulares sin enlace simbólico final; se comprueba lectura estable y la estabilidad del manifiesto principal durante la consulta. No acepta parámetros de ruta/nombre ni URLs.

Se proyectan campos conocidos: ciclo, estado reportado, inicio, fin, duración cuando existe fin, límite horario, motivo operativo, disponibilidad de entrega y etapas pendientes; contadores de extracción/consolidación/análisis, pendientes, errores, reutilización de caché y tokens de entrada/salida cuando estén presentes. Fechas en hora de Honduras, formato de 24 horas. Ausencia de campo no se convierte en cero. La información se consulta al abrir y cada 60 segundos; Volver a consultar solo hace GET.

Reportes ausentes, sin lectura, inválidos o cambiantes se indican por separado. Una etapa de otro ciclo se identifica como tal; si no hay manifiesto principal válido no se atribuye pertenencia al ciclo actual. No se infiere la salud del proceso del sistema operativo a partir de un estado `ejecutando` persistido. Es el último estado escrito, no un monitor del servicio. `output_ready` es la entrega informada por Scout; la importación conserva su validación separada de CSV/huella.

No se devuelve JSON crudo, mensajes de excepción, listas de URLs, rutas locales ni configuración; los motivos se reducen a códigos conocidos y las incidencias a presencia/contadores. Los retiros del maestro son operaciones de Scout y no cambian la conservación del histórico de Bastion. Esta información no requiere una migración nueva.

## Pendientes que no deben resolverse por inferencia

- Políticas editables y depuración quedan aplazadas por decisión del usuario. La política vigente es conservación total; no habilitar operaciones destructivas hasta acordar criterio y protección de filas con marcas/notas.
- El maestro de Scout no contiene columnas separadas de municipio, dirección ni especificaciones. No inventarlas ni deducirlas de IA como datos fuente. Su paridad requiere definir origen/mapeo.
- Otros productores y entregas por red requieren definir su identidad y contrato; la recepción implementada se limita a Scout local. No aceptar rutas de archivos ni URLs arbitrarias en la API.
- Verificación de paridad visual con una sesión Legacy y validación de volumen real.
