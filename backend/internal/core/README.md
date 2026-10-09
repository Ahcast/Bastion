# Core implementado

Go: identidad/sesiones/TOTP, RBAC, usuarios/invitaciones/equipos, settings/SMTP, auditoría, idempotencia/outbox/jobs, archivos privados y adaptadores S3/SMTP, registro modular y MCP técnico. [Especificación](../../../docs/architecture/core.md), [API](../../../docs/contracts/core-openapi.json), [ejecución](../../../docs/development/core-runbook.md).

API y MCP llaman Service; Core no importa módulos. Registry compone solo módulos implementados. Configuración y credenciales externas se validan separadamente. OAuth delegado, módulos comerciales, análisis externo de archivos, fiscalidad y despliegue productivo no quedan acreditados por esta implementación.
