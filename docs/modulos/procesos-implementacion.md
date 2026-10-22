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

Consulta de estado, listado paginado por expediente/resumen/descripción, entidad, fechas de recepción y marca; detalle de campos fuente y análisis; marcas/notas compartidas con control de versión; actualización desde Scout local y exportación. Contrato HTTP: [procesos.openapi.yaml](procesos.openapi.yaml). La pantalla carga bajo demanda y la sección Procesos en Ajustes explica su estado pendiente. La allowlist y el estado administrativo se comprueban también en cada operación del servidor. La compatibilidad de migraciones comprueba versiones y checksums de su ledger.

`migrate` aplica Core y el ledger independiente de Procesos con la credencial migradora. `serve` comprueba su migración antes de permitir activación; vacío en `BASTION_ALLOWED_MODULES` conserva Core solo. Deshabilitar conserva registros/importaciones. No ejecutar migraciones automáticamente al servir.

La validación de GitHub incluye una instancia desechable de MySQL 5.7, migraciones repetidas, idempotencia, actualización del mismo ID, archivos antiguos, permisos, límites y desactivación. Los ejemplos y pruebas son sintéticos. El despliegue y la equivalencia visual Legacy requieren validación posterior.

## Pendientes que no deben resolverse por inferencia

- Políticas editables y depuración quedan aplazadas por decisión del usuario. La política vigente es conservación total; no habilitar operaciones destructivas hasta acordar criterio y protección de filas con marcas/notas.
- El maestro de Scout no contiene columnas separadas de municipio, dirección ni especificaciones. No inventarlas ni deducirlas de IA como datos fuente. Su paridad requiere definir origen/mapeo.
- Otros productores y entregas por red requieren definir su identidad y contrato; la recepción implementada se limita a Scout local. No aceptar rutas de archivos ni URLs arbitrarias en la API.
- Verificación de paridad visual con una sesión Legacy y validación de volumen real.
