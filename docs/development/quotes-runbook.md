# Ejecutar Cotizaciones (borradores)

Requiere Core Go, migraciones globales hasta 0006 en MySQL5.7 aislado y Angular. Seguir [Core](core-runbook.md); no migrar producción como parte de pruebas. BASTION_QUOTES_ENABLED=true (default) registra rutas/tools, false requiere reinicio y conserva tablas. Clientes/Procesos deshabilitados retiran sus integraciones; manual sigue funcionando. Roles requieren asignación explícita cotizaciones.view/create/update/delete; root técnico no recibe esos permisos. Papelera/restauración requieren reautenticación reciente desde el Core.

[Contrato](../contracts/quotes.md) y [OpenAPI](../contracts/quotes-openapi.json). Guardar requiere agregado editable completo, versión cadena y Idempotency-Key. Resolver un ID de cliente carga snapshot autorizado; conservar el ID mantiene el snapshot. Desvincular ID permite edición manual; un cambio del maestro no reescribe cotizaciones. Conversión desde proceso usa fecha elegida, solo entidad y expediente, deja ítems vacíos; no crea cliente automáticamente. Falta de permiso Clientes usa texto manual sin revelar coincidencias.

La interfaz Cotizaciones mantiene filtros en URL, consulta/edición/clonación/historial/papelera y cálculo solicitado al backend. Procesos muestra creación y lista paginada de cotizaciones relacionadas según permisos. Precio/cantidad/costo/descuento se transportan como cadenas; nunca usar Number para importe. El usuario revisa la última vista previa después de editar; guardar recalcula independientemente.

Validación: gofmt/vet/build y go test -race con BASTION_TEST_MYSQL_DSN dedicado a bastion_test, npm ci/build/test, scripts/check_repository.py. Browser seed es exclusivo bastion_browser_test; nunca ejecutarlo contra datos reales. Playwright añade escenario de conversión/cálculo/recarga/clonación y trazabilidad. Pruebas no acreditan S3, correo ni Bluehost real.

## Continuidad obligatoria

Cotizaciones permanece en desarrollo. Antes de cerrar módulo: estados/aprobaciones y revisiones de emisión, plantilla PDF pública sin costos/proveedores/notas, snapshot PDF durable en S3, política de adjuntos y cuarentena, envío inmediato/programado autorizado con mismo PDF y ledger, bulk trazable y FX fechado según necesidad comercial. Conversiones OC/Factura deben usar servicios propietarios al implementarlos sin escribir sus tablas desde Quotes. AI opcional sin impedir operación manual. No avanzar al siguiente módulo por haber aprobado solamente borradores/cálculo.
