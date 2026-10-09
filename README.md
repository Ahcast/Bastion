# Bastion

Aplicación operativa de Castillo IT. Este repositorio contiene especificaciones para desarrollar todo el proyecto y un crawler Python existente; backend Go y frontend Angular aún no están implementados.

**Entrada del agente:** [AGENTS.md](AGENTS.md) → [maestro](docs/Proyecto_Bastion_Documento_Maestro.md) → [plan](docs/development/plan.md).

| Ruta | Contenido | Alojamiento |
| --- | --- | --- |
| [frontend](frontend/README.md) | Angular; Core visual, módulos y componentes compartidos | Bluehost |
| [backend](backend/README.md) | Go/Core/API/MCP, crawler Python y workers | AWS Lightsail |
| [docs](docs/README.md) | Especificaciones únicas, contratos y plan de implementación | GitHub |
| backend/database (al implementar) | Migraciones SQL y acceso a datos | MySQL alojado en Bluehost |
| S3 | Archivos privados, evidencia y backups | AWS |

Solo main está permitida. Los commits conservan versiones; no crear ramas adicionales. Ver [regla Git](docs/development/git-policy.md).

No se despliega software ni se activa correo por actualizar documentación. Las capacidades externas sin verificar se registran en [estado](docs/development/status.md).
