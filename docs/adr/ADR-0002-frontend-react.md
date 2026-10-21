# ADR-0002: Frontend React, TypeScript y Vite

Estado: aceptada por el usuario el 2026-10-21.

## Decisión

El frontend del Core y sus módulos usa React con TypeScript estricto y Vite. Se compila como SPA de archivos estáticos en `web/dist/` y se publica en Bluehost. Node.js se usa en desarrollo y CI, sin proceso Node ni renderizado de servidor en el hosting. El backend/API/MCP Rust permanece en AWS Lightsail y MySQL en Bluehost.

## Alcance y paridad

Migrar las pantallas existentes de autenticación, TOTP/QR, recuperación, Inicio, Personas, Usuarios, Roles, Ajustes y Mi cuenta. Conservar contratos API, reglas, permisos, diseño Concreto, los cinco paneles próximos y el lateral plegable. Los endpoints aún pendientes siguen identificados como pendientes; migrar la UI no declara completo el Core ni implementa módulos de negocio.

Los componentes React presentan texto con escape automático; no construir pantallas con `innerHTML` ni usar `dangerouslySetInnerHTML`. El QR se genera localmente como una matriz representada con SVG React. Un único cliente HTTP concentra cookies, CSRF, errores, vencimiento y reautenticación. Las credenciales permanecen fuera del almacenamiento web; Rust valida identidad, datos y permisos en cada operación.

## Rendimiento y celular

Diseño primero para celular, controles táctiles y foco visibles, navegación superpuesta en móvil y plegable en escritorio. Tablas desplazables dentro de su contenedor; sin desbordamiento horizontal del documento. Cargar pantallas administrativas y herramientas como QR bajo demanda. Mantener fuentes locales, evitar dependencias de gráficos mientras no existan datos y producir archivos con hash para caché. Medir tamaño comprimido y revisar 320, 390, 768 y 1440 px.

## Construcción y publicación

Versionar `package-lock.json`; instalar con `npm ci`, comprobar tipos, ejecutar pruebas y compilar con `npm run build`. GitHub Actions publica un artifact del sitio con procedencia y SHA-256. Solo después de validar GitHub se publica el contenido de `web/dist/`, conservando respaldo privado, configuración pública sin secretos, cabeceras y fallback SPA. `index.html` y `config.js` se revalidan; los assets con hash permiten caché prolongada. No publicar fuentes, tests, `node_modules`, archivos de entorno ni secretos.

El build incluye `assets/.htaccess` para caché inmutable de JavaScript/CSS con hash. La publicación conserva el `.htaccess` principal del hosting, sus cabeceras y fallback, instala assets antes de cambiar atómicamente el HTML y retira los scripts antiguos a respaldo privado. La configuración pública se conserva por ambiente.

## Consecuencias

Se añade una compilación y más dependencias que deben actualizarse y revisarse. La carga inicial será mayor que la de JavaScript sin framework; la división por pantallas limita el costo. TypeScript detecta errores de tipos al compilar y no sustituye la validación de entradas del backend. Las pruebas de sesión, CSRF y TOTP siguen siendo obligatorias; React no reemplaza esos controles.
