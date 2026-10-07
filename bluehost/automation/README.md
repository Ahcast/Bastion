# Automatización en Bluehost

Cron y jobs cortos para importar lotes desde S3, coordinar trabajos externos y enviar correo aprobado.
Estado de negocio y deduplicación persistidos en MySQL. Locks con vencimiento, duración máxima y reintentos limitados.
No ejecutar crawling pesado, OCR ni inferencia dentro de una petición HTTP. Validar frecuencia y límites reales de Bluehost.
Ver [reglas compartidas](../../docs/automation/README.md).
