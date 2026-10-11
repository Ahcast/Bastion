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

Frontend Angular/MySQL/correo permanecen en Bluehost. Core/API/MCP/crawler/workers en Lightsail; archivos S3. No se incluye MySQL de producción en AWS. [Crawler Python existente](crawler/README.md) se conserva sin cambios. Clientes y Proveedores implementados; catálogo de fuente y receptor de Procesos implementados. Demás módulos comerciales pendientes según el plan.

Proveedores: internal/modules/suppliers, migración 0003_suppliers.sql y composición independiente de Clientes. [Ejecutar y validar](../docs/development/suppliers-runbook.md).

Cotizaciones cerrado para alcance HNL/USD separado: cálculo, PDF/adjuntos, aprobación/emisión, correo, estados/bulk y FX auxiliar. [Runbook](../docs/development/quotes-runbook.md). OC de clientes es el módulo activo: [runbook](../docs/development/purchase-orders-runbook.md). Conversiones comerciales se completan mediante sus propietarios en sus unidades.

Facturas Borradores: BASTION_INVOICES_ENABLED=true|false (true predeterminado), permisos explícitos facturas.view/create/update, sin concesiones automáticas; borrador/cálculo y configuración fiscal certificados; emisión transaccional en validación, sin pagos/PDF todavía. Migración 0021 de desarrollo y pruebas; no activar series/pagos/emisión por habilitar el módulo.

Configuración fiscal de Facturas: migración 0022, GET/PUT /invoices/fiscal-settings, permisos fiscal.read/manage globales y reauth para writes. Sin grants/configuración precargada. Primer contador invoice se crea atómicamente; editar configuración no modifica el correlativo. CI 38180883877 certificado.

Emisión en validación: 0023, facturas.emit por registro, humano/reauth/CSRF/versiones e idempotencia. GET /invoices/{id}/emission-context revisa datos guardados; POST /invoices/{id}/emit congela snapshots y asigna correlativo atómicamente. No producción certificada ni pagos/PDF en este bloque.
