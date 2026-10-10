# Bastion

Aplicación operativa de Castillo IT. Core Go/Angular y catálogos de Clientes y Proveedores y catálogo/ingestión de Procesos implementados; Cotizaciones en desarrollo con borradores, cálculo, PDF/adjuntos y estados preliminares/revisiones y aprobación humana vinculada, con crawler Python existente. Los demás módulos comerciales e integraciones productivas se desarrollan por fases. [Ejecutar Core](docs/development/core-runbook.md) · [Ejecutar Clientes](docs/development/customers-runbook.md) · [Ejecutar Proveedores](docs/development/suppliers-runbook.md).

**Entrada del agente:** [AGENTS.md](AGENTS.md) → [maestro](docs/Proyecto_Bastion_Documento_Maestro.md) → [plan](docs/development/plan.md).

| Ruta | Contenido | Alojamiento |
| --- | --- | --- |
| [frontend](frontend/README.md) | Angular; Core visual, módulos y componentes compartidos | Bluehost |
| [backend](backend/README.md) | Go/Core/API/MCP, crawler Python y workers | AWS Lightsail |
| [docs](docs/README.md) | Especificaciones únicas, contratos y plan de implementación | GitHub |
| backend/database | Migraciones SQL y acceso a datos | MySQL alojado en Bluehost |
| S3 | Archivos privados, evidencia y backups | AWS |

Solo main está permitida. Los commits conservan versiones; no crear ramas adicionales. Ver [regla Git](docs/development/git-policy.md).

No se despliega software ni se activa correo por actualizar documentación. Las capacidades externas sin verificar se registran en [estado](docs/development/status.md).

[Ejecutar Procesos y receptor crawler](docs/development/processes-runbook.md).

[Ejecutar borradores de Cotizaciones](docs/development/quotes-runbook.md).
