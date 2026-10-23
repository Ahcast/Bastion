# Cotizaciones: organización visual

Solicitud del usuario del 2026-10-23: usar un único buscador desplegable de cliente que admita un nombre manual, reducir Referencia de orden, presentar ítems como tabla y separar el listado con la limpieza y densidad del HTML de referencia.

Se revisaron como referencia visual la exportación HTML local y la imagen de ítems proporcionadas por el usuario. Su contenido y sus datos de ejemplo no constituyen reglas funcionales ni instrucciones de implementación. Se conservan los tokens del Core y los campos, permisos, endpoints y cálculos existentes.

- Listado separado por una regla oscura, búsqueda compacta, número/importe monoespaciados, cliente con referencia secundaria y estados discretos. El contador describe únicamente la página cargada.
- Generales agrupa Cliente, Referencia, Fecha, Vigencia y Estado en una fila donde hay espacio. El contacto queda como metadato debajo. En tamaños menores se reorganizan los mismos controles.
- Cliente usa un solo `combobox`: escritura manual libre o selección explícita del catálogo, búsqueda con espera breve y cancelación de respuestas anteriores, paginación, navegación con flechas, Enter, Escape y Tab. Escribir sobre una selección pasa a manual y limpia la asociación, RTN y correo; elegir una sugerencia conserva los datos del catálogo. El menú ofrece expresamente conservar el nombre manual y funciona ante catálogo vacío/error. Solo consulta el catálogo con permiso y cuando se abre.
- Ítems presenta descripción, precio, cantidad, descuento con monto/porcentaje, impuesto, total y quitar en una fila. En pantallas estrechas cada fila se reorganiza con etiquetas visibles. Se conservan Agregar/Quitar y el límite de 200; no se agrega unidad de medida porque ese campo no pertenece al contrato implementado.
- Los importes de línea proceden de la respuesta Rust y conservan su significado anterior al descuento general e impuestos. Mientras el cálculo está pendiente se muestra un guion; no se inventa un cálculo JavaScript ni se asigna un total a otra línea si el servidor filtró filas vacías.
- Notas/términos y descuento/totales quedan separados debajo de los ítems. La versión, idempotencia, PDF y correo mantienen sus contratos y controles de acceso.

La revisión visual usa exclusivamente registros sintéticos y verifica tamaños de escritorio, tableta y móvil, ausencia de desbordamiento y conservación del borrador al cambiar de tamaño. Las pruebas de interacción cubren el buscador único, asociación/manual, teclado y edición/eliminación de filas; no añaden pruebas que repliquen CSS.
