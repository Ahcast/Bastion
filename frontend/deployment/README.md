# Despliegue del frontend en Bluehost

Compilar Angular fuera del hosting y publicar únicamente los activos estáticos por SSH/SFTP. Configurar fallback SPA, caché de activos versionados y CSP. Dominio `bastion.castilloit.net`; document root reportado `~/public_html/website_091cdca8`.

La URL API es configuración pública; secretos, BD, correo y permisos se controlan en backend. API propuesta: `https://api.bastion.castilloit.net`. No se instala runtime Go en Bluehost.

Mantener una versión recuperable de los activos y validar navegación, login/MFA, CORS, CSRF y errores antes de cambiar la versión publicada. No desplegar automáticamente mediante este documento.
