# Conversiones desde Cotizaciones

Solicitud del usuario del 2026-10-23: habilitar ambas conversiones cuando la integración sea correcta.

## Contrato antes de implementar

Las conversiones usan los permisos documentos/ver y documentos/editar de Cotizaciones y del destino, módulos activos y migraciones exactas. No activan módulos desde el navegador. HTTP conserva sesión, CSRF, cuota, If-Match fuerte e Idempotency-Key. No se copian archivos ni se envían correos.

Factura: reproduce createFromQuote del controlador Legacy: crea un borrador relacionado, crédito de 30 días, fecha actual de Honduras, mismos datos comerciales, impuestos y descuentos. Conserva la instantánea del cliente sin consultar de nuevo un catálogo mutable. Costos iniciales cero y gastos vacíos porque no existen en la cotización; deben completarse en Facturas. No asigna número fiscal ni emite una factura. Una factura por cotización, incluso con otra clave/actor: vuelve a abrir la existente.

Orden: reproduce la preparación del Legacy solicitando número de orden y destino antes de crear. Copia cliente/RTN, productos, venta e impuesto compatible; costos iniciales cero. Conserva el cálculo actual de Órdenes. No redondea cantidades, trunca descripciones ni elimina descuentos/ISV18: esos casos incompatibles se rechazan con explicación, y el total/impuesto/base deben coincidir exactamente con la cotización. No se inventan destinos; se administran en Ajustes > Órdenes de compra. Nueva orden explícita permitida como Legacy; reintentos de la misma operación no crean otra.

La relación guarda ID/número/revisión de origen y una instantánea histórica interna de la cotización. El destino expone solo esos tres identificadores y conserva la relación al editar; las notas siguen los permisos del documento. La instantánea conserva términos/referencias no representables en el destino sin inventar campos de formulario.

## Consistencia y arquitectura

El servicio interno document-flows coordina casos de uso públicos transaccionales de cada módulo; no escribe tablas ajenas. Cada propietario autoriza, bloquea y persiste su documento en la transacción compartida. Orden de bloqueo: Cotizaciones, después el módulo destino. Crear destino, marcar Convertido, registrar relación/recibo y auditoría se confirman juntos o se revierten juntos.

document-flows tiene migración propia; no es un módulo de navegación ni concede permisos. Tablas propias document_quote_conversions y document_conversion_receipts. Índice único de origen para Facturas, y por destino; clave idempotente por actor. Misma clave/cuerpo devuelve resultado original; cuerpo distinto devuelve 409. Las revisiones se comprueban para una creación nueva; recuperar un resultado ya confirmado no duplica documentos. Un 412 conserva los datos introducidos.

Eliminar la cotización por su flujo existente no elimina el destino: la relación conserva su identificador histórico y la instantánea; su FK viva queda nula. La eliminación futura de un destino relacionado exige su contrato y no borra la trazabilidad automáticamente.

## HTTP

GET /api/v1/cotizaciones/{id}/order-conversion, con If-Match, devuelve quote_version y reason nulo o explicación de incompatibilidad.
POST /api/v1/cotizaciones/{id}/convert-order, cuerpo n_orders e id_destination, crea una orden.
POST /api/v1/cotizaciones/{id}/convert-invoice, sin cuerpo, crea o recupera el borrador de factura.
Los POST requieren If-Match e Idempotency-Key. Respuesta: target_module, target_id, quote_id, quote_version, reused. La UI abre el documento destino por su API autorizada.

Interfaz: conserva las diez acciones. Ambas conversiones se habilitan con módulo/permisos de origen y destino; se informa el motivo si falta acceso. Órdenes solicita cabecera en un diálogo responsive, informa incompatibilidades y catálogo vacío. Una respuesta ambigua conserva cuerpo/clave y permite reintentar la misma operación. La cotización solo cambia a Convertido tras confirmar el guardado.


## Publicación verificada — 2026-10-23

Código publicado en c1eda4b8b684e385888ef134082526deb0b91be0, después de [Core checks exitoso](https://github.com/Ahcast/Bastion/actions/runs/40065905688): formato/compilación Rust, integración con MySQL 5.7 en dos bases aisladas, Typst y 99 pruebas frontend, tipos/build. Se probaron permisos de ambos módulos, sesión/CSRF/origen, revisiones, rollback por destino inválido, concurrencia con claves diferentes, recuperación idempotente, importes complejos de factura, bloqueo de pérdidas en Órdenes, conservación de origen al editar y trazabilidad al eliminar la cotización.

Compilación Rust local correcta; la ejecución autoritativa de backend se validó en CI Linux/MySQL. La revisión de navegador usó únicamente documentos y destinos sintéticos en localhost: menú habilitado, preparación y creación de ambos destinos, navegación al módulo correcto, origen visible y ausencia de errores de consola. Diálogo de Órdenes revisado a 390 px sin desbordamiento horizontal; conserva los mismos controles.

AWS y Bluehost instalados desde artifacts de ese commit, verificando SHA-256 de ZIP, paquetes y archivos. Respaldo privado previo, comparación lógica de datos de las 47 tablas existentes y de las columnas anteriores de Facturas/Órdenes; cambios únicamente aditivos. Migraciones Facturas 2, Órdenes 2 y document-flows 1 aplicadas. Se conservaron configuración de documentos/SAR y activación administrativa de todos los módulos: ambos destinos ya estaban activos, por lo que no se modificó esa configuración.

Verificación HTTPS: live/ready 200, hashes de 24 assets y consulta de preparación sin sesión 401; ambos comandos de conversión sin sesión/CSRF rechazados con 403. No se crearon documentos de prueba, enviaron correos ni consumieron números fiscales en producción.

El catálogo real de destinos sigue vacío. La conversión a Órdenes está habilitada por permisos, pero requiere que el administrador registre los destinos reales en Ajustes antes de guardar; no se agregaron valores ficticios. Las incompatibilidades documentadas conservan su explicación y bloqueo. La conversión a Facturas crea borradores, no documentos emitidos.
