# Core backend

Paquetes objetivo: `backend/internal/core`. Núcleo común del monolito Go, separado de `backend/internal/modules`.

| Área | Responsabilidad |
| --- | --- |
| [Identity](Identity/README.md) | Usuarios, equipos, sesiones, autenticación y MFA. |
| [Access](Access/README.md) | RBAC, registro de permisos y evaluación común de acceso. |
| [Settings](Settings/README.md) | Empresa, parámetros generales y configuración SMTP. |
| [Api](Api/README.md) | Convenciones REST, middleware y respuestas. |
| [Mcp](Mcp/README.md) | Servidor MCP, conexiones y registro de herramientas. |
| [Audit](Audit/README.md) | Auditoría y contexto de ejecución. |
| [Messaging](Messaging/README.md) | Transporte SMTP y notificaciones. |
| [Files](Files/README.md) | Almacenamiento privado, metadatos técnicos y acceso a archivos. |
| [Execution](Execution/README.md) | Jobs, idempotencia, outbox y coordinación técnica. |
| [Contracts](Contracts/README.md) | Interfaces públicas consumidas por módulos. |
| [Modules](Modules/README.md) | Registro y dependencias de módulos; no contiene módulos de negocio. |

Core no depende de clases concretas de módulos. Workflow conserva aprobaciones y transiciones; AI conserva proveedores y tareas de IA; Collaboration conserva organización documental y comentarios.

Estas carpetas documentan responsabilidades; todavía no contienen servicios ejecutables. El contrato de integración está en [Core y módulos](../../../docs/architecture/core-modules.md).

## Lineamientos consolidados

Ver [core.md](../../../docs/architecture/core.md) para el alcance completo del Core, infraestructura confirmada, tecnologías API/MCP, seguridad, operación y decisiones pendientes de esta sesión.

