# API del core y los módulos

## Propósito y principios

- Una API HTTP/JSON versionada sirve al frontend, a integraciones autorizadas y al adaptador MCP. Ningún cliente accede directamente a MySQL.
- El core centraliza identidad, autorización, validación, límites, trazas y formato de errores. Cada módulo registra sus rutas y casos de uso.
- UI, API y MCP invocan los mismos casos de uso. Cambiar de adaptador no cambia las reglas de negocio ni amplía permisos.
- Las rutas representan recursos y acciones de dominio. No se crea una ruta por cada botón ni se expone CRUD genérico directo a tablas.
- Esta especificación es normativa para el diseño; el contrato endpoint por endpoint se completa en OpenAPI antes de implementar las rutas.

## Versionado y contrato

- El prefijo base propuesto es `/api/v1`; la política de versionado compatible queda fijada así: cambios aditivos que no rompan clientes pueden permanecer en `v1`; cambios incompatibles requieren una versión mayor nueva y un plan de migración/deprecación.
- `openapi.yaml` será el contrato verificable para rutas, parámetros, esquemas, autenticación, permisos requeridos, límites, errores y ejemplos.
- La especificación se compone con las operaciones del core y los módulos habilitados en el release. No debe anunciar rutas de módulos no publicados/deshabilitados.
- Cada operación del contrato enlaza módulo, caso de uso, permiso/alcance, criterios de aceptación y compatibilidad. Los DTO son explícitos y estables; no reflejan directamente las tablas.
- El método y herramienta para generar, validar o dividir OpenAPI por módulos queda pendiente de selección técnica; el archivo integrado debe corresponder con el backend desplegado.

### Estado del primer corte

La implementación inicial solo expone `GET /health/live` y `GET /health/ready` como endpoints de infraestructura fuera de `/api/v1`. Readiness comprueba MySQL con `SELECT 1` y responde `503` sin exponer detalles de conexión si falla. No hay todavía endpoints de usuario, autenticación ni operaciones de negocio; deberán aparecer en OpenAPI y aplicar el contrato completo de esta especificación antes de habilitarse.

## Convenciones de solicitudes y respuestas

- JSON UTF-8 para solicitudes/respuestas JSON. Cada endpoint declara y valida `Content-Type`, forma, campos permitidos, tipo, longitud y rango; rechaza propiedades no admitidas para esa operación.
- Fechas y horas usan ISO 8601 con zona horaria explícita. El sistema interpreta y muestra horas en `America/Tegucigalpa`, y persiste las marcas de tiempo en UTC. Los importes se representan con precisión decimal y código ISO de moneda; no usar aritmética binaria de punto flotante para dinero.
- Las operaciones usan métodos y estados HTTP según su semántica: `GET` consulta y nunca cambia datos; `POST` crea o ejecuta un comando de dominio; `PUT` reemplaza cuando corresponda; `PATCH` cambia campos explícitamente permitidos; `DELETE` elimina o desactiva según el dominio.
- Las transiciones relevantes se expresan como operaciones explícitas —por ejemplo, registrar pago o convertir cotización— y el servidor valida que la transición sea permitida.
- Las respuestas documentan los códigos HTTP. Una creación devuelve `201` y `Location` cuando corresponda; un trabajo realmente asíncrono puede devolver `202`, solo si ese flujo está definido para el usuario.
- No incluir contraseñas, tokens, códigos OTP, claves, ni datos sensibles en URLs. No exponer SQL, rutas internas, trazas o detalles de infraestructura.

## Errores

Usar `application/problem+json` basado en [RFC 9457](https://www.rfc-editor.org/rfc/rfc9457.html), con tipos de problema y códigos estables. El objeto puede añadir extensiones comunes:

```json
{
  "type": "https://api.bastion.castilloit.net/problems/validation-error",
  "title": "La solicitud no es válida",
  "status": 422,
  "detail": "Revisa los campos indicados.",
  "instance": "urn:request:01J...",
  "code": "VALIDATION_ERROR",
  "request_id": "01J...",
  "errors": [
    { "pointer": "/valid_days", "code": "OUT_OF_RANGE", "detail": "El valor está fuera del rango permitido." }
  ]
}
```

`detail` es para personas, no para lógica del cliente; los clientes usan `status`, `code` y las extensiones documentadas. No devolver causas internas. Usar códigos HTTP con su significado correcto; el contrato define el código apropiado por operación, incluyendo `401`, `403`/`404`, `409`, `412`, `413`, `415`, `422`, `429` y errores `5xx` cuando apliquen. En respuestas `429`, enviar `Retry-After` si hay un tiempo de reintento útil.

## Listados y filtros

- Cada listado define una paginación y un tamaño máximo por operación. Usar páginas numeradas para flujos existentes que las requieran; considerar cursor para colecciones grandes o con cambios frecuentes.
- Usar orden determinista, aunque el cliente no envíe orden. Filtros y campos de orden provienen de una lista permitida; nunca se aceptan expresiones SQL o nombres arbitrarios de columnas.
- Filtros, rangos de fecha, filas por página y tamaño de respuesta tienen topes definidos en el contrato del endpoint. La UI no pierde su paginación/filtros actuales por una decisión interna de API.
- Reportes/exportaciones limitan rango, filas, tiempo y concurrencia. El resultado al alcanzar el límite se especifica por operación; no se agrega un flujo asíncrono implícitamente.

## Límites de recursos y abuso

- Cada endpoint declara topes de cuerpo, archivos, listas/lotes, filtros, filas, rango de fechas, timeout y concurrencia según su costo. Aplicarlos antes de leer/procesar todo el contenido o consultar grandes volúmenes.
- Login, OTP y recuperación tienen cuotas separadas por IP y por cuenta/desafío. Las rutas de negocio tienen límites por actor o cuenta.
- Cloudflare Free aporta la regla perimetral disponible por IP/ruta para tráfico repetitivo. No sustituye cuotas de aplicación ni límites completos de cuerpo/archivo.
- Responder con errores controlados al superar los límites, sin bloqueo permanente inducible de la cuenta. Valores concretos se fijan por operación y se afinan en ambiente de prueba para no romper flujos normales.
- Las operaciones costosas, incluyendo PDF/Typst, reportes, importaciones y exportaciones, consumen los presupuestos comunes de tiempo y concurrencia.

## Autenticación de navegador e integraciones

- Para la interfaz web, usar una sesión del servidor entregada en cookie host-only de `api.bastion.castilloit.net`; la cookie lleva `Secure`, `HttpOnly` y `SameSite` explícito. No guardar credenciales de sesión en `localStorage` o `sessionStorage`.
- El frontend `https://bastion.castilloit.net` envía la cookie en sus solicitudes cross-origin con credenciales habilitadas; CORS permite solo ese origen exacto y no se combina con orígenes comodín.
- Las solicitudes que cambian estado requieren token CSRF válido y `Origin` exacto. `Referer` exacto es respaldo documentado; `SameSite` y CORS no sustituyen estas verificaciones. Aplicar controles también al login, verificación OTP, recuperación y logout.
- El login con contraseña/OTP sigue el flujo conceptual de este documento y de `08-seguridad-y-permisos.md`: el desafío OTP no crea acceso completo; el reconocimiento de equipo/IP y la reautenticación de 15 días se aplican antes de otorgar o mantener acceso.
- Integraciones externas y clientes de IA usan credenciales distintas de la cookie del navegador. Solo un cliente previamente autorizado por un Administrador del sistema puede iniciar el flujo OAuth; esta autorización administrativa permite solicitar consentimiento, pero no otorga acceso a datos. El Administrador define el conjunto máximo de alcances por cliente; cada usuario solo puede consentir los que están dentro de ese conjunto y de sus permisos vigentes. Si cambia la ficha CIMD aprobada, el Core bloquea nuevas conexiones hasta la aprobación administrativa del cambio. Si el cliente no está autorizado, o pide alcances fuera de su máximo, el usuario puede enviar al Administrador una solicitud para autorizar el cliente o ampliar sus alcances; queda pendiente y no inicia conexión ni concede permisos hasta su aprobación. Tras aprobarse, el estado Aprobada se muestra en Ajustes y se envía un correo al solicitante para que complete OAuth y autorice sus alcances, incluidos los nuevos que haya solicitado; la aprobación administrativa no crea grants ni tokens. Si se rechaza, el estado se muestra en Ajustes > Mi cuenta > Aplicaciones conectadas, se notifica por correo al solicitante y se permite volver a solicitarlo más adelante. Para IA interactiva, usar OAuth Authorization Code + PKCE en nombre del usuario conectado; mostrar el nombre de la aplicación y alcances solicitados para que cada usuario apruebe o rechace su propia conexión. Si la aplicación solicita alcances adicionales después, pedir aprobación de nuevo. Cada solicitud se autoriza con los permisos vigentes de ese usuario y se audita bajo su identidad. Conceder alcances mínimos y permitir al usuario consultar/revocar las aplicaciones conectadas desde `Ajustes > Mi cuenta`. Emitir access tokens de 15 minutos y refresh tokens rotatorios con vigencia máxima de 15 días; al vencer, exigir nueva autorización del usuario con TOTP. Revocar el grant al cambiar/recuperar contraseña, desactivar la cuenta, revocarlo manualmente o retirar la autorización administrativa del cliente; al retirar esta última, invalidar de inmediato todos los grants y tokens asociados a ese cliente. Al quitar alcances del máximo, revocar inmediatamente los grants y tokens afectados; el usuario debe volver a completar OAuth y consentir solo los alcances aún permitidos. No compartir contraseña ni cookie web con el cliente. Las cuentas de servicio quedan para automatizaciones futuras que no tengan usuario presente; no se usan en el primer release interactivo.

## Autorización por acción, objeto y propiedad

- Cada operación declara permiso y alcance de datos; el servidor vuelve a comprobarlos en cada petición, independientemente de lo que muestre la UI o solicite un cliente IA.
- La matriz de capacidades es `ver`, `editar` y `eliminar`: `editar` incluye crear y actualizar; los comandos de dominio no destructivos usan `editar`; borrar o purgar usa `eliminar`; las consultas y exportaciones de lectura usan `ver`. La confirmación requerida por operaciones sensibles no concede permiso por sí sola.
- Al consultar o cambiar un registro, verificar acceso al objeto concreto y al tenant/empresa aplicable. Si mostrar que el objeto existe revelaría información protegida, responder de forma que no permita enumerarlo.
- Cada DTO especifica qué campos puede consultar o modificar el actor. No aplicar asignación masiva de propiedades recibidas.
- Denegar por defecto. Las acciones de alto impacto pueden requerir confirmación según el contrato del caso de uso; la confirmación no reemplaza permisos.

## Idempotencia y concurrencia de edición

- Operaciones cuya repetición puede duplicar efectos —pagos, conversiones, importaciones y envíos— aceptan `Idempotency-Key` cuando corresponda.
- La clave se limita al actor/cliente y operación. Repetir la misma clave con la misma solicitud devuelve el resultado previo; reutilizarla con contenido diferente se rechaza como conflicto. Duración de retención y almacenamiento se definen antes de implementar cada operación.
- Para recursos editables, se recomienda entregar `ETag` y exigir `If-Match` en actualizaciones para detectar ediciones basadas en una versión obsoleta. Un conflicto de versión devuelve `412 Precondition Failed` y una respuesta que permita a la interfaz recargar; no se sobrescribe silenciosamente.

## API y MCP

El adaptador MCP no invoca endpoints internos mediante una ruta privilegiada ni contiene reglas de negocio duplicadas. Valida identidad/alcance de cliente MCP y llama los mismos casos de uso que API, respetando permisos, límites, confirmación y auditoría de `06-mcp.md`.

## Recursos de referencia

- [OpenAPI Specification](https://spec.openapis.org/oas/)
- [RFC 9457 — Problem Details for HTTP APIs](https://www.rfc-editor.org/rfc/rfc9457.html)
- [OWASP REST Security Cheat Sheet](https://cheatsheetseries.owasp.org/cheatsheets/REST_Security_Cheat_Sheet.html)
- [OWASP API Security Top 10](https://owasp.org/API-Security/editions/2023/en/0x11-t10/)

## Pendiente de especificación por endpoint

Quedan por definir los nombres de rutas/DTO del core y de cada módulo, paginación concreta por listado, cuotas generales por operación, errores específicos, rutas de login/OTP y tiempos de retención de idempotencia. La vigencia del desafío, `Mantenerme loggeado`, la delegación de usuario, OAuth Authorization Code + PKCE, los tiempos/rotación/revocación de tokens y los límites TOTP por cuenta/IP se encuentran definidos en `08-seguridad-y-permisos.md`. Cada decisión se añade a OpenAPI y a la ficha del módulo antes de implementar la operación.

## Proveedores

El primer corte define listado y búsqueda, catálogo resumido, consulta individual, alta idempotente y edición versionada en `/api/v1/proveedores`. Exige los permisos `proveedores/catalogo/ver` o `proveedores/catalogo/editar` según la operación. DTO, respuestas y topes están en [OpenAPI](modulos/proveedores.openapi.yaml); las decisiones técnicas que aún no se cotejaron con Legacy se identifican en [la implementación](modulos/proveedores-implementacion.md).

## Resúmenes para Inicio

GET /api/v1/procesos/dashboard y GET /api/v1/cotizaciones/dashboard admiten period=month|year|all. Reutilizan autenticación, límites y permiso de lectura del módulo activo. Contratos, criterios de fecha y límites en [16-dashboard-modulos.md](16-dashboard-modulos.md).


Numeración SAR: GET/PUT `/api/v1/settings/sar`, exclusivamente para el Administrador. Guardado atómico de cuatro campos con versión opaca y 412 ante edición obsoleta; estado calculado por Rust. Ver [contrato](empresa-sar.openapi.yaml) y [reglas](17-empresa-sar.md).

Colaboración: `/api/v1/colaboracion` lista y crea archivos; admite selección mínima de Personas, policy en `/settings`, preview, descarga, papelera, restauración y purga confirmada. Usa sesión, CSRF, autorización por carpeta y auditoría comunes. Ver [OpenAPI](modulos/colaboracion.openapi.yaml).

## Gastos

El registro general expone `GET/POST /api/v1/gastos` y `GET/PUT/DELETE /api/v1/gastos/{id}`, con borrado lógico y resumen filtrado. `POST /api/v1/gastos/calculo` calcula sin persistir y exige editar/CSRF. Usa permisos `gastos/registros/ver`, `editar` y `eliminar`; Rust valida y calcula ISV/total. Parámetros, DTO, idempotencia y `ETag`/`If-Match` constan en [OpenAPI](modulos/gastos.openapi.yaml). Asociación con órdenes/facturas y filtros por usuario/orden siguen pendientes.

## Reportes

`GET /api/v1/reportes/{ordenes|facturas|gastos}` consulta los registros y agregados del período; `GET .../export` genera un libro SpreadsheetML 2003 con resumen y detalle. Requiere `reportes/{dominio}/ver` y lectura del módulo fuente. Gastos excluye registros borrados. DTO, filtros, respuestas y límites propuestos en [OpenAPI](modulos/reportes.openapi.yaml). MCP permanece pospuesto conforme a `06-mcp.md`.
