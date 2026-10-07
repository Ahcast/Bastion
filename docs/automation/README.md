# Automatización

Jobs persistentes con clave de idempotencia, correlación, estado, intentos, próxima ejecución y error resumido. Estados propuestos: pending, running, succeeded, retry_wait, failed.
Lock con vencimiento, recuperación de trabajos abandonados y límites de duración. Reintentar errores transitorios; enviar errores permanentes a revisión.
Horarios del crawler: 01:00 y 19:00, zona America/Tegucigalpa, configurables. La planificación de extracción pertenece a AWS; validar el runtime elegido.
No crear procesos duplicados ni repetir comunicaciones por un retry. Jobs sensibles usan aprobación y outbox. Este directorio contiene documentación, no tareas activas.

## Reparto
AWS ejecuta extracción y procesamiento externo. Bluehost importa, coordina y aplica cambios autorizados; también envía el correo. Cada lado deduplica sus efectos y comparte identificadores de correlación.
