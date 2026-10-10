# Ejecutar Cotizaciones (borradores)

Requiere Core Go, migraciones globales hasta 0007 en MySQL5.7 aislado y Angular. Seguir [Core](core-runbook.md); no migrar producción como parte de pruebas. BASTION_QUOTES_ENABLED=true (default) registra rutas/tools, false requiere reinicio y conserva tablas. Clientes/Procesos deshabilitados retiran sus integraciones; manual sigue funcionando. Roles requieren asignación explícita cotizaciones.view/create/update/delete/pdf; root técnico no recibe esos permisos. Papelera/restauración requieren reautenticación reciente desde el Core.

[Contrato](../contracts/quotes.md) y [OpenAPI](../contracts/quotes-openapi.json). Guardar requiere agregado editable completo, versión cadena y Idempotency-Key. Resolver un ID de cliente carga snapshot autorizado; conservar el ID mantiene el snapshot. Desvincular ID permite edición manual; un cambio del maestro no reescribe cotizaciones. Conversión desde proceso usa fecha elegida, solo entidad y expediente, deja ítems vacíos; no crea cliente automáticamente. Falta de permiso Clientes usa texto manual sin revelar coincidencias.

La interfaz Cotizaciones mantiene filtros en URL, consulta/edición/clonación/historial/papelera y cálculo solicitado al backend. Procesos muestra creación y lista paginada de cotizaciones relacionadas según permisos. Precio/cantidad/costo/descuento se transportan como cadenas; nunca usar Number para importe. El usuario revisa la última vista previa después de editar; guardar recalcula independientemente.

Validación: gofmt/vet/build y go test -race con BASTION_TEST_MYSQL_DSN dedicado a bastion_test, npm ci/build/test, scripts/check_repository.py. Browser seed es exclusivo bastion_browser_test; nunca ejecutarlo contra datos reales. Playwright añade escenario de conversión/cálculo/recarga/clonación y trazabilidad. Pruebas no acreditan S3, correo ni Bluehost real.

## Continuidad obligatoria

Cotizaciones permanece en desarrollo. Antes de cerrar módulo: estados/aprobaciones y revisiones de emisión, emisión del PDF aprobado e integración S3 real, envío inmediato/programado autorizado con mismo PDF y ledger, bulk trazable y FX fechado según necesidad comercial. Conversiones OC/Factura deben usar servicios propietarios al implementarlos sin escribir sus tablas desde Quotes. AI opcional sin impedir operación manual. No avanzar al siguiente módulo por haber aprobado solamente borradores/cálculo.

## Archivos y PDF de borrador

Core Files debe estar habilitado, con bucket privado y IAM limitado; no activar una cuenta externa durante pruebas. Configurar company en Configuración (nombre, RTN/dirección) y otorgar cotizaciones.pdf, cotizaciones.view y core.documents.write para generación, core.documents.read para descarga. BASTION_PDF_LOGO/BASTION_PDF_STAMP son rutas locales opcionales a PNG ≤1 MiB/2000x2000; inválidos detienen arranque con Quotes habilitado. Omitirlos conserva operación manual sin inventar logo/sello.

Generación usa versión guardada, snapshot público y objetos inmutables. Render/upload fuera de TX; ante fallo reintentar misma intención para recuperar pendiente, sin duplicar. PDF A4 BORRADOR, límite 250 páginas/10 MiB; unsupported glyph es 422. Si se editó después del fallo, recuperar intención anterior conserva aquella versión; generar versión nueva requiere intención nueva. S3 necesita checksum SHA256 disponible en HeadObject y PutObject condicional. La verificación de esta unidad usa store sintético; no certifica IAM/hosting real.

UI permite vincular documento existente con versión, subir directamente mediante presign sin cookie, verificar integridad y vincular. Reintento de ACK de verificación conserva cuarentena/disponibilidad ya confirmada tras reautorizar. Solo scanner autorizado puede aprobar contenido subido. Papelera/restauración Core versionada no borra objetos; bloquea descarga/reemplazo. PDFs generados no admiten reemplazo/papelera genérica. Clonación no copia adjuntos. Vista previa abre visor del navegador con tipo seguro y URL temporal; descarga fuerza attachment. Ambas requieren contenido aprobado y políticas live. Revisar PDF/adjuntos y datos guardados antes de futura emisión.

Siguiente unidad obligatoria: Workflow debe poseer aprobación humana por acción/entidad/versión/hash/vigencia, consumida por Quotes mediante servicio público. Estados y emisión no aceptan PATCH libre ni aprobación inventada. Definir contrato/migración/políticas antes de código; comprobar revisión, cambios de adjuntos, revocación y aprobación obsoleta antes de conectar SMTP inmediato/programado.

## Estados preliminares

Aplicar 0008 en entorno autorizado y asignar cotizaciones.transition explícitamente; no hay grants automáticos. Desde borrador pasar a Costeo; guardar ítems antes de Revisión. Volver a Borrador/Costeo para corregir, o guardar una edición (genera versión en Borrador). Adjuntos se modifican solo en Borrador/Costeo; papelera comercial solo Borrador. Cada paso usa versión confirmada e intención. Un timeout conserva la clave; un conflicto exige consultar la versión actual. Las ediciones sin guardar se conservan y bloquean cambios de estado en UI. PDF continúa como BORRADOR, sin certificar aprobación ni envío. Aprobaciones/correo/emisión y configuración de definiciones siguen pendientes.
