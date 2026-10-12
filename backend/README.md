# Backend Bastion — Go

Monolito modular: Core en internal/core, propietarios en internal/modules y composición en cmd/server/worker/admin. Migraciones SQL ordenadas en database/migrations. [Runbook Core](../docs/development/core-runbook.md), [stack](../docs/development/stack.md), [checkpoint vigente](../docs/ESTADO_DESARROLLO.md) y [plan](../docs/development/plan.md).

```bash
cd backend
go mod download
go vet ./...
go test -race -count=1 ./...
make build
```

Integraciones requieren `BASTION_TEST_MYSQL_DSN` con MySQL 5.7 y esquema dedicado `bastion_test`: destruyen/recrean ese esquema. Sin DSN se omiten y no acreditan persistencia. CI exige MySQL 5.7.44 oficial con digest fijado.

Core, Clientes, Proveedores, Procesos/receptor crawler, Cotizaciones, OC de clientes, Facturas/pagos, Gastos y Reportes canónicos/Excel están implementados/certificados para su alcance de desarrollo sintético. **Siguiente módulo: Colaboración**. Los flags BASTION_*_ENABLED de composición habilitan propietarios sin otorgar permisos automáticamente. UI española; IDs/importes JSON como cadenas; MFA/RBAC/CSRF/reauth y acciones transaccionales/versionadas.

Frontend/MySQL/correo permanecen en Bluehost. Core/API/MCP/crawler/workers en Lightsail, archivos privados S3. No MySQL productivo ni OCR en AWS. [Crawler existente](crawler/README.md) conserva su contrato/CLI; no cambiar ritmo ni ACK silenciosamente.

Runbooks: [Clientes](../docs/development/customers-runbook.md), [Proveedores](../docs/development/suppliers-runbook.md), [Procesos](../docs/development/processes-runbook.md), [Cotizaciones](../docs/development/quotes-runbook.md), [Órdenes de Compra](../docs/development/purchase-orders-runbook.md), [Gastos](../docs/development/expenses-runbook.md), [Reportes](../docs/development/reports-runbook.md). Contratos [Facturas](../docs/contracts/invoices.md) y [Gastos](../docs/contracts/expenses.md) documentan alcance y restricciones.

Las pruebas usan datos y adaptadores sintéticos. Producción, infraestructura real, fiscalidad SAR y correo/banca/Siafi/OAuth externos exigen autorización y comprobación independientes. No ejecutar browserseed/browserapp ni credenciales de prueba en producción.
