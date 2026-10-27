# Facturas: acciones completas del listado

## Decisiones confirmadas — 2026-10-27

El usuario solicita terminar las acciones del desplegable y confirma:

- El número SAR se asigna únicamente al **marcar como enviado**. Crear, convertir o clonar conserva un borrador sin número fiscal.
- Enviado y Pagado no admiten edición ni eliminación. Solo se eliminan borradores sin pagos, conservando las relaciones de origen.
- Para instalar la migración 5 se autoriza reutilizar la credencial MySQL existente únicamente durante ese proceso, después de un respaldo privado verificado. No crear cuentas, cambiar privilegios ni persistir credenciales de migración en el servicio.

Se conserva el orden Ver, Editar, Marcar como enviado, Ingresar pago, Clonar, PDF, Nota de entrega, Emitir Recibo y Eliminar. Una acción se deshabilita por permisos, estado, saldo, pagos o configuración real; se retiran los bloqueos permanentes por implementación pendiente.

## Estados, numeración y pagos

Marcar como enviado es una confirmación explícita; no envía correo. En una transacción se asigna el siguiente número, se capturan empresa/SAR, se incrementa versión y se registra auditoría. Queda Enviado con saldo o Pagado si ya está liquidada. La numeración pertenece al servicio compartido del Core; Facturas no accede directamente a los settings. Para emitir se requieren empresa válida, cliente y al menos una línea, fecha límite vigente y rango interpretable. Son controles de aplicación, no una validación externa ante SAR ni una declaración de cumplimiento legal.

Formato técnico admitido: `NNN-NNN-NN-NNNNNNNN` para factura actual; rango completo con una cota superior o dos extremos del mismo prefijo, separados por texto/espacios. La fecha límite no puede ser anterior al día actual de Honduras ni a la fecha del documento. No hay números fiscales predeterminados. `core/global/company.sar.sequence` conserva la máxima secuencia utilizada por prefijo y la restricción única en Facturas evita duplicados. La reserva y el avance de `company.sar.current_invoice` se confirman juntos con la factura; un error no consume número. Ajustes no permite retroceder un prefijo utilizado y eliminar no recicla números.

Pago: importe decimal positivo, fecha (día de Honduras si se omite o es nula), método opcional de hasta 120 caracteres y referencia opcional de hasta 180, como `FacturasController::addPayment` observado. No se inventa un catálogo de métodos. No hay flotantes monetarios. Se rechazan importes mayores al saldo, se suman pagos y se actualizan pagado/saldo/estado/version/auditoría juntos. Los parciales conservan Enviado y saldo cero pasa a Pagado. Límite técnico nuevo: 200 pagos por factura, además de cuotas y tamaño de petición existentes.

Los borradores pueden guardar un pago inicial dentro de la misma transacción mediante `initial_payment`; los de contado exigen total cubierto al guardar. El campo no se persiste dentro del JSON editable ni permite reemplazar el historial. La vista previa de un borrador existente consulta su versión e historial en servidor. Se conservan fechas, descuentos, costos y la utilidad confirmada el 2026-10-26: venta − ISV − inversión total.

## Clonar y eliminar

Clonar conserva los datos comerciales y del cliente como instantánea, actualiza fecha a Honduras, reinicia estado/pagos/saldo y omite número, datos fiscales emitidos y relaciones de origen. Como en Legacy, clonar contado crea un borrador sin pagos: exige pago total antes de guardar o emitir. No modifica la fuente ni consume numeración. Idempotencia por actor, clave y versión impide duplicar una clonación cuyo resultado se perdió.

Eliminar exige permiso `eliminar`, versión y confirmación. Se archiva el borrador y se excluye de detalle, búsqueda, Inicio y Reportes. No se borran la fila de origen ni los vínculos de conversión; no se permite volver a convertir un mismo origen por haber quitado su factura del listado. Los documentos privados derivados quedan en una cola de limpieza reintentable. No se añade Anulada ni se revierte contador; esta decisión sustituye el comportamiento observado de `FacturasController::destroy`.

## PDF, nota de entrega y recibo

Se usa Typst mediante `DocumentService`, almacenamiento S3 privado y prefijo `facturas/`. El módulo conserva metadatos de versión, tipo, SHA-256 y tamaño. Las descargas requieren `ver`, sesión y módulo activo, incluso después del trabajo de almacenamiento. Las tres acciones usan POST con CSRF y la versión consultada; no requieren `editar` por generar un documento.

- PDF de factura: líneas de venta, descuentos, bases/ISV, pagado/saldo y datos SAR de la instantánea emitida. Un borrador se identifica **BORRADOR — SIN EMISIÓN FISCAL**.
- Nota de entrega: cliente, referencia, número/fecha, descripción, cantidad y notas públicas; sin precios, costos ni impuestos.
- Recibo: requiere pagos registrados y muestra su fecha/método/referencia/importe, total pagado y saldo. Generarlo no registra otro pago ni inventa un correlativo fiscal de recibos.

Factura y nota usan el logo compartido del Core, sin firma/sello añadidos. No se exponen notas privadas, costos, inversión ni utilidad. El texto recibido se trata como datos de JSON, no como código Typst. Se cachea por versión, contenido, plantilla y recursos de marca; cada render tiene clave propia y el archivo huérfano se limpia sin borrar el de otro generador. Los borrados de objetos siguen la política existente del servicio privado; la retención de versiones S3 sigue el pendiente operativo común.

## Persistencia, concurrencia y alcance

Migración propia 5: columnas de número/instantáneas/archivo lógico; tablas `facturas_payments`, `facturas_command_receipts`, `facturas_documents` y `facturas_document_deletions`. No modifica precios, totales, utilidad, versiones ni orígenes de los borradores existentes. Las migraciones 1–4 permanecen inalteradas. `bastion-server migrate-facturas` aplica exclusivamente el ledger de Facturas; no migra otros módulos ni activa ninguno.

Comandos requieren `If-Match` fuerte y `Idempotency-Key` vinculada al actor y contenido; lectura/edición antigua responde 412 y otro contenido con la misma clave 409. Las respuestas ambiguas conservan clave y datos en el navegador. El evento de dominio no contiene notas, datos fiscales ni referencias de pago innecesarias. La administración del Core y los permisos separados `ver`/`editar`/`eliminar` permanecen vigentes. Reportes e Inicio reconocen Enviado/Pagado, pagos y saldos y excluyen borradores archivados. Ver [OpenAPI](facturas.openapi.yaml).

La revisión estática del controlador Legacy confirmó pagos, clonación, numeración y comportamiento de borrado; las decisiones del usuario anteriores prevalecen sobre las diferencias identificadas. No se implementan correo de facturas, anulación/reembolsos, migración Legacy, origen de adjudicación de Licitaciones ni MCP, que no son las acciones solicitadas y requieren sus contratos propios.

## Validación

Pruebas de dominio/HTTP existentes, prueba de integración nueva con MySQL 5.7 aislado y transporte S3 sintético (sin cuentas ni datos reales), y frontend con permisos, menú, confirmación y reintentos. La prueba de acciones cubre emisión concurrente, idempotencia, pagos parciales/finales/sobrepagos, clonación, contado, conservación de origen al eliminar, estados inmutables, fecha vencida, snapshots y los tres PDF reales con Typst. CI debe terminar correctamente antes de desplegar.

Revisión local de los tres PDF mediante Typst y Poppler, con texto hostil literal y datos sintéticos. Las pruebas locales sin variables MySQL no comprueban persistencia real; la evidencia operativa de CI y despliegue se registra después de verificarla. No se crean facturas, pagos o números de prueba en producción.
