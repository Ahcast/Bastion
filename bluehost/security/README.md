# Seguridad de Bluehost

Aplicar [seguridad global](../../docs/security/README.md): Sanctum SPA, Fortify, TOTP, CSRF, políticas y auditoría.
Solo public/ de Laravel expuesto; secretos y archivos privados fuera del webroot.
Credenciales de integración limitadas a objetos y operaciones necesarios. Validar todo resultado de AWS.
WAF, acceso al origen, versiones y límites del hosting pendientes de validar.
