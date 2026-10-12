# Reportes — operación de desarrollo

Contratos: [Reportes](../contracts/reports.md), [Excel](../contracts/reports-export.md), [OpenAPI](../contracts/reports-openapi.json). Evidencia de certificación vigente en [checkpoint](../ESTADO_DESARROLLO.md) y [estado](status.md).

## Activación

Migraciones previas y 0033 aplicadas en MySQL 5.7 de prueba. `BASTION_REPORTS_ENABLED=true` por defecto; `false` elimina las rutas/menú/resumen y conserva fuentes. Cambio exige reinicio. Fuentes obedecen sus flags independientes. No tablas comerciales/reportes paralelas ni materializaciones que ignoren revocación.

Conceder permisos explícitos por rol mediante Core: `reportes.view` y cada permiso fuente autorizado. El alcance más restrictivo de los permisos intersectados determina filas/totales. `reportes.export` agrega un tercer scope en descarga de OC. Sin concesiones automáticas de producción; los operadores de browserseed son exclusivamente sintéticos.

## Consulta y conciliación

1. Elegir reporte/período DATE inclusivo y moneda; fechas sin hora no cambian por zona. Estado aplica solo a Facturas/Órdenes.
2. Consultar: totales SQL abarcan TODO el conjunto autorizado, no solo página. No combinar HNL/USD ni OC con sus facturas. Un gasto vinculado no vuelve a restarse de utilidad OC.
3. En Inicio consultar mismo período/moneda: el total coincide por fuente, salvo cambios confirmados entre peticiones. Fuentes sin permiso no aparecen.
4. Una revocación confirmada se evalúa en siguiente petición. Error de consulta limpia datos anteriores. Una lectura ya iniciada conserva snapshot; no promete invalidación retroactiva.

## Excel (tras certificar unidad 2)

Consultar Órdenes primero. Confirmar identidad con Core y descargar Excel. Se exportan filtros de la consulta confirmada, no campos editados sin consultar. Sin paginación. Si límites de filas/tamaño se exceden, reducir intervalo; no esperar truncamiento. Descarga privada autenticada, sin enlace reutilizable ni persistencia del libro en servidor.

Importes e IDs visibles se conservan como texto exacto; Totales contiene resultados SQL por moneda y fórmulas auxiliares de centavos solamente dentro de su cota segura. Un aviso de precisión deshabilita únicamente la fórmula auxiliar; el monto exacto permanece. No cambiar a números Excel los importes grandes para conciliar con MySQL.

La auditoría acredita generación, no recepción por el navegador. Tras error de red puede solicitar una nueva proyección; esta lectura no cambia fuentes ni duplica una operación comercial. No reutiliza el archivo de otra sesión/usuario.

## Verificación

Desde raíz: `python scripts/check_repository.py` y `git diff --check`. Desde backend: gofmt sobre archivos afectados, `go vet ./...`, `go test -race -count=1 ./...` con `BASTION_TEST_MYSQL_DSN` del esquema dedicado bastion_test/MySQL 5.7; `make build`. Sin DSN las integraciones se omiten y no acreditan SQL real. Desde frontend: `npm ci`, `npm run build`, `npm test -- --watch=false`. CI navegador crea bastion_browser_test aislada, browserseed y API/Angular reales; `npx playwright test`.

No ejecutar fixtures contra esquemas comerciales. No aplicar migraciones productivas ni activar SMTP/S3/despliegue sin autorización. Las pruebas sintéticas no acreditan hosting, SAR/fiscalidad real ni integraciones externas.
