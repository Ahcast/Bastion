# Procesos: contrato de incorporación de bastion-scout

Estado: desarrollo del primer módulo; seguimiento en [GitHub #1](https://github.com/Ahcast/Bastion/issues/1). Publicación primero en GitHub/main y validación de CI antes de instalar en servidores.

## Fuente confirmada

El usuario confirmó el 21 de octubre de 2026 que la entrada es `maestro.csv` de **bastion-scout una vez analizado por IA**, no el CSV del crawler anterior. Contrato contrastado con [common.py](https://github.com/Ahcast/bastion-scout/blob/main/common.py) y [README](https://github.com/Ahcast/bastion-scout/blob/main/README.md). No ejecutar análisis IA ni extracción desde Bastion.

UTF-8 con BOM opcional, separador coma, comillas CSV y saltos de línea escapados. Encabezados exactos de Scout:

```text
run_id,proceso_id,url,entidad,expediente,fecha_recepcion,fecha_recepcion_original,descripcion,items_fuente,extraido_en,extraccion_estado,contenido_hash,primera_deteccion,ultima_deteccion,ultima_modificacion,puntaje,rubro,resumen_corto,items,hash_analizado,analisis_estado,analizado_en,analisis_modelo,analisis_error
```

`items` es JSON de textos; `items_fuente` conserva el JSON del extractor. `puntaje` es entero 0–100. Solo se admiten filas con extracción `completo`, análisis `analizado` y `hash_analizado=contenido_hash`; el archivo entero se rechaza si contiene filas pendientes/incongruentes. Fechas normalizadas RFC 3339 con zona explícita. El contenido de IA se muestra como texto, nunca como HTML ni instrucciones ejecutables.

`proceso_id` identifica el registro, y `contenido_hash` identifica la versión analizada. Se conservan ambos valores de Scout; Bastion no sustituye su algoritmo. Un ID ausente del CSV conserva su histórico. Una versión anterior no sobrescribe una observación más reciente. Reimportar no duplica filas ni borra marcas/notas. La incorporación y su auditoría deben confirmarse en una transacción.

## Integración

Dominio en `crates/modules/procesos`, adaptador HTTP común y pantalla React bajo demanda. Core controla acceso, permisos, activación y auditoría. Módulo inicialmente desactivado y excluido del ambiente por defecto. `BASTION_ALLOWED_MODULES=procesos` permite ofrecerlo al administrador, que decide su activación en Ajustes. Una migración identificada de Procesos crea sus tablas sin alterar registros Legacy. API y navegación bloquean el módulo desactivado incluso para administradores.

Consulta/detalle/exportación requieren `ver`; importación y marcas/notas requieren `editar`, con alcances separados. No se asignan permisos a roles nuevos. MCP sigue pendiente del transporte e identidad delegada del Core.

## Límites técnicos del primer corte

Importación manual de archivo analizado, hasta 8 MiB de CSV y 5.000 filas por solicitud; JSON HTTP hasta 16 MiB para permitir el escape de texto. Validar todo antes de escribir. Listados de 1–100 filas, filtros de texto de hasta 200 caracteres y orden estable. Exportación acotada a 5.000 filas: rechazar el exceso sin truncarlo. Importación/exportación serializadas y con tiempo máximo de 60 segundos; el exceso devuelve un error controlado y permite reintentar. Son topes de implementación, no campos heredados ni settings editables.

## Pendientes que no deben resolverse por inferencia

- Alcance personal o compartido de las marcas/notas y concurrencia de edición.
- Settings: no hay valores editables confirmados; mostrar la sección pendiente en Ajustes, sin afirmar que el módulo está terminado.
- Edad/fecha de purga y protección de filas con marcas/notas. No habilitar una operación destructiva hasta acordar el criterio.
- El maestro de Scout no contiene columnas separadas de municipio, dirección ni especificaciones. No inventarlas ni deducirlas de IA como datos fuente. Su paridad requiere definir origen/mapeo.
- Recepción automática, autenticación de Scout y manifiesto `main.json`: fuera del primer importador manual. No aceptar rutas de archivos ni URLs arbitrarias en la API.
- Verificación de paridad visual con una sesión Legacy y validación de volumen real.
