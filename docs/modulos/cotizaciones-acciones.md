# Cotizaciones: acciones por documento

Decisiones del usuario, 22 de octubre de 2026: menú desplegable por cotización con las diez opciones siguientes; las conversiones permanecen visibles y deshabilitadas hasta implementar sus módulos. PDF toma como referencia el diseño de CastilloIT y correo conserva destinatarios, copias, mensaje y envío inmediato/programado. Menú publicado en el ambiente de pruebas desde el commit `8576ffcce61b1ed18d6ec20f4c3b6b8b3e892083`; documentos activados después de verificar el almacenamiento privado bajo la identidad del servicio. Empresa y SMTP global están configurados; el envío exige además las credenciales personales del actor.

## Acciones y paridad

| Opción | Comportamiento | Permiso |
|---|---|---|
| Ver | Abre los datos sin habilitar edición. Puede pasar a Editar si tiene permiso. | ver |
| Editar | Abre el editor de la revisión actual. Guardado con If-Match. | editar |
| Marcar como enviado | Cambia a Enviada; no envía correo. Conserva ítems, contacto y cálculos. | editar |
| Convertir a orden de compra | Deshabilitado por acuerdo. Legacy abre una nueva orden con quote_id para completar destino y número de orden. | editar, al implementar |
| Convertir a factura | Deshabilitado por acuerdo. Legacy crea una factura relacionada y abre su detalle; evita duplicar una conversión previa. | editar, al implementar |
| Aprobar | Cambia a Aprobada; no inventa una matriz de transiciones. | editar |
| Clonar | Copia ítems, referencia, notas, términos, descuentos y vigencia. Limpia cliente; asigna nuevo número, contacto del actor, Borrador y fecha actual de Honduras. Abre la copia para editar. No modifica el original. | editar |
| PDF | Genera/descarga el documento de la revisión seleccionada mediante Typst compartido, usando el cálculo guardado. Excluye notas privadas. | ver |
| Enviar por correo | Prepara el PDF, muestra su descarga/previsualización y permite Para, CC, CCO, asunto, mensaje y programación en hora de Honduras. Encola un envío con credenciales personales y servidor global del Core. | editar |
| Eliminar | Solicita confirmación con el número; borra con If-Match y auditoría. Cancela correos pendientes y elimina sus objetos privados mediante una cola de limpieza persistente. Rechaza mientras un correo esté enviándose. | eliminar |

Todos los permisos corresponden a `cotizaciones/documentos`. Core comprueba sesión, CSRF, actor activo, módulo permitido/habilitado y autorización. La UI no concede permisos adicionales. El menú admite teclado, Escape y cierre al pulsar fuera; se muestra fuera del contenedor de tabla para evitar recortes en pantallas pequeñas.

Fuentes estáticas: listado activo `chunk-WAOX4DBJ.js` (`markStatus`, `cloneQuote`, `convertToOrder`, `convertToInvoice`, `deleteQuote`); editor/PDF activos; `CotizacionesController.php` (`updateStatus`, `clone`, `destroy`, `sendEmail`, `processDueScheduledEmails`); `FacturasController.php` (`createFromQuote`). Revisadas en la copia indicada por el usuario. No se ejecutó Legacy ni se consultaron sus clientes, envíos o credenciales.

## Concurrencia y repetición

Estado, clonar, PDF, correo y eliminar usan la versión seleccionada. `412` exige consultar la versión actual; no se sobrescriben cambios ajenos. Estado actualiza la fila sin recargar toda la tabla. Clonar y encolar correo usan `Idempotency-Key`: repetir una solicitud devuelve su resultado, sin duplicar copia o correo; reutilizar la clave con otro cuerpo devuelve `409`.

La fecha de la copia usa Honduras (UTC−06), no el reloj del navegador. Su vigencia se recalcula con los mismos días del original. Se mantienen números históricos, fórmulas y redondeos existentes.

## Documentos compartidos del Core

`company.profile` pertenece a Ajustes > Core > Empresa, exclusivo del Administrador. Campos: nombre, RTN, correo, teléfono y dirección, derivados de la cabecera existente. No se presupone que los defaults compilados del Legacy sean datos vigentes de la empresa; se configuran explícitamente.

Typst 0.15.1 se ejecuta con plantilla fija y JSON de datos, raíz de archivos limitada a un directorio temporal privado, dos compilaciones simultáneas y plazo de 20 segundos. El contenido del cliente no se evalúa como código. Logo y sello/firma, si se configuran, son archivos PNG privados administrados por el operador; no se aceptan rutas o URLs desde el navegador. La plantilla conserva cabecera, cliente, referencia, fechas, seis columnas de ítems, bases/impuestos, valor en letras, términos y preparador. Su numeración de páginas admite documentos largos.

PDFs y adjuntos pertenecen al almacenamiento privado AWS acordado; este corte conecta PDFs mediante el puerto común de almacenamiento. No publica URLs S3. El backend verifica SHA-256 al recuperar el documento. Los originales no vencen automáticamente; al eliminar la cotización se retiran de forma eventual con reintentos persistentes. Una operación que falla no puede borrar un objeto todavía vinculado a otro resultado válido.

Configuración de entorno, solo backend: `BASTION_DOCUMENTS_ENABLED`, `BASTION_TYPST_BIN`, `BASTION_AWS_CLI`, `BASTION_DOCUMENTS_BUCKET`, `BASTION_DOCUMENTS_REGION`, `BASTION_DOCUMENTS_WORK_DIR` y `BASTION_BRANDING_DIR`. La identidad AWS necesita únicamente GetObject/PutObject/DeleteObject en el prefijo privado de cotizaciones. Bucket, región efectiva, identidad y credenciales operativas se conservan fuera del repositorio público. Mantener bloqueo público, cifrado y respaldo del bucket. Documentos permanece deshabilitado por ambiente hasta verificar lectura/escritura/eliminación con la identidad del servicio; indicar el bucket no concede esos permisos.

## Correo

La UI muestra en cola/programado hasta que SMTP confirma aceptación; no anuncia entrega por el solo hecho de guardar el trabajo. El trabajador valida de nuevo actor, permisos, módulo y revisión antes de enviar. Un cambio previo de revisión detiene el trabajo; si ocurre durante SMTP, no marca una revisión nueva como enviada. Esta protección de concurrencia es explícita de Bastion.

Se guarda cifrado el mensaje y la referencia al PDF de la revisión seleccionada. CC y CCO se deduplican; CCO forma parte del sobre SMTP, nunca de las cabeceras visibles. Host, puerto y TLS proceden del Core; las credenciales corresponden al actor, nunca al contacto histórico de la cotización ni a una cuenta alternativa. Se verifica una dirección pública y se fija esa IP manteniendo el nombre configurado para TLS. Programación guarda UTC y la UI convierte desde Honduras.

Sin asunto/mensaje se usan los defaults cotejados: referencia de orden o número de cotización, saludo al cliente y mención del PDF adjunto. El cliente manual no aporta correo de catálogo: se informa al preparar el envío. Un error o resultado incierto no se reenvía automáticamente; requiere revisar el correo enviado antes de preparar otro. La cola distingue pending/delivering/sent/failed/cancelled y registra auditoría sin texto ni destinos privados.

Límites técnicos del corte: PDF hasta 8 MiB, hasta 50 destinatarios distintos; Para hasta 255 caracteres, CC/CCO hasta 1000 cada uno, asunto hasta 255 y mensaje hasta 5000. Generación/encolado hasta 60 segundos; demás operaciones conservan 30 segundos y cuota de 120 solicitudes por actor/minuto. Son topes de implementación. No se ejecutan envíos reales en las pruebas.

## Verificación y pendientes

Se agregan pruebas de interacción del menú, lectura frente a edición, estados sin recarga, versiones, permisos, clonación idempotente y confirmación de eliminación; pruebas de contenido/MIME de PDF y zona horaria; pruebas Rust/MySQL para estados, copia, permisos y auditoría, y generación real de plantilla con texto malicioso tratado como datos.

Las conversiones siguen deshabilitadas por acuerdo. Adjuntos adicionales, selección de documentos externos, catálogo de cliente vinculado al correo, acciones masivas, interoperabilidad MCP y reconciliación contable siguen pendientes. No se presentan como terminados por añadir el menú. PDF y correo se habilitan al configurar Empresa, documentos privados y SMTP; sus errores de infraestructura no simulan resultados.

## Publicación verificada - 2026-10-22

[GitHub Actions aprobado](https://github.com/Ahcast/Bastion/actions/runs/39875208159): 50 pruebas frontend, tipos/build, Rust, MySQL 5.7 y compilación real de la plantilla con texto no confiable. Backend AWS y frontend Bluehost instalados desde artifacts del mismo commit, con hashes ZIP/tar/archivos comprobados y respaldos privados previos. La migración 0003 se aplicó y se conservaron la cotización existente, los 835 procesos y la activación de Clientes, Cotizaciones y Procesos. Se verificaron HTTPS, hashes de 18 assets públicos, health 200, consulta de negocio anónima 401 y arranque del acceso sin errores de consola. No se hicieron envíos reales ni escrituras de prueba en documentos del usuario.

Un primer intento se revirtió al detectar diferencias de CHECKSUM TABLE. Se comprobó que ese cálculo era inestable para JSON de MySQL 5.7 y que los INSERT del respaldo coincidían exactamente; el despliegue final verificó SHA-256 del contenido lógico de las tablas durante la migración. No hubo cambios de datos de negocio.

Tras el aviso del usuario de que S3 estaba preparado, se generó un PDF sintético con la plantilla real bajo el usuario restringido y el espacio de montajes del servicio. Se verificaron escritura cifrada AES256, recuperación con SHA-256 coincidente, rechazo del acceso anónimo con 403 y borrado del objeto de prueba. Con respaldo previo de configuración, se activó `BASTION_DOCUMENTS_ENABLED=true` y se reinició el backend. La misma prueba pasó tras el reinicio; health respondió 200 y las rutas de documentos conservaron 401 sin sesión. No se modificaron cotizaciones ni procesos para realizar estas pruebas.

La consulta de indicadores del Core confirmó Empresa y los tres ajustes globales SMTP; una de las dos cuentas activas dispone de credenciales personales SMTP. La dirección de Empresa está vacía y es opcional. La cola de correo estaba vacía. Quedan pendientes la prueba autenticada desde la UI de una cotización real, la confirmación de un envío real autorizado y la reconciliación contable. Credenciales, bucket real, respaldos y resultados operativos se conservan fuera del repositorio público.
