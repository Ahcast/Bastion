# Licitaciones: frontend con la dirección de Cotizaciones

Solicitud del usuario del 2026-10-27: desplegar el frontend siguiendo los lineamientos de diseño establecidos en Cotizaciones. Se aplican [Concreto](../12-direccion-visual.md) y la [organización de Cotizaciones](cotizaciones-frontend.md), sin trasladar campos, filtros o cálculos de ese dominio.

El listado de expedientes y la biblioteca de formularios usan reglas oscuras de sección, encabezados compactos, contador limitado a la página cargada, identificadores monoespaciados y estados discretos. La paginación mantiene los controles existentes. No se añade búsqueda porque el contrato de Licitaciones no la ofrece.

Las secciones del expediente sustituyen las tarjetas por reglas y espacio. Los controles de captura se organizan en dos columnas donde hay ancho suficiente y en una columna en pantallas estrechas; textos largos, mensajes y etiquetas pueden ajustarse. Las pestañas se reorganizan en dos columnas en móvil. Las tablas conservan la adaptación móvil del Core y sus etiquetas de columna. Los esquemas de formulario tienen grupos de variables separados por líneas y un pie para la publicación administrativa. Se comparten componentes de sección/paginación y tokens centrales, sin paleta nueva ni cambios en Cotizaciones.

Se conservan permisos, aprobación/publicación, navegación, captura de valores, idempotencia, protección de cambios sin guardar y manejo de conflictos 412. No se incorporan formularios o campos de negocio predeterminados. La interfaz no habilita Licitaciones: su activación administrativa sigue siendo independiente.

La validación usa la suite existente de 125 pruebas frontend y el build de producción sobre una exportación aislada de `main` con sólo este cambio, excluyendo el trabajo local de Facturas. El backend permanece en `0d951d5`; esta entrega de frontend no aplica migraciones ni reinicia AWS. La revisión visual mediante navegador local encontró un bloqueo de acceso a loopback y no contó con aprobación para Chrome; no se considera verificado el recorrido autenticado de producción por disponer del build.

La publicación usa el artifact de CI del commit correspondiente, comprueba su procedencia y hashes, respalda Bluehost, conserva `config.js`, ambas reglas `.htaccess` y los assets con hash previos, e instala los assets antes de reemplazar el HTML. La comprobación pública de archivos y rutas se registra después del despliegue.
