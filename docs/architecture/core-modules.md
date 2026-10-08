# Estructura del repositorio

Distribución aprobada el 7 de octubre de 2026; no acredita despliegue.

| Ruta | Responsabilidad | Runtime/alojamiento |
| --- | --- | --- |
| `frontend/src/app/core` | Sesión, API, shell, Inicio y Configuración | Angular estático Bluehost |
| `frontend/src/app/modules` | Módulos de presentación | Bluehost |
| `frontend/src/app/shared` | UI y utilidades reutilizables | Bluehost |
| `backend/internal/core` | Identidad, RBAC/MFA, settings/SMTP, API/MCP, auditoría, archivos y ejecución | Go Lightsail |
| `backend/internal/modules` | Módulos de negocio aprobados, AI y Workflow | Go Lightsail |
| `backend/database` | Esquema, migraciones y contratos de datos | Percona/MySQL Bluehost |
| `backend/crawler` | Extracción y estado técnico SQLite | Python Lightsail |
| `backend/workers` y `backend/scheduler` | Trabajos, outbox, retries y backups | Lightsail |
| `backend/storage/s3` | Contratos de archivos y respaldo | Amazon S3 |
| `backend/deployment` | Operación/despliegue backend | Lightsail |
| `frontend/deployment` | Despliegue estático | Bluehost |
| `docs` | Maestro y contratos transversales | GitHub |

La ubicación del código de BD no modifica su hosting: MySQL y correo permanecen en Bluehost. No hay carpetas raíz por proveedor.

Core no importa módulos concretos; la composición raíz registra módulos y sus rutas/herramientas. Módulos consumen contratos de Core. API REST y MCP llaman los mismos servicios de aplicación. Workflow conserva aprobaciones y AI tareas/resultados. No se agregan módulos ni se cambia el menú.

Frontend compone rutas desde la raíz; módulos consumen Core/shared, shared no importa negocio. Texto visible en español; nombres técnicos en inglés.

Go y librerías concretas pendientes de validación. MySQL 5.7.44-48 reportado; no asumir SQL MySQL 8. Dominio frontend bastion.castilloit.net, API/MCP api.bastion.castilloit.net propuesto. CORS, cookies host-only y CSRF explícito. Sin OCR ni inferencia en Lightsail.

Ver [Core](core.md), [contrato modular](../contracts/modules.md) y [maestro](../Proyecto_Bastion_Documento_Maestro.md).
