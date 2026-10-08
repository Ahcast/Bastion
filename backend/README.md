# Backend Go — AWS Lightsail

Monolito modular Go con Core en `internal/core` y módulos en `internal/modules`. API REST y MCP llaman los mismos servicios de aplicación, validación y autorización. El crawler conserva Python en `crawler`.

La BD permanece en Bluehost (Percona/MySQL 5.7.44-48 reportado); `database` contiene su esquema y contratos, no un servidor local. Archivos en S3; correo por SMTP Bluehost.

Workers y scheduler se ejecutan en Lightsail con límites, idempotencia y outbox durable. No OCR ni modelos locales. Objetivo inicial: 2 GB RAM, 2 vCPU, 60 GB SSD; requiere medir crawler y workers juntos.

API/MCP en `api.bastion.castilloit.net` propuesto. Sesiones opacas, TOTP, RBAC y CSRF; CORS con origen exacto del frontend. OAuth delegado y librerías Go pendientes de selección y prueba, sin implementar criptografía o protocolo OAuth desde cero.

Versión Go, dependencias y lockfiles se fijarán al implementar. Ver [Core](../docs/architecture/core.md) y [estructura](../docs/architecture/core-modules.md).
