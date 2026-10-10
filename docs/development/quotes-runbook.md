# Ejecutar Cotizaciones (borradores)

Requiere Core Go, migraciones globales hasta 0010 en MySQL5.7 aislado y Angular. Seguir [Core](core-runbook.md); no migrar producción como parte de pruebas. BASTION_QUOTES_ENABLED=true (default) registra rutas/tools, false requiere reinicio y conserva tablas. Clientes/Procesos deshabilitados retiran sus integraciones; manual sigue funcionando. Roles requieren asignación explícita cotizaciones.view/create/update/delete/pdf; root técnico no recibe esos permisos. Papelera/restauración requieren reautenticación reciente desde el Core.

[Contrato](../contracts/quotes.md) y [OpenAPI](../contracts/quotes-openapi.json). Guardar requiere agregado editable completo, versión cadena y Idempotency-Key. Resolver un ID de cliente carga snapshot autorizado; conservar el ID mantiene el snapshot. Desvincular ID permite edición manual; un cambio del maestro no reescribe cotizaciones. Conversión desde proceso usa fecha elegida, solo entidad y expediente, deja ítems vacíos; no crea cliente automáticamente. Falta de permiso Clientes usa texto manual sin revelar coincidencias.

La interfaz Cotizaciones mantiene filtros en URL, consulta/edición/clonación/historial/papelera y cálculo solicitado al backend. Procesos muestra creación y lista paginada de cotizaciones relacionadas según permisos. Precio/cantidad/costo/descuento se transportan como cadenas; nunca usar Number para importe. El usuario revisa la última vista previa después de editar; guardar recalcula independientemente.

Validación: gofmt/vet/build y go test -race con BASTION_TEST_MYSQL_DSN dedicado a bastion_test, npm ci/build/test, scripts/check_repository.py. Browser seed es exclusivo bastion_browser_test; nunca ejecutarlo contra datos reales. Playwright añade escenario de conversión/cálculo/recarga/clonación y trazabilidad. Pruebas no acreditan S3, correo ni Bluehost real.

## Continuidad obligatoria

Cotizaciones permanece en desarrollo. Antes de cerrar módulo: revisiones de emisión y emisión del PDF aprobado e integración S3 real, envío inmediato/programado autorizado con mismo PDF y ledger, bulk trazable y FX fechado según necesidad comercial. Conversiones OC/Factura deben usar servicios propietarios al implementarlos sin escribir sus tablas desde Quotes. AI opcional sin impedir operación manual. No avanzar al siguiente módulo por haber aprobado solamente borradores/cálculo.

## Archivos y PDF de borrador

Core Files debe estar habilitado, con bucket privado y IAM limitado; no activar una cuenta externa durante pruebas. Configurar company en Configuración (nombre, RTN/dirección) y otorgar cotizaciones.pdf, cotizaciones.view y core.documents.write para generación, core.documents.read para descarga. BASTION_PDF_LOGO/BASTION_PDF_STAMP son rutas locales opcionales a PNG ≤1 MiB/2000x2000; inválidos detienen arranque con Quotes habilitado. Omitirlos conserva operación manual sin inventar logo/sello.

Generación usa versión guardada, snapshot público y objetos inmutables. Render/upload fuera de TX; ante fallo reintentar misma intención para recuperar pendiente, sin duplicar. PDF A4 BORRADOR, límite 250 páginas/10 MiB; unsupported glyph es 422. Si se editó después del fallo, recuperar intención anterior conserva aquella versión; generar versión nueva requiere intención nueva. S3 necesita checksum SHA256 disponible en HeadObject y PutObject condicional. La verificación de esta unidad usa store sintético; no certifica IAM/hosting real.

UI permite vincular documento existente con versión, subir directamente mediante presign sin cookie, verificar integridad y vincular. Reintento de ACK de verificación conserva cuarentena/disponibilidad ya confirmada tras reautorizar. Solo scanner autorizado puede aprobar contenido subido. Papelera/restauración Core versionada no borra objetos; bloquea descarga/reemplazo. PDFs generados no admiten reemplazo/papelera genérica. Clonación no copia adjuntos. Vista previa abre visor del navegador con tipo seguro y URL temporal; descarga fuerza attachment. Ambas requieren contenido aprobado y políticas live. Revisar PDF/adjuntos y datos guardados antes de futura emisión.

Aprobación implementada: Workflow posee aprobación humana por acción/entidad/versión/hash/vigencia, consumida por Quotes mediante servicio público. La emisión usa el contrato descrito abajo; estados no aceptan PATCH libre ni aprobación inventada. Comprobar revisión, adjuntos y aprobación obsoleta antes de conectar SMTP.

## Estados preliminares

Aplicar 0008 en entorno autorizado y asignar cotizaciones.transition explícitamente; no hay grants automáticos. Desde borrador pasar a Costeo; guardar ítems antes de Revisión. Volver a Borrador/Costeo para corregir, o guardar una edición (genera versión en Borrador). Adjuntos se modifican solo en Borrador/Costeo; papelera comercial solo Borrador. Cada paso usa versión confirmada e intención. Un timeout conserva la clave; un conflicto exige consultar la versión actual. Las ediciones sin guardar se conservan y bloquean cambios de estado en UI. PDF continúa como BORRADOR, sin certificar aprobación ni envío. Correo/emisión y configuración editable de definiciones siguen pendientes; aprobación interna descrita abajo.

## Aprobación interna humana

Migración 0009 y BASTION_WORKFLOW_ENABLED=true. Asignar cotizaciones.request_approval, cotizaciones.approve y cotizaciones.revoke_approval explícitamente según funciones; no se conceden al root por defecto. Solicitante y creador nunca deciden su propia solicitud. Revisión guardada, empresa configurada y todos los adjuntos activos verificados/disponibles son requisitos. Aprobador necesita lectura comercial y Core de cada adjunto. La vigencia se captura en hora de Honduras y se almacena UTC; no hay aprobación automática por margen/plazo.

Consultar aprobaciones → solicitar con vigencia explícita → otro usuario revisa → confirmar contraseña y TOTP/código de recuperación → aprobar o rechazar con motivo. Revocar pendiente/aprobada vuelve a Revisión y permite nueva solicitud; sirve también para renovar una solicitud vencida. Editar Aprobada internamente crea Borrador e invalida la decisión. Cambios de compañía/plantilla/adjuntos o pérdida de autoridad hacen que la decisión deje de ser vigente aunque conserve su estado histórico. Consultar la vigencia, no inferirla de la etiqueta de estado.

Ante timeout repetir misma intención/payload; el replay de solicitud confirmada funciona incluso después de vencer y vuelve a exigir permisos actuales. Una intención nueva necesita vigencia futura. Confirmar identidad borra contraseña y segundo factor del formulario tras cada intento. Deshabilitar Workflow retira rutas de aprobación; datos se conservan, hash/versiones se revalidan al reactivar.

La aprobación por sí sola no emite ni envía. La emisión es una acción separada, descrita abajo.

## Emitir PDF aprobado

Aplicar 0010 y asignar cotizaciones.emit explícitamente, además de view/pdf y core.documents.write. Configurar Files/almacenamiento privado y Workflow. Consultar aprobaciones de una versión guardada approved: solo una aprobación vigente para esa versión permite Emitir PDF aprobado. Confirmar identidad si se requiere; guardar o cancelar ediciones pendientes primero. Una emisión consume la aprobación y aparece en Consultar PDFs como Emitida; quotes conserva estado Aprobada internamente hasta implementar entrega real. No se marca Enviada ni se encola correo.

Timeout: reintentar misma intención/payload. pending conserva la proyección congelada; edición/revocación/vencimiento/pérdida de autoridad impiden completarla. Después del commit un replay autorizado recupera el documento histórico sin consumir otra aprobación, aunque se haya creado otra revisión. PDFs emitidos no se reemplazan ni se eliminan; editar crea borrador y requiere nueva revisión/aprobación para emitir de nuevo. Cambios de empresa/activos no reescriben documentos históricos.

Prueba navegador: tests/browserapp es una composición exclusiva de bastion_browser_test, development/loopback, sin email/MCP; usa almacenamiento en memoria con objetos inmutables y URLs temporales para probar descarga de bytes. No forma parte de make build ni del servidor productivo; no certifica S3/IAM. CI arranca esta composición y conserva API, políticas y MySQL reales. Producción usa cmd/server y adapter S3 original.

Próxima unidad: envío inmediato/programado con autorización específica de destinatarios/contenido/adjuntos, mismo PDF emitido y ledger SMTP; luego bulk/FX y aceptación integral antes de OC.
