# Backend Go en AWS Lightsail

Leer [AGENTS](../AGENTS.md), [estructura](../docs/architecture/README.md), [Core](../docs/architecture/core.md) y [módulos](../docs/modules/README.md). API/MCP son adaptadores del mismo monolito. Backend Go aún no implementado.

Paquetes futuros backend/internal/core y backend/internal/modules; composición backend/cmd/server y worker; migraciones backend/database/migrations. Paquetes Go minúsculos, pruebas junto al paquete. Crear carpetas cuando contengan código, no README por cada paquete vacío.

[El crawler Python](crawler/README.md) ya existe y permanece aquí. MySQL remoto en Bluehost; archivos S3; SMTP/IMAP Bluehost. Workers/scheduler en Lightsail. Sin OCR ni modelos locales. [Plan](../docs/development/plan.md) define orden y aceptación.
