# Despliegue Lightsail

Fuente: [Documento maestro vigente](../../docs/Proyecto_Bastion_Documento_Maestro.md). Alcance conceptual; implementación y validación técnica pendientes.

# 38. Despliegue

## 38.1 Lightsail

Compilar Go reproduciblemente fuera del servidor; desplegar binario, configuración privada y servicios systemd por SSH. Crawler conserva Python, venv, requirements.lock y timers. Workers/scheduler también en Lightsail. Validar salud, API/MCP, conexión cifrada MySQL, consumo y rollback antes de activar.

## 38.2 Bluehost

Desplegar Angular compilado por SSH/SFTP al document root reportado. MySQL y correo permanecen aquí. Esquema/migraciones los administra el backend con permisos controlados; no desplegar Go ni workers en Bluehost. Validar backup y rollback de migraciones por separado.

---


