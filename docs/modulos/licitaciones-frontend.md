# Licitaciones: frontend con la dirección de Cotizaciones

Solicitud del usuario del 2026-10-27: desplegar el frontend siguiendo los lineamientos de diseño establecidos en Cotizaciones. Se aplican [Concreto](../12-direccion-visual.md) y la [organización de Cotizaciones](cotizaciones-frontend.md), sin trasladar campos, filtros o cálculos de ese dominio.

El listado de expedientes y la biblioteca de formularios usan reglas oscuras de sección, encabezados compactos, contador limitado a la página cargada, identificadores monoespaciados y estados discretos. La paginación mantiene los controles existentes. No se añade búsqueda porque el contrato de Licitaciones no la ofrece.

Las secciones del expediente sustituyen las tarjetas por reglas y espacio. Los controles de captura se organizan en dos columnas donde hay ancho suficiente y en una columna en pantallas estrechas; textos largos, mensajes y etiquetas pueden ajustarse. Las pestañas se reorganizan en dos columnas en móvil. Las tablas conservan la adaptación móvil del Core y sus etiquetas de columna. Los esquemas de formulario tienen grupos de variables separados por líneas y un pie para la publicación administrativa. Se comparten componentes de sección/paginación y tokens centrales, sin paleta nueva ni cambios en Cotizaciones.

Se conservan permisos, aprobación/publicación, navegación, captura de valores, idempotencia, protección de cambios sin guardar y manejo de conflictos 412. No se incorporan formularios o campos de negocio predeterminados. La interfaz no habilita Licitaciones: su activación administrativa sigue siendo independiente.

La validación usa la suite existente de 125 pruebas frontend y el build de producción sobre una exportación aislada de `main` con sólo este cambio, excluyendo el trabajo local de Facturas. El backend permanece en `0d951d5`; esta entrega de frontend no aplica migraciones ni reinicia AWS. La revisión visual mediante navegador local encontró un bloqueo de acceso a loopback y no contó con aprobación para Chrome; no se considera verificado el recorrido autenticado de producción por disponer del build.

La publicación usa el artifact de CI del commit correspondiente, comprueba su procedencia y hashes, respalda Bluehost, conserva `config.js`, ambas reglas `.htaccess` y los assets con hash previos, e instala los assets antes de reemplazar el HTML. La comprobación pública de archivos y rutas se registra después del despliegue.

## Publicación verificada del 2026-10-27

Se publicó en Bluehost el frontend del commit `51885f26295e2f06e1b936b60e7d4038cbb9036e`, con [Core checks aprobado](https://github.com/Ahcast/Bastion/actions/runs/40587541378) y Scout control checks exitoso. Se verificaron la procedencia del artifact, el digest del ZIP y el SHA-256 del paquete `bastion-web-static.tar.gz`: `f633fa14cfd0c2a0c1455d50a7b28a9fabc7b5a1f8c45a1d11eecba921f52281`.

La versión anterior era `50efa7ebda634fb1c7dcd7d8baa26179522be9ca`. Se verificó el respaldo privado comparando los archivos archivados con los originales, se conservaron `config.js`, las dos reglas `.htaccess` y los assets anteriores para caché, y se instalaron los assets antes del cambio atómico de HTML. Se comprobaron los 46 archivos del paquete en disco y los hashes de 44 recursos públicos del manifiesto por HTTPS; las reglas Apache se verificaron en el host y se preservaron. Las rutas SPA de Licitaciones, Cotizaciones y Colaboración devolvieron el HTML esperado. El CSS publicado contiene las secciones Concreto del cambio.

Readiness HTTPS respondió 200 y las rutas privadas de Licitaciones y biblioteca de formularios sin sesión respondieron 401. El navegador integrado confirmó que la página pública carga y presenta el inicio de sesión. La revisión visual autenticada de las pantallas de Licitaciones y su adaptación en dispositivos sigue pendiente, por los límites de acceso al navegador local descritos arriba. No se usaron datos reales ni se publicaron fixtures de revisión. No se reinició AWS, no se ejecutaron migraciones ni se cambió la activación del módulo.
