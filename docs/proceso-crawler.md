# Flujo del crawler de HonduCompras

Estado: especificación del proceso; todavía no implementado.

## Componentes

- `main.py`: mantiene la programación, ejecuta `crawler.py`, valida su resultado y coordina la importación mediante `combiner.py`.
- `crawler.py`: extrae los procesos de HonduCompras y produce un CSV y un `run.json` por corrida.
- `combiner.py`: recibe un CSV concreto, lo valida y agrega o actualiza sus procesos en MySQL. Debe poder ejecutarse por separado, sin iniciar el crawler.

Toda programación y evaluación de vigencia utiliza `America/Tegucigalpa` y fechas con zona horaria.

## Programación y reintentos

La hora y frecuencia programadas quedan pendientes de definir.

1. Una extracción exitosa deja la siguiente ejecución del crawler en el próximo horario programado.
2. Una extracción fallida deja la siguiente ejecución en el primero de estos momentos:
   - Cuatro horas después de finalizar el intento fallido.
   - La siguiente ejecución programada.
3. Si ambos momentos coinciden, se ejecuta una sola vez.
4. Cada nuevo fallo vuelve a aplicar esta regla.
5. No se permiten dos corridas simultáneas del crawler.
6. El estado de programación debe persistir para recuperarse después de reiniciar `main.py`. Una ejecución pendiente que ya venció debe recuperarse una sola vez, sin repetir todos los horarios perdidos.

Ejemplo, únicamente ilustrativo: con una programación diaria a las 06:00, un fallo que termina a las 05:00 se reintenta a las 06:00; un fallo que termina a las 07:00 se reintenta a las 11:00.

## Resultado de la extracción

Cada corrida debe tener un identificador propio y guardar juntos su CSV y su `run.json`. Esto evita confundir un archivo exitoso anterior con una corrida nueva que falló.

`run.json` debe contener como mínimo:

- `run_id`: identificador de la corrida.
- `started_at` y `finished_at`: inicio y fin con zona horaria.
- `exit_code`: código de salida del crawler; `0` significa éxito y cualquier otro valor significa fallo.
- `complete`: indica si terminó toda la extracción esperada.
- `csv_file`: archivo correspondiente a esa corrida.
- `record_count`: cantidad de procesos extraídos.
- `error`: detalle del fallo, si lo hubo.

Se considera exitosa una corrida cuando el proceso termina con código `0`, el `run.json` corresponde a esa corrida e informa éxito y extracción completa, y el CSV existe y supera la validación de estructura y datos obligatorios. El código del proceso y el manifest deben coincidir.

Un `run.json` ausente, inválido o incongruente, una extracción parcial o un CSV inválido hacen que la corrida se considere fallida. Un resultado de cero procesos solo es válido si se comprobó que la consulta terminó correctamente.

El CSV se termina de escribir antes de publicar el `run.json` final. Los archivos finales se publican mediante reemplazo atómico para que el combiner no lea archivos incompletos.

## Identidad e importación en MySQL

`proceso_id` es la identidad única del proceso. `content_hash` es una huella de sus datos y sirve para detectar cambios. No se usa la combinación `(proceso_id, content_hash)` como clave única del registro vigente, porque eso duplicaría los procesos al cambiar su contenido.

El hash se calcula con SHA-256 sobre una representación estable de los campos de negocio: claves ordenadas, texto normalizado, fechas normalizadas y tratamiento consistente de valores vacíos. Se excluyen valores que cambian por corrida, como la hora de extracción o el número de fila.

| Caso | Acción |
| --- | --- |
| ID nuevo | Insertar el proceso y su hash. |
| ID existente y hash igual | Conservar los datos; actualizar la última observación. |
| ID existente y hash diferente | Actualizar el mismo registro y recalcular su vigencia. |
| ID ausente del CSV | Conservarlo y evaluar su vigencia por fecha. |
| Proceso vencido | Excluirlo de los archivos de vigentes y conservar su histórico en MySQL. |

La restricción única en MySQL debe impedir duplicados por `proceso_id`. Importar nuevamente el mismo CSV debe producir el mismo resultado de negocio, sin duplicar registros. Una importación debe confirmar todos sus cambios o revertirlos si falla.

## Ausencias y vencimientos

Que un proceso no aparezca en una corrida no implica que deba eliminarse ni desactivarse. Esto también aplica a corridas exitosas.

La fecha y hora límite de recepción de ofertas determina su vigencia. Cuando se alcanza ese límite en hora de Honduras, el proceso queda vencido y deja de aparecer en los archivos de vigentes. El registro de MySQL y los CSV originales de las corridas se conservan como histórico.

La depuración por vencimiento debe ejecutarse de manera independiente del éxito del crawler y de los reintentos. No debe esperar a que termine una extracción exitosa. Los archivos de vigentes deben regenerarse al importar cambios, al arrancar el servicio y cuando se alcance el siguiente vencimiento conocido.

Una actualización de la fecha límite debe recalcular la vigencia, incluso si vuelve vigente un proceso que antes había vencido.

## Fallos del combiner

El resultado del crawler y el del combiner son independientes. Una extracción exitosa seguida de un fallo de MySQL mantiene el crawler en su horario programado.

El CSV pendiente se conserva y el combiner vuelve a intentar importarlo sin ejecutar el crawler. Su resultado se registra por separado y no modifica el código de salida del crawler en `run.json`.

Los pendientes se procesan en orden de extracción; un CSV antiguo no debe sobrescribir datos de una corrida más reciente. Los archivos de vigentes solo se publican tras una importación confirmada y una generación completa del archivo nuevo. Si falla la publicación, debe poder repetirse usando los datos ya confirmados en MySQL.

## Decisiones propuestas para casos no definidos

- Importar automáticamente solo CSV de corridas completas y exitosas; conservar los parciales para diagnóstico.
- Si no se conoce la fecha límite o no se puede interpretar con seguridad, conservar el proceso y marcarlo para revisión, sin inventar un vencimiento.
- Si la fuente muestra únicamente una fecha sin hora, precisar la regla antes de implementar su vencimiento automático.
- Reintentar las importaciones pendientes cada cuatro horas y al recuperar el servicio, sin alterar la programación del crawler.

Estas decisiones son propuestas de implementación y pueden ajustarse al cerrar los detalles del proceso.
