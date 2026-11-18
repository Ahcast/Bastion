# Navegación web y enlaces directos

Decisión del propietario, 2026-11-18: todos los módulos y pantallas web deben tener navegación por URL, Atrás/Adelante y enlaces directos, conservando permisos. El cliente sigue siendo React/TypeScript compilado con Vite y servido estáticamente en Bluehost; la API Rust y MySQL no cambian.

## Contrato

- `web/src/routes.ts` centraliza rutas, parámetros admitidos y comprobaciones de acceso. `web/src/navigation.ts` sigue siendo la fuente de menú y permisos por sección; no se duplican roles ni se cambia el JSON de permisos.
- `web/src/routing.tsx` adapta React Router 8.5.1, History API y los estados de pantalla. `main.tsx` monta el router de navegador una sola vez. Node mínimo 22.22.0; CI usa 24.19.0.
- La URL contiene únicamente identificadores, pestañas, modo y paginación. Contraseñas, tokens, importes, archivos pendientes y borradores no se escriben en la URL ni en `history.state`. Compartir un enlace comparte el destino, no una sesión ni un borrador.
- Abrir o recargar un registro consulta su versión actual con la API existente. Las acciones de creación, envío, pago, eliminación, restauración o cambio de autenticador requieren su interacción y validación habituales: navegar nunca las ejecuta automáticamente.
- Cada navegación genera una entrada. Cambios relacionados del mismo clic se agrupan. Al guardar un documento nuevo, su URL provisional se reemplaza por el ID persistido cuando ya no hay archivos/cambios pendientes.
- Atrás/Adelante usan el mismo aviso de cambios sin guardar que el menú. Se puede seguir editando, descartar o guardar si la pantalla ofrece guardado coordinado. Una escritura pendiente bloquea la salida. Cerrar o recargar la pestaña usa el aviso nativo; no se promete conservar un borrador tras recargar.
- Las respuestas de cargas abandonadas se cancelan o ignoran para no abrir el registro anterior sobre el destino nuevo.

## Rutas

Los ID de ejemplo son ilustrativos; deben sustituirse por registros existentes y autorizados. `modo=view`/`modo=edit` distingue consulta y edición cuando la pantalla ofrece ambos.

| Pantalla | Ejemplo |
| --- | --- |
| Inicio y panel | `/?panel=cotizaciones` |
| Procesos, Compra menor y detalle | `/procesos`, `/procesos/ID-PROCESO/compras` |
| Procesos, Licitación de Scout | `/procesos/licitaciones` |
| Cotizaciones, listado/detalle/nuevo | `/cotizaciones`, `/cotizaciones/42`, `/cotizaciones/nuevo?modo=edit` |
| Correo e historial de envíos | `/cotizaciones?correo=42`, `/cotizaciones?envios=42` |
| Clientes / Proveedores | `/clientes/42`, `/proveedores/42`, variantes `/nuevo` |
| Órdenes y documentos asociados | `/ordenes-compra/42`, `/ordenes-compra?documentos=42` |
| Facturas / Gastos | `/facturas/42`, `/gastos/42`, variantes `/nuevo` |
| Contabilidad | `/contabilidad/diario`, `/contabilidad/42/diario`, `/contabilidad/integrar`, `/contabilidad/cuentas`, `/contabilidad/periodos`, `/contabilidad/balance` |
| Cuenta o período contable | `/contabilidad/cuentas?cuenta=1101`, `/contabilidad/periodos?periodo=2026-11` |
| Reportes | `/reportes/ordenes`, `/reportes/facturas`, `/reportes/gastos` |
| Colaboración | `/colaboracion/files?ambito=empresa`, `/colaboracion/files?ambito=persona&persona=42`, `?papelera=1`, `?archivo=42` |
| Biblioteca de formularios | `/colaboracion/forms`, `/colaboracion/42/forms` |
| Expediente de Licitaciones | `/licitaciones/42/resumen`, `/variables`, `/legal`, `/tecnica`, `/economica`, `/pdf`, `/adjudicaciones` como última sección del mismo expediente |
| Lote / revisión de adjudicación | `/licitaciones/42/economica?lote=2`, `/licitaciones/42/adjudicaciones?adjudicacion=7` |
| Mi cuenta | `/ajustes/cuenta/profile`, `/ajustes/cuenta/security`, `/ajustes/cuenta/smtp` |
| Personas / Usuarios / Roles | `/ajustes/personas/42`, `/ajustes/usuarios?invitar=1`, `/ajustes/roles/42` |
| Core | `/ajustes/core/modules`, `/company`, `/smtp`, `/audit`, `/mail` como última sección del Core |
| Respaldos / MCP | `/ajustes/respaldos`, `/ajustes/mcp` (solo si está habilitado) |
| Ajustes de módulos | `/ajustes/` más la clave del módulo registrada, por ejemplo `ordenes_compra`, `contabilidad`, `licitaciones` |

Las confirmaciones transitorias, selectores auxiliares y despliegues de contenido dentro de una pantalla no son órdenes ejecutables mediante URL. Los PDF siguen abriéndose con su visor y nombre de archivo existentes. Filtros de texto y borradores mantienen su tratamiento anterior; el enlace no serializa todo el formulario.

## Permisos y errores

Primero se carga la sesión y su catálogo de acceso. Sin sesión se muestra el ingreso conservando el destino. Un módulo inactivo, sección administrativa o modo de edición no autorizado se rechaza antes de montar la pantalla. La API sigue verificando usuario, módulo, alcance, acción y objeto en cada petición; conocer un ID no concede acceso. No se alteran CSRF, TOTP, cookies, límite de sesiones ni idempotencia.

Una ruta desconocida muestra «Página no encontrada». Los errores de lectura de un registro se muestran sin sustituirlo por otro registro ni crear uno nuevo. Los enlaces de activación, recuperación de contraseña y consentimiento MCP conservan sus flujos específicos y limpieza de tokens.

## Despliegue y comprobación

Bluehost debe responder con `index.html` cuando la ruta no corresponde a un archivo/directorio real. La regla existente `RewriteCond ... !-f`, `!-d`, `RewriteRule ^ index.html [L]` ya lo permite. Los recursos usan rutas absolutas. Se conserva `config.js`, CSP, `.htaccess`, favicon y recursos anteriores necesarios para pestañas abiertas.

Pruebas locales/CI: `npm test`, `npm run build`, `npm audit --omit=dev --audit-level=high`. Las nuevas suites `routing.test.tsx` y `app-routes.test.tsx` cubren catálogo, rechazo de parámetros ajenos, permisos, lectura directa, historial, agrupación de entradas, cancelación de cargas y guardas. No crean usuarios ni registros en producción.

Comprobación manual con una cuenta existente: abrir un listado, abrir un documento, usar Atrás y Adelante; copiar la URL en otra pestaña y recargar; entrar a Ajustes y sus secciones; editar un campo y probar cancelar/descartar la salida; comprobar que un usuario de solo lectura no abre `?modo=edit`. Un documento eliminado o no autorizado debe mostrar error sin revelar su contenido.
