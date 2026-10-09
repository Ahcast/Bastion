# Ejecutar y validar Clientes

Leer [Core](core-runbook.md), [contrato](../contracts/customers.md) y [OpenAPI](../contracts/customers-openapi.json). Backend interno: internal/modules/customers; composición: internal/composition; Angular: src/app/modules/customers. MySQL permanece Bluehost; pruebas usan únicamente esquemas desechables locales/CI.

## Arranque y permisos

Aplicar 0002_customers.sql con go run ./cmd/admin migrate en el esquema de desarrollo y reiniciar servidor. No modificar migraciones aplicadas; no ejecutar contra producción sin autorización. BASTION_CUSTOMERS_ENABLED=true por defecto; false y reinicio bloquea rutas/tools/menú, conserva datos y deja archivos vinculados inaccesibles por falta de política registrada.

En Configuración, el administrador técnico crea un rol con clientes.view/create/update y scopes all/self/team; asignarlo explícitamente a un empleado invitado que haya completado MFA. El administrador inicial sigue protegido y no obtiene permisos comerciales automáticamente; tampoco puede modificar sus propios roles. No cambiar esa protección para acceder al módulo. El empleado autorizado inicia sesión y verá Clientes.

Crear ficha, agregar contactos/direcciones y guardar. RTN opcional; si se captura debe contener 14 dígitos (espacios/guiones permitidos). Nombre/RTN equivalentes causan conflicto incluso si la otra ficha está inactiva. No existe fusión ni borrado físico. Cambiar equipo requiere update all; inactivar/reactivar o vincular archivos requiere confirmar identidad. Notas append-only, historial y listas paginadas se autorizan en Go. Frente a conflicto, conservar edición local y reabrir ficha para comparar antes de guardar otra intención.

Documentos se cargan con Core en Configuración y se vinculan desde la ficha con permisos técnicos de archivo y clientes.update. Quarantined no se descarga; análisis externo y S3 reales se validan por separado. Sin integración habilitada no se afirma cargar/descargar archivos reales.

## Pruebas

Backend: gofmt, go vet ./..., go test -race -count=1 ./... con BASTION_TEST_MYSQL_DSN apuntando exclusivamente a bastion_test MySQL 5.7; make build. Frontend: npm ci, npm run build y npm test -- --watch=false. Browser: semilla tests/browserseed exclusivamente bastion_browser_test, API y Angular reales, npx playwright test. Crawler: desde backend/crawler ejecutar python -m unittest discover -s tests. Verificación documental: desde raíz ejecutar python scripts/check_repository.py. Las bases de prueba se limpian; no usar datos reales.

Pruebas de Clientes cubren normalización, DTO, duplicados canónicos/RTN entre ocho actores concurrentes, doce solicitudes idempotentes, rollback ante fallo de auditoría, edición optimista, scopes/revocación, paginación y filtros literales, notas, historial, snapshot independiente del maestro, documentos/cuarentena y SDK MCP. Navegador comprueba alta, contacto, nota, edición persistida tras recarga, duplicado y filtro de inactivos. Pruebas UI comprueban DTO sin campos protegidos, conservación de edición/clave ante fallo, lector sin acciones y paginación de backend.

Vista 360° enlazará procesos/cotizaciones/órdenes/facturas/pagos conforme se implementen esos propietarios. Hoy no se generan documentos comerciales ni resúmenes IA ficticios. Snapshot/ResolveExact están preparados y probados, pero las futuras tablas de documentos comerciales aún no existen. Estado y evidencia de CI en [status](status.md).
