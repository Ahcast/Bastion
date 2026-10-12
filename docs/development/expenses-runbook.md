# Gastos — ejecutar y verificar en desarrollo

Configuración, instalación de Go/Angular y arranque: [Core](core-runbook.md). Contratos: [Gastos](../contracts/expenses.md), [OpenAPI](../contracts/expenses-openapi.json) y [aceptación integral](../contracts/expense-acceptance.md). Sólo datos y bases de prueba; producción exige autorización y sus gates externos.

## Arranque

`BASTION_EXPENSES_ENABLED=true` habilita rutas/menú (default true). `false` no borra datos y elimina la política documental del registro; reiniciar backend después de cambiar flags. El registro manual funciona aunque Proveedores, los propietarios de referencias o IA estén deshabilitados.

Con entorno Core de desarrollo configurado y una BD MySQL 5.7 dedicada:

```bash
cd backend
go run ./cmd/admin migrate
go run ./cmd/server
```

Migraciones Gastos: 0030 expenses, 0031 expense_references/expense_documents, 0032 índice documental Core. Son aditivas; no editar versiones aplicadas. No ejecutar estos comandos contra una BD productiva sin autorización.

La administración de roles debe conceder permisos explícitos de `gastos.*`: view/create/update/delete/restore/references.write/documents.write con scope all/self/team. No hay grants automáticos. El usuario debe completar MFA. Transferir equipo, eliminar/restaurar, cambiar referencias o comprobantes exige confirmación de identidad vigente.

Proveedor catalogado exige `proveedores.view` sobre un proveedor activo. El snapshot se conserva cuando la edición mantiene el mismo ID, incluso si el maestro cambia. Proveedor libre no depende del catálogo. Referencias exigen view en cada propietario y versión revisada; la factura anulada y fuentes en papelera se rechazan. Un gasto referenciado no modifica pagos, costos ni estados de fuente.

## Archivos y recuperación

Comprobantes requieren `gastos.view`, `gastos.documents.write`, `core.documents.write` y reauth. Lectura/preview/download requiere además core.documents.read según documento. Configurar Files/S3 mediante Core en entorno autorizado; sin adaptador, una carga devuelve error explícito y no se simula almacenamiento.

Carga: crear metadata/URL privada → PUT con checksum → finalize/integridad → vincular documento. Estado quarantined no permite preview/download; sólo scanner Core autorizado puede pasar a available/rejected. No marcar limpio para superar una prueba ni activar OCR en Lightsail. Un documento existente se vincula con su ID y versión vigentes. La papelera del comprobante es reversible y conserva el vínculo; la del gasto oculta todos sus archivos hasta restaurar el gasto.

Ante ACK incierto, reintentar la misma intención conserva cuerpo/clave y etapa de carga. Una operación confirmada conserva ID/versión aun si falla recarga; recuperar la ficha antes de otro write. Cambiar de ficha está bloqueado durante HTTP. No repetir una carga finalizada para recuperar un vínculo pendiente.

## Pruebas

```bash
cd backend
go vet ./...
go test -race -count=1 ./...
make build
```

`BASTION_TEST_MYSQL_DSN` debe seleccionar `bastion_test` en MySQL 5.7: las integraciones destruyen y recrean ese esquema sintético. Sin DSN, se omiten y no acreditan persistencia.

```bash
cd frontend
npm ci
npm run build
npm test -- --watch=false
npm run test:e2e
```

Navegador usa API/Angular/MySQL de prueba con el seed y browserapp indicados en `.github/workflows/core.yml`. Verificar registro/cálculo, HNL/USD separados, proveedor histórico, duplicados visibles, edición/nota, papelera de gasto, referencia, carga en cuarentena y papelera/restauración de comprobante. No usar browserseed/browserapp ni sus identidades sintéticas en producción.
