# Despliegue en Bluehost Choice Plus

## Objetivo
Aplicación Laravel, frontend Angular compilado, MySQL y correo en Bluehost.

## Validación previa
Confirmar versión de PHP, extensiones, Composer/SSH, MySQL, cron, frecuencia mínima, límites de procesos, memoria, ejecución, disco y envío SMTP. Confirmar que el document root puede apuntar únicamente a public/ de Laravel.
No se asumen Redis, Docker, servicios persistentes, supervisor ni workers permanentes.

## Estrategia
Compilar Angular fuera del hosting. Ejecutar jobs cortos mediante cron y cola de base de datos si los límites lo permiten, con lock y tiempo máximo.
OCR, crawling con navegador, inferencia y generación pesada requieren un runtime compatible; ubicación por decidir.

## Publicación futura
Configurar HTTPS, secretos, permisos, backups y modo producción sin debug. Aplicar migraciones con respaldo y plan de rollback. Comprobar login/MFA, autorización, cron, SMTP y restauración antes de uso real.
Este repositorio aún no contiene scripts de despliegue.
