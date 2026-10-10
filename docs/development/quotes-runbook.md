# Ejecutar Cotizaciones

Requiere Core Go, migraciones globales hasta `0013_quote_batches.sql` en MySQL 5.7 aislado y Angular. Seguir [Core](core-runbook.md); no migrar producción como parte de pruebas. `BASTION_QUOTES_ENABLED=true` (default) registra rutas/tools; `false` requiere reinicio y conserva tablas. Clientes/Procesos deshabilitados retiran sus integraciones; operación manual sigue funcionando. Roles requieren asignación explícita de permisos `cotizaciones.*`; root técnico no recibe permisos comerciales por defecto. Papelera/restauración y las acciones sensibles conservan los requisitos de reautenticación del Core.

[Contrato](../contracts/quotes.md) y [OpenAPI](../contracts/quotes-openapi.json). Guardar requiere agregado editable completo, versión como cadena e `Idempotency-Key`. Resolver un ID de cliente carga snapshot autorizado; conservar el ID mantiene el snapshot. Desvincular ID permite edición manual; un cambio del maestro no reescribe cotizaciones. Conversión desde Proceso usa fecha elegida, solo entidad y expediente, deja ítems vacíos y no crea cliente automáticamente. Falta de permiso Clientes usa texto manual sin revelar coincidencias.

La interfaz Cotizaciones mantiene filtros, consulta/edición/clonación/historial/papelera, cálculo solicitado al backend, aprobación, emisión, correo y bulk según permisos. Procesos muestra creación y lista paginada de cotizaciones relacionadas. Precio/cantidad/costo/descuento se transportan como cadenas; nunca usar `Number`/float para importes. Guardar recalcula independientemente de la vista previa.

Validación: `gofmt`, `go vet`, `make build` y `go test -race -count=1 ./...` con `BASTION_TEST_MYSQL_DSN` dedicado a `bastion_test`; `npm ci`, build/test Angular; `scripts/check_repository.py`; Playwright con `bastion_browser_test`. Los seeds de navegador son sintéticos y nunca se ejecutan contra datos reales.

## Continuidad obligatoria

Cotizaciones permanece como módulo activo. Borradores/cálculo, archivos/PDF, estados, aprobación, emisión, correo comercial y bulk están implementados y cubiertos en CI. La unidad siguiente es **FX fechado**, pero el contrato comercial todavía no define fuente/tipo de tasa, fecha efectiva, snapshot, redondeo, autoridad de modificación ni presentación. No inventar esas reglas ni implementar conversión implícita.

Las conversiones Cotización→OC/Factura deben usar servicios propietarios cuando existan esos módulos; Quotes no escribe directamente sus tablas. No iniciar OC todavía mientras la aceptación restante de Cotizaciones esté abierta.

Integraciones reales S3/IAM/scanner, SMTP/IMAP Bluehost, Percona Bluehost y despliegue siguen siendo validaciones externas de producción. Los adapters, stores sintéticos y CI no las certifican.

## Archivos y PDF de borrador

Core Files debe estar habilitado, con bucket privado e IAM limitado; no activar una cuenta externa durante pruebas. Configurar empresa en Configuración (nombre, RTN/dirección) y otorgar `cotizaciones.pdf`, `cotizaciones.view` y `core.documents.write` para generación, `core.documents.read` para descarga. `BASTION_PDF_LOGO`/`BASTION_PDF_STAMP` son rutas locales opcionales a PNG ≤1 MiB/2000x2000; inválidos detienen arranque con Quotes habilitado. Omitirlos conserva operación manual sin inventar logo/sello.

Generación usa versión guardada, snapshot público y objetos inmutables. Render/upload ocurre fuera de TX; ante fallo se reintenta la misma intención para recuperar pendiente sin duplicar. PDF A4 BORRADOR, límite 250 páginas/10 MiB; glyph no soportado es 422. Si se editó después del fallo, recuperar la intención anterior conserva aquella versión; generar versión nueva requiere intención nueva. S3 necesita checksum SHA256 disponible en `HeadObject` y `PutObject` condicional. La verificación de CI usa almacenamiento sintético y no certifica IAM/hosting real.

UI permite vincular documento existente con versión, subir mediante presign sin cookie, verificar integridad y vincular. Reintento de ACK conserva cuarentena/disponibilidad ya confirmada tras reautorizar. Solo scanner autorizado puede aprobar contenido subido. Papelera/restauración Core versionada no borra objetos; bloquea descarga/reemplazo. PDFs generados no admiten reemplazo/papelera genérica. Clonación no copia adjuntos. Vista previa abre tipo seguro con URL temporal; descarga fuerza attachment. Ambas requieren contenido aprobado y políticas live.

## Estados

Aplicar `0008_quote_states.sql` y asignar `cotizaciones.transition` explícitamente. Desde Borrador pasar a Costeo; guardar ítems antes de Revisión. Volver a Borrador/Costeo para corregir, o guardar una edición, que genera otra revisión en Borrador. Adjuntos se modifican solo en Borrador/Costeo; papelera comercial solo Borrador. Cada transición usa versión confirmada e intención. Un timeout conserva la clave; un conflicto exige consultar versión actual. Ediciones sin guardar bloquean cambios de estado en UI.

## Aprobación interna humana

Migración `0009_approvals.sql` y `BASTION_WORKFLOW_ENABLED=true`. Asignar `cotizaciones.request_approval`, `cotizaciones.approve` y `cotizaciones.revoke_approval` según funciones. Solicitante y creador no deciden su propia solicitud. Revisión guardada, empresa configurada y adjuntos activos verificados/disponibles son requisitos. Aprobador necesita lectura comercial y Core de cada adjunto. La vigencia se captura en hora de Honduras y se almacena UTC; no hay aprobación automática por margen/plazo.

Consultar aprobaciones → solicitar con vigencia explícita → otro usuario revisa → confirmar contraseña y TOTP/código de recuperación → aprobar/rechazar con motivo. Revocar pendiente/aprobada vuelve a Revisión y permite nueva solicitud. Editar una cotización aprobada crea Borrador e invalida vigencia. Cambios de compañía/plantilla/adjuntos o pérdida de autoridad también invalidan la decisión aunque conserve estado histórico. Consultar `valid`, no inferir vigencia solo de la etiqueta.

Ante timeout repetir misma intención/payload. Replay de solicitud confirmada funciona incluso después de vencer y vuelve a exigir permisos actuales. Intención nueva necesita vigencia futura. Deshabilitar Workflow retira rutas de aprobación; datos se conservan y se revalidan al reactivar.

## Emitir PDF aprobado

Aplicar `0010_quote_emission.sql` y asignar `cotizaciones.emit`, además de view/pdf y `core.documents.write`. Solo una aprobación vigente de la versión guardada permite emitir. Una emisión consume la aprobación y crea PDF inmutable; no re-renderiza el documento histórico al reintentar.

Timeout: repetir misma intención/payload. `pending` conserva la proyección congelada; edición/revocación/vencimiento/pérdida de autoridad impiden completarla. Después del commit un replay autorizado recupera el documento histórico sin consumir otra aprobación. PDFs emitidos no se reemplazan ni eliminan; editar crea Borrador y exige nueva revisión/aprobación/emisión.

## Correo comercial inmediato y programado

El transporte Core de correo y la programación durable usan `0011_outbox_schedule.sql`; las autorizaciones comerciales de Quotes usan `0012_quote_email_authorizations.sql` y Workflow habilitado.

Enviar una cotización exige autorización humana específica ligada a destinatarios To/CC/BCC, asunto, cuerpo, PDF emitido, adjuntos y horario. La autorización tiene versión/hash/vigencia y no equivale a permiso general de correo. El worker vuelve a comprobar permisos y binding inmediatamente antes del efecto externo.

El ledger de entrega es durable. Integridad de PDF y adjuntos se verifica por SHA/size antes de SMTP. Si el resultado SMTP es incierto, queda `uncertain` y el sistema no reenvía a ciegas. Revocación de autorización sigue disponible aunque la evidencia cambie; revocar no afirma que un correo ya enviado pueda recuperarse.

Las pruebas sintéticas verifican autorización, revalidación, programación, ledger y condiciones de incertidumbre, pero no certifican el SMTP/IMAP ni límites reales de Bluehost.

## Bulk de Cotizaciones

Aplicar `0013_quote_batches.sql`. Las rutas son:

- `POST /api/v1/quotes/bulk/status`
- `POST /api/v1/quotes/bulk/clone`
- `POST /api/v1/quotes/bulk/export`

Todas requieren `Idempotency-Key`, selección no vacía de máximo 100 cotizaciones y versiones como cadenas. La selección no admite IDs repetidos. La cabecera del lote y sus ítems se persisten; la respuesta devuelve `batch_id`, estado del lote y resultado por posición/cotización.

Estado y clonación reutilizan las operaciones individuales existentes con claves derivadas deterministas. Un replay con la misma intención recupera el mismo `batch_id`; no duplica efectos. Fallos de autorización/validación/conflicto/not-found quedan como resultado por ítem. Un error interno/transitorio no se convierte en éxito: deja trabajo pendiente para poder reintentar sin asumir si ocurrió el efecto. Permisos y versión se revalidan mediante las operaciones propietarias.

Exportación congela por ítem un resumen público autorizado: número, cliente, referencia, fecha, moneda, total, estado y versión. No exporta costo, margen ni notas privadas. Campos con prefijos de fórmula de hoja de cálculo se neutralizan. Replay devuelve el CSV congelado aun si la cotización cambió después.

Angular presenta selección múltiple solo sobre filas visibles y no muestra bulk en Papelera. Tras una operación muestra conteo de éxitos/fallos por lote. Un retry de la misma acción/payload conserva la clave de idempotencia. Playwright real cubre selección de dos cotizaciones y descarga CSV sin fuga de nota privada.

Checkpoint certificado para bulk: commit `e3f70fee85a8fd1be6dcdd56fd9fc0ada20f6328`, Core checks `38088666258`, Documentation checks `38088666202`, Single branch `38088666297`.

## FX — siguiente unidad

La moneda de una cotización sigue siendo explícitamente `HNL` o `USD`; no existe conversión implícita. Antes de escribir migración o código FX debe cerrarse un contrato normativo que defina como mínimo:

1. fuente de tasa;
2. compra/venta/u otra tasa aplicable;
3. fecha/hora efectiva y zona horaria;
4. snapshot/historial y comportamiento ante correcciones;
5. precisión y redondeo;
6. permisos para crear/corregir/sustituir tasa;
7. efecto sobre borradores, aprobadas y emitidas;
8. representación en UI, PDF y exportaciones.

Hasta que eso exista, cualquier conversión automática USD↔HNL sería una regla comercial inventada y debe permanecer deshabilitada.


## Cierre monomoneda

HNL o USD por cotización, sin conversión automática (decisión del usuario). Tras SMTP aceptado, recargar ficha muestra Enviada; no acredita lectura/entrega final. Registrar motivo y pasar a Aceptada/Rechazada/Vencida. Vencida requiere fecha local posterior a fecha+validez. Reabrir en Borrador antes de editar una cotización enviada/final. PDF emitido histórico se conserva. Lotes requieren cerrar la ficha; tras timeout usar Reintentar lote, que conserva IDs/versiones/cuerpo/clave. CSV confirmado puede descargarse otra vez.

El navegador de CI usa API/Angular/MySQL reales y adaptadores de objetos y correo sintéticos sin red; nunca configura Bluehost ni ejecuta SMTP real. El worker de pruebas ejecuta la misma lógica comercial. Ver status.md para evidencia final.
