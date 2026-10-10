# Bastion

Aplicación operativa de Castillo IT. Core Go/Angular, Clientes, Proveedores y Procesos/receptor crawler tienen sus unidades base implementadas. Cotizaciones es el módulo activo: borradores y cálculo exacto, PDF/adjuntos, estados/revisiones, aprobación humana, emisión segura, correo comercial durable y bulk trazable están implementados y probados. La siguiente unidad es FX fechado, bloqueada hasta cerrar su política comercial; no iniciar Órdenes de Compra todavía. Los demás módulos comerciales e integraciones productivas se desarrollan por fases.

**Entrada del agente:** [AGENTS.md](AGENTS.md) → [checkpoint operativo](docs/ESTADO_DESARROLLO.md) → [maestro](docs/Proyecto_Bastion_Documento_Maestro.md) → [plan](docs/development/plan.md).

| Ruta | Contenido | Alojamiento |
| --- | --- | --- |
| [frontend](frontend/README.md) | Angular; Core visual, módulos y componentes compartidos | Bluehost |
| [backend](backend/README.md) | Go/Core/API/MCP, crawler Python y workers | AWS Lightsail |
| [docs](docs/README.md) | Especificaciones únicas, contratos y plan de implementación | GitHub |
| backend/database | Migraciones SQL y acceso a datos | MySQL alojado en Bluehost |
| S3 | Archivos privados, evidencia y backups | AWS |

Solo `main` está permitida. Los commits conservan versiones; no crear ramas adicionales. Ver [regla Git](docs/development/git-policy.md).

No se despliega software ni se activa correo por actualizar documentación. Las capacidades externas sin verificar se registran en el [checkpoint operativo](docs/ESTADO_DESARROLLO.md) y el [estado histórico](docs/development/status.md).

Runbooks: [Core](docs/development/core-runbook.md) · [Clientes](docs/development/customers-runbook.md) · [Proveedores](docs/development/suppliers-runbook.md) · [Procesos y receptor crawler](docs/development/processes-runbook.md) · [Cotizaciones](docs/development/quotes-runbook.md).
