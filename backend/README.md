# Backend Bastion — Go

Core implementado en internal/core; composición cmd/server, worker y admin; migraciones SQL en database/migrations. [Cómo ejecutar](../docs/development/core-runbook.md), [contrato OpenAPI](../docs/contracts/core-openapi.json), [estado real](../docs/development/status.md).

```bash
cd backend
go mod download
go vet ./...
go test -race ./...
make build
```

Pruebas de integración requieren MySQL 5.7 dedicado: BASTION_TEST_MYSQL_DSN debe seleccionar bastion_test. Sin esa variable, las pruebas MySQL se omiten explícitamente; no contar ese resultado como validación de persistencia. CI exige el servicio MySQL 5.7.

Frontend Angular/MySQL/correo permanecen en Bluehost. Core/API/MCP/crawler/workers en Lightsail; archivos S3. No se incluye MySQL de producción en AWS. [Crawler Python existente](crawler/README.md) se conserva sin cambios. Módulos comerciales aún pendientes según el plan.
