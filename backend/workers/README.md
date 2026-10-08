# Workers en AWS Lightsail

Go procesa cola/outbox durable en MySQL Bluehost: importación, correo, notificaciones, mantenimiento de tareas y trabajos ligeros. Scheduler y servicios systemd/cron en Lightsail; no hay workers backend en Bluehost.

Concurrencia limitada, leases con vencimiento, timeouts, checkpoints, idempotencia y reintentos con espera creciente. No mantener un worker ocupado mientras espera una respuesta RFQ. Sin OCR ni inferencia en Lightsail.

ChatGPT Work ejecuta investigación externamente y recoge pendientes por MCP; el scheduler Go no llama Plus como API ni garantiza despertar Work. Programación real y compatibilidad requieren validación.

[Backup](backup/README.md) extrae datos Bluehost mediante acceso cifrado validado y guarda en S3. Dashboard inicial con polling HTTP. Ver [Core](../../docs/architecture/core.md).
