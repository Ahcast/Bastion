# Identidad visual de documentos

Decisión del usuario del 2026-10-23: usar el logo de Castillo IT suministrado en cotizaciones, facturas y notas de entrega. La imagen suministrada de firma y sello se usa en cotizaciones, en el cuadrante inferior izquierdo y a la altura de la tabla de subtotales/totales.

## Recursos compartidos

El servicio de documentos del Core lee `logo.png` y `signature.png` desde `BASTION_BRANDING_DIR`, fuera del frontend y del repositorio público. Se conservan los PNG originales, su transparencia y su proporción; no se redibujan ni alteran la firma, el sello o el logo. El directorio y los archivos permiten lectura al usuario restringido del servicio y escritura únicamente a administración del servidor. La huella de marca y el contenido de la plantilla forman parte de la identidad de cada PDF, para que cambiar recursos o diseño no reutilice un PDF anterior.

Las fichas de Facturas y notas de entrega deben reutilizar el mismo logo del Core al implementar sus plantillas. Esta decisión no habilita sus acciones PDF pendientes ni extiende automáticamente el uso de firma/sello a otros tipos de documento.

## Cotizaciones

La cabecera usa el logo compartido. Firma/sello y tabla de subtotales se colocan en dos columnas del mismo bloque: imagen a la izquierda y totales a la derecha. El bloque no se fragmenta entre páginas; en documentos largos permanece junto al cierre de los ítems. La firma no se repite a la derecha del pie. Los datos de preparador, términos, importes y permisos conservan su comportamiento existente.

Verificación visual: renderizar la plantilla Typst con los PNG suministrados y datos sintéticos claramente marcados sin validez comercial, revisar transparencia/proporciones y comprobar un documento corto y uno de varias páginas. No se crean cotizaciones reales ni se envían correos para revisar el diseño.
