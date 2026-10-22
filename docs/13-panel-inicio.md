# Panel de inicio del Core

## Decisión de producto

El usuario solicita inspirar el inicio en la referencia visual Bastion · Panel y confirma cinco áreas seleccionables: Procesos activos, Cotizaciones, Órdenes de compra, Facturas y Gastos. Seleccionar una actualiza el resumen, los espacios de gráficos y la tabla inferior, sin salir de Inicio.

Esta decisión permite mostrar vistas previas de esas cinco áreas aunque todavía no existan módulos de negocio habilitados. Son espacios informativos del inicio del Core: no agregan entradas de navegación de módulos, rutas, permisos, activaciones ni herramientas MCP. Los módulos futuros deben integrar datos mediante sus contratos y autorización del servidor.

## Estado sin módulos

- Las cinco tarjetas son seleccionables y muestran «Próximamente» y «—» como valor no disponible.
- La parte inferior cambia título, descripción, evolución, distribución y encabezados de tabla según el área seleccionada.
- No se muestran importes, porcentajes, conteos, pendientes o registros ficticios. «Sin datos disponibles» no se interpreta como cero, como ausencia de pendientes ni como carga en curso.
- No hay acciones de negocio habilitadas ni solicitudes a endpoints de módulos futuros.
- Los encabezados usan conceptos descritos en las fichas existentes. No definen nuevos estados, cálculos o campos obligatorios para esos módulos.
- Las fechas de la cabecera siguen la zona horaria compartida definida por el Core. No se presenta una hora ficticia de actualización de estadísticas.

## Diseño y celular

Paleta, tipografía IBM Plex, superficies cuadradas y líneas de separación siguen `12-direccion-visual.md`. El panel seleccionado usa carbón y texto claro. El color óxido se reserva para alertas y acciones destructivas. Las fuentes se sirven localmente bajo su licencia SIL OFL.

En escritorio se muestran cinco tarjetas y una composición de varias columnas. En pantallas pequeñas las tarjetas tienen desplazamiento horizontal y puntos de ajuste; la indicación de deslizamiento permanece visible. El resumen inferior pasa a una columna y las tablas conservan su desplazamiento dentro del contenedor. La navegación móvil tiene fondo de cierre, botón de cierre, salida con Escape y foco contenido mientras está abierta.

La selección usa el patrón accesible de pestañas: un foco de Tab, selección con flechas izquierda/derecha e Inicio/Fin, relación con el panel y anuncio del área seleccionada. No depende exclusivamente del color ni del hover.

El usuario solicita también plegar el lateral para ampliar el área de trabajo. En escritorio un único botón de menú, siempre visible en la barra superior, oculta o muestra por completo el lateral. El área de trabajo ocupa el espacio liberado. El estado se conserva al navegar dentro de la sesión de la interfaz; no agrega ajustes de apariencia ni persiste valores de cuenta. En celular se mantiene el menú superpuesto, independiente del plegado de escritorio. Al ocultarse, el lateral sale del recorrido de teclado y del árbol accesible; el botón refleja el estado y conserva una etiqueta accesible.

## Integración

El menú principal contiene Inicio, los módulos habilitados que correspondan y Ajustes. Personas, Usuarios, Roles y permisos y Mi cuenta se abren dentro de Ajustes. Mi cuenta permanece disponible para cualquier usuario autenticado; las demás secciones mantienen sus permisos y los ajustes globales están agrupados en Core. El menú lateral conserva su plegado y comportamiento móvil.

`web/src/pages/Dashboard.tsx` contiene los metadatos de presentación y los componentes React de Inicio. `web/src/styles/dashboard.css` contiene su distribución adaptable; `web/src/styles/panel.css` mantiene los tokens compartidos y la navegación. El estado seleccionado se conserva en React. El código de referencia se consulta para apariencia y comportamiento solicitado, sin incorporar sus scripts ni sus datos de demostración. La compilación y publicación siguen ADR-0002.

## Validación del frontend

Con una sesión ficticia aislada se comprueba en navegador que las cinco tarjetas actualizan título, descripción, gráficos vacíos y encabezados de tabla. Se validan selección con Inicio/Fin, recorrido circular con flechas, apertura/cierre del menú, Escape y ciclo de foco del menú móvil. Se revisan anchos de 320, 390, 768 y 1440 píxeles sin desbordamiento horizontal del documento; solo las tarjetas móviles y las tablas tienen desplazamiento propio. El navegador no registra errores JavaScript. La comprobación de enrolamiento QR anterior también sigue pasando.

Se comprueba además que el plegado de escritorio libera los 248 píxeles del lateral, conserva la selección del panel de inicio y puede revertirse con Enter sobre el mismo botón. El menú oculto queda inerte y fuera del árbol accesible. Se verifica el paso a celular con el menú superpuesto y el regreso a escritorio plegado, sin fondo de cierre residual ni foco en controles ocultos.
