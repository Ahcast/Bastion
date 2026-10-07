# Seguridad de AWS

Requisitos de diseño: S3 privado, cifrado, TLS y permisos mínimos por componente/prefijo.
Separar crawler, lectores de Bluehost, workers y backups. Preferir identidad temporal donde el runtime la permita; nunca credenciales en Git.
Validar job_id, versión y objetos autorizados; limitar recursos, costos y salida de red. Sin acceso directo a MySQL.
Registrar ejecuciones sin secretos y auditar acceso. Configuración real pendiente de inspección.
