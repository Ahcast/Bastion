# Transporte de correo y programación Core

Unidad técnica previa al correo comercial de Cotizaciones. No expone nuevas rutas ni autoriza destinatarios, cambios de estado comerciales o envíos por sí misma.

## Sobre de transporte

`core.Email` conserva `Recipient` para la prueba SMTP existente. Los módulos pueden usar `To`, `CC`, `BCC` y `Attachments` en su lugar; no combinar `Recipient` y `To`. El propietario comercial debe autorizar y congelar el contenido antes de llamar al adaptador.

- `To`: al menos una dirección ASCII sin nombre de presentación; máximo 20 destinatarios entre To/CC/BCC, sin duplicados ignorando mayúsculas. BCC pertenece al sobre SMTP y no aparece en el mensaje MIME.
- `Subject`: 1–200 caracteres Unicode válidos, sin caracteres de control. `Body`: texto UTF-8 de hasta 100.000 bytes, sin controles excepto tabulación y saltos de línea. No se acepta HTML en esta unidad.
- `MessageID`: `<` + ID técnico hexadecimal de 32 caracteres + `@bastion.castilloit.net>`. El propietario lo conserva para reintentos y conciliación; no se regenera al serializar.
- Adjuntos: hasta 10; nombres únicos sin rutas ni controles, tipos PDF/PNG/JPEG/TXT de Core y bytes no vacíos, máximo conjunto 10 MiB sin codificar. Esta es una cota de aplicación, no una validación de los límites de Bluehost. Los bytes se suministran desde evidencia privada verificada; el adaptador no busca archivos ni URLs.
- Fallos de validación producen `validation` antes de conexión SMTP. El adaptador usa la configuración TLS existente y la dependencia fijada go-mail v0.8.1. Una respuesta del transporte no acredita lectura o entrega al buzón.

## Cola programada

Migración `0011_outbox_schedule.sql`: agrega `outbox_events.available_at DATETIME(6) NULL`. Los eventos previos mantienen su ejecución inmediata; su fecha efectiva es `created_at`. No modifica migraciones anteriores.

`EnqueueAt(ctx, tx, actor, kind, permission, payload, availableAt)` aplica la misma autorización que `Enqueue`, convierte la fecha a UTC y trunca a microsegundos. Rechaza fechas fuera del rango MySQL. La fecha viaja en la misma transacción que el evento y pasa intacta a `jobs.available_at`, aun si el publicador se ejecuta antes o después del vencimiento. `Enqueue` conserva comportamiento inmediato.

Estados, permisos y reintentos actuales de jobs no cambian. `pending` futuro no se reclama antes de su vencimiento; un trabajo ya publicado no vuelve a programarse al repetir publicación. No crea cron público ni servicios externos.

## Aceptación

Pruebas MIME verifican destinatarios del sobre, ausencia de BCC en cabeceras, asunto/cuerpo Unicode, Message-ID estable y bytes/tipos/nombres de varios adjuntos. Validación debe bloquear inyección, rutas, duplicados y límites. Pruebas MySQL 5.7 verifican publicación concurrente única, programación UTC, vencimiento, publicación tardía, rollback y permisos. La regresión SMTP conserva `uncertain` sin reenvío automático. Cotizaciones.send, aprobación vinculada a contenido/destinatarios, historial comercial y formulario siguen en la unidad propietaria siguiente.
