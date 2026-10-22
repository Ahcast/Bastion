# Criterios de aceptación y cierre de paridad

## Frontend React

- React + TypeScript estricto + Vite, con lockfile y build estático reproducible para Bluehost; verificar tipos, pruebas y compilación en CI antes de publicar.
- Preservar las pantallas y contratos existentes del Core, QR generado localmente, sesiones/CSRF y reautenticación, navegación y permisos. No declarar implementados endpoints pendientes.
- No insertar HTML dinámico; datos del API presentados por React. Sin credenciales en almacenamiento web o build.
- Comprobar 320, 390, 768 y 1440 px, teclado, foco de modales y menú, lateral plegable y cinco áreas de Inicio sin estadísticas ficticias. Sin scroll horizontal del documento.
- Comprobar recuperación CSRF, un único reintento seguro, logout fallido visible, vencimiento/reautenticación y errores sin perder el control de la interfaz.
- Publicar únicamente el build validado, respaldar la versión previa fuera del directorio público y verificar por HTTPS los hashes de los archivos publicados.

## Criterio por pantalla

- Roles y permisos usa la matriz visual descrita en `14-permisos-por-rol.md`: selector de rol, columnas Ver, Crear y editar (una sola casilla) y Eliminar independiente, dependencias con Ver, controles masivos, conservación de permisos fuera de matriz, confirmaciones y protección del borrador al salir o reautenticar. El backend rechaza cambios sin autoridad administrativa, cambios al rol integrado, duplicados y permisos nuevos de módulos no disponibles; conservar una entrada histórica exacta no le concede nuevas capacidades.

Una pantalla se considera documentada cuando especifica:

- ruta, módulo, rol y permiso;
- campos con etiqueta, nombre, tipo, requerido, valor inicial, opciones, formato y validación;
- acciones/botones y resultado, confirmación, mensaje y navegación posterior;
- búsqueda, filtros, orden, paginación y acciones por fila;
- estados de carga, vacío, éxito y error;
- datos leídos/modificados, efecto secundario y auditoría;
- comparación con evidencia de la aplicación actual.

## Criterio por módulo

- Está en catálogo basado en el informe.
- Tiene ficha en `docs/modulos/` y contrato de registro definido.
- Su sección aparece bajo Ajustes con settings/versionado/permisos.
- API y MCP invocan los mismos casos de uso y permisos.
- Datos, cálculos y conversiones tienen criterios comprobables.
- Facturas usan `Borrador`, `Enviado` y `Pagado`; un pago parcial deja la factura `Enviado` y solo saldo cero permite `Pagado`. La transición a `Enviado` y la corrección posterior están definidas antes de implementar esas operaciones.
- Los PDFs se generan mediante Typst y respetan datos, cálculos, permisos y acciones documentados para el módulo; cualquier diferencia respecto al documento actual queda identificada al revisar paridad.
- Sus flujos principales cubren éxito, validación, falta de permiso y error de dependencia.
- Cada endpoint está descrito en OpenAPI antes de implementarse; contrato y backend coinciden con los módulos habilitados y declaran esquemas, permisos, límites y respuestas.
- Los errores HTTP usan `application/problem+json`, código estable e identificador de solicitud; no exponen SQL, stack traces ni secretos.
- Los listados tienen paginación/tamaño máximo, orden determinista y filtros permitidos; conservan los controles de listado existentes en la interfaz.
- Las mutaciones que pueden duplicar pagos, conversiones, importaciones o envíos son idempotentes cuando su contrato así lo declara; las repeticiones no duplican efectos.
- Las ediciones concurrentes detectan versiones obsoletas mediante `ETag`/`If-Match` donde se habilite y permiten recargar los datos actuales sin sobrescritura silenciosa.

## Criterio del core

- Cada pantalla de Core enumerada en `02-core-y-ajustes.md` tiene inventario de campos, botones, validaciones, resultados y navegación trazable al informe/código Legacy; lo no comprobado está marcado como pendiente y no se inventa.
- El HTML de Bastion guía la presentación visual; los campos, permisos y comportamiento se derivan del informe/código del Legacy.
- Inicio de sesión, perfil y administración respetan roles.
- En Personas, únicamente el nombre es obligatorio; DNI, país y género son opcionales. Si se proporcionan, backend valida formato, longitud, catálogo y unicidad aplicable de forma uniforme para crear/editar.
- Roles y Pantallas validan en backend todos los campos y relaciones de alta/edición, rechazan propiedades no permitidas, verifican unicidad normalizada y referencias, y tienen restricciones de base de datos que cubren concurrencia. Los errores no revelan detalles internos.
- Los permisos de UI, API y MCP son `ver`, `editar` (incluye crear, actualizar y comandos de dominio no destructivos) y `eliminar`; el servidor los aplica por acción y objeto, con denegación por defecto.
- Los controles de seguridad, dispositivos y sesiones en `Ajustes > Mi cuenta` se limitan al usuario autenticado; desde allí no se pueden cambiar políticas globales del Core. El usuario ve y revoca sus dispositivos individualmente.
- En Sesiones, el usuario ve dispositivo/navegador, IP, fecha de inicio, última actividad y la sesión actual; puede revocarla. Hay solo una sesión activa por usuario. Un nuevo acceso la reemplaza solo después de completar la autenticación requerida y la anterior muestra un aviso al siguiente intento. Un token revocado deja de autorizar solicitudes.
- En Seguridad, el usuario consulta eventos de acceso de su propia cuenta de los últimos 30 días, con fecha, IP, dispositivo y resultado.
- Tras login exitoso desde dispositivo nuevo se intenta enviar aviso al correo registrado usando las credenciales SMTP de ese usuario; si no las configuró o el envío falla, se registra sin bloquear el acceso. Un cambio de IP sin dispositivo nuevo no genera aviso por correo.
- `Ajustes > Mi cuenta` conserva el acceso al perfil y a las credenciales SMTP personales del usuario autenticado; ningún usuario puede leer o cambiar las credenciales de otra cuenta.
- Solo el Administrador del sistema puede configurar el host, puerto y TLS del servidor SMTP global en `Ajustes > Core > Correo saliente`; cada usuario solo configura su correo y contraseña para autenticarse en ese servidor. El backend rechaza overrides de host/puerto/TLS enviados por usuarios y no devuelve contraseñas SMTP.
- El correo personal es la identidad SMTP AUTH y la dirección remitente; la API valida campos y encabezados de correo, rechaza CR/LF y no acepta comandos SMTP crudos.
- `Mi cuenta` se limita a perfil, seguridad, dispositivos, sesiones, historial de acceso y credenciales SMTP personales; no agrega preferencias de apariencia al Core inicial.
- Empresa/SAR se trata como configuración del Core. Cada módulo habilitado que tenga settings obtiene su propia sección; cada setting declara alcance global, empresa o usuario y los permisos `ver`/`editar` correspondientes.
- Solo el Administrador del sistema puede consultar o cambiar cualquier ajuste del Core, incluidos los operativos y Empresa/SAR; ningún permiso de rol personalizado amplía ese acceso. Los módulos aplican los permisos que declaren para sus propios settings.
- Los settings de módulo aplican `ver`/`editar` por rol, módulo y alcance; los roles nuevos no reciben acceso automáticamente, el Administrador del sistema tiene acceso total y el alcance usuario limita la lectura/escritura al propio usuario.
- Los secretos de settings no se devuelven en respuestas ni se escriben en logs; una lectura puede indicar que ya existe un valor configurado sin revelar su contenido.
- Las contraseñas se almacenan como hashes Argon2id con sal individual y parámetros versionados; se permiten frases largas y la verificación tiene límites de recursos.
- Los tokens de sesión, dispositivo, invitación, recuperación y OAuth se generan aleatoriamente y solo se persisten como hashes; los enlaces de un solo uso no pueden consumirse dos veces ni mediante solicitudes concurrentes.
- Los correos de cuenta declaran MIME 1.0, texto UTF-8 y codificación de transferencia explícita. Al decodificar el mensaje recibido, el enlace y su token deben coincidir exactamente con los generados, incluidos correos con acentos y enlaces largos. Un enlace de invitación con formato alterado muestra una instrucción de reenvío antes de guardar la contraseña; no se intenta reparar ni aceptar tokens distintos.
- Semillas TOTP y credenciales SMTP se almacenan cifradas con cifrado autenticado y claves protegidas por AWS KMS; la rotación de claves no pierde acceso a los valores vigentes.
- Los códigos de recuperación se consumen una sola vez; TOTP rechaza la reutilización de un intervalo ya aceptado; refresh tokens OAuth rotan y la reutilización revoca la familia afectada.
- Los settings de Core y módulos se guardan en una tabla común por módulo, alcance, clave y versión; Rust valida el JSON según el schema y los secretos se cifran antes de persistirlos.
- Con contraseña válida, OTP TOTP se pide si el equipo es nuevo/no reconocido, si la IP es nueva/no reconocida, o si vencieron 15 días de confianza del dispositivo.
- Después de completar TOTP, el Core registra automáticamente el dispositivo por hasta 15 días, informa al usuario y permite consultar/revocar esa confianza en Ajustes.
- Si la sesión permanece abierta por `Mantenerme loggeado`, al cumplirse 15 días se solicita TOTP de nuevo antes de continuar; tras validarlo se renueva la confianza por hasta otros 15 días.
- En equipo e IP reconocidos dentro de esos 15 días, el inicio normal no vuelve a pedir OTP, salvo otros indicadores de riesgo que se definan.
- El TOTP no puede reutilizarse durante su periodo de validez; hay límites de intentos, recuperación segura y revocación de dispositivos/sesiones.
- Tres códigos TOTP incorrectos invalidan el desafío actual y regresan al login; los límites TOTP por cuenta/IP son independientes del bloqueo progresivo de contraseña.
- Entre desafíos TOTP, se permiten hasta 10 fallos por cuenta o 30 por IP en una ventana de 15 minutos; al alcanzar uno de esos límites, se suspenden las verificaciones TOTP del alcance correspondiente durante 15 minutos.
- El desafío TOTP vence 5 minutos después de emitirse y no permite acceder a recursos tras vencer.
- La sesión normal vence a las 24 horas sin actividad. `Mantenerme loggeado` no vence por inactividad ni por plazo absoluto, pero exige TOTP cada 15 días; no amplía la confianza del dispositivo ni evita OTP por equipo/IP nuevos.
- IP asociadas a inicios y eventos de seguridad se conservan como máximo 30 días.
- La bitácora de acciones administrativas y de negocio conserva cada evento durante 1 año; expirar la bitácora no elimina registros ni documentos de negocio.
- El Administrador consulta la bitácora de solo lectura en `Ajustes > Core > Bitácora`, filtrando por fecha, actor, módulo, acción y resultado; no puede editar ni borrar manualmente eventos desde la interfaz.
- El enrolamiento TOTP se recupera con códigos de un solo uso; estos no se revelan de nuevo luego de generarse y se invalidan al utilizarse.
- Si el usuario pierde el autenticador y todos los códigos, el Administrador puede restablecer TOTP tras verificación presencial en la oficina; se registra quién verificó y cuándo, la acción queda auditada y el usuario no accede a módulos hasta completar nuevo enrolamiento y generar códigos.
- El usuario que conserva su TOTP puede cambiarlo desde Mi cuenta tras confirmar contraseña y factor vigente; se verifica el nuevo enrolamiento, se regeneran los códigos de recuperación y se revocan sesiones y dispositivos confiables. No puede desactivar TOTP.
- La activación no permite entrar a módulos hasta completar TOTP y mostrar/guardar los códigos de recuperación de un solo uso.
- El enlace de invitación es de un solo uso y vence a las 24 horas desde un envío SMTP exitoso. Si el primer envío falla, el usuario queda como `Invitación pendiente`, el vencimiento aún no comienza y el administrador puede reintentar.
- El enlace de recuperación de contraseña es de un solo uso, vence a la hora y la solicitud no confirma si la cuenta existe.
- Cambiar o recuperar una contraseña revoca todas las sesiones y dispositivos confiables y exige autenticación completa con contraseña y TOTP.
- Si SMTP falla al enviar una invitación o recuperación, el error es visible para el administrador, no se filtran credenciales, el Core no cambia de cuenta emisora y permite reintentar tras corregir las credenciales del administrador creador o la configuración global.
- Contraseñas aceptan frases sin mezcla obligatoria, requieren al menos 8 caracteres y no vencen periódicamente; solo se cambian por solicitud o alerta de seguridad.
- Tras 3 fallos consecutivos de contraseña se notifica al administrador y se aplica bloqueo progresivo de 30, 60, 120 minutos, duplicando hasta un máximo de 24 horas; el nivel sube por cada grupo de 3 fallos luego de vencer la pausa y se reinicia tras un acceso correcto.
- La notificación de bloqueo llega a todos los Administradores del sistema activos una vez por incidente; se permite otra notificación después de 24 horas sin incidentes.
- Un usuario tiene un rol. Solo el Administrador del sistema puede gestionar usuarios, crear roles y asignar permisos; los roles personalizados empiezan sin permisos.
- No se envía una invitación sin un rol asignado al usuario.
- Un cambio de correo de acceso, iniciado por el usuario o por el Administrador, no sustituye el correo actual hasta que se confirme la dirección nueva mediante el flujo previsto.
- Al cambiar su correo desde Mi cuenta, el usuario debe revalidar contraseña y TOTP antes de iniciar la confirmación de la dirección nueva.
- Al cambiar el correo de otra cuenta, el Administrador revalida su propia contraseña y TOTP; el destinatario confirma la dirección nueva antes de sustituir la anterior.
- Al completarse el cambio de correo, se envía al correo anterior un aviso de seguridad que no contiene enlaces de acción.
- No se crea ni invita un usuario sin vincularlo a una Persona existente; la relación no crea Personas implícitamente y ninguna Persona puede estar vinculada a más de una cuenta. El backend y MySQL hacen cumplir la referencia y su unicidad.
- El Administrador puede reasignar una cuenta a una Persona libre en cualquier estado; el servidor verifica la unicidad atómicamente y audita actor, cuenta, Persona anterior/nueva y fecha.
- Desactivar una cuenta bloquea nuevos accesos y revoca inmediatamente sus sesiones, desafíos preautenticados y dispositivos confiables; al reactivarla se requiere autenticación normal con TOTP según corresponda. Desactivación y reactivación quedan auditadas; la observación administrativa es opcional.
- Desactivar una cuenta no libera automáticamente su Persona; esta permanece reservada hasta una reasignación auditada a una Persona libre.
- La interfaz y API no ofrecen eliminación definitiva de cuentas; desactivarlas conserva sus relaciones e historial de auditoría.
- Desactivar una cuenta con invitación pendiente invalida inmediatamente el enlace; reactivarla requiere enviar una nueva invitación de un solo uso.
- El backend impide eliminar, desactivar o quitar el rol al último Administrador del sistema activo; el rol integrado tampoco puede eliminarse, renombrarse ni perder permisos completos desde la interfaz.
- Administrador del sistema tiene `ver`, `editar` y `eliminar` en Core y todos los módulos habilitados.
- Una cuenta activa con rol personalizado (`roles.system_key` nulo) completa contraseña/TOTP y carga su catálogo de acceso aunque no tenga permisos asignados. No recibe privilegios administrativos implícitos. El administrador puede editar o desactivar esa cuenta con los controles existentes.
- La primera cuenta se crea solo mientras no exista Administrador del sistema, mediante asistente web protegido por código temporal de alta entropía; el asistente queda desactivado tras crearla y obliga al primer administrador a completar TOTP antes de entrar al Core.
- Un Administrador del sistema puede liberar manualmente una pausa de inicio antes de su vencimiento; la acción queda auditada.
- Un desafío preautenticado no puede leer ni modificar recursos de usuario antes de verificar el OTP.
- Las cookies de sesión cumplen flags seguros; los cambios de estado pasan CSRF y validación exacta de origen; `GET` no modifica datos.
- Login, verificación OTP, cierre, recuperación, emisión y revocación de dispositivos quedan auditados sin incluir códigos, secretos o tokens.
- Puede desplegarse y funcionar en servidores sin módulos de negocio registrados.
- Se valida el core en un ambiente de prueba con base separada de producción antes de publicar módulos.
- Fechas/horas visibles y correos programados siguen `America/Tegucigalpa`; marcas de tiempo persistidas y transmitidas usan UTC o ISO 8601 con zona explícita, sin cambiar el día de campos que son solo fecha.
- Añadir/registrar módulo actualiza navegación y Ajustes.
- Cada release de módulo actualiza el backend Rust común e incorpora un módulo a la vez; su habilitación la controla el servidor por ambiente.
- El mismo artefacto puede operar con Core solo o con los módulos habilitados en ese ambiente; un módulo deshabilitado no publica UI, rutas API o herramientas MCP.
- En `Ajustes > Core > Módulos`, solo el Administrador activa/desactiva módulos incluidos en el release, permitidos por el ambiente y con migraciones compatibles aplicadas; la API comprueba esas mismas condiciones y audita los cambios.
- Desactivar un módulo bloquea sus pantallas, rutas API y herramientas MCP sin borrar sus datos.
- Habilitar, probar, deshabilitar o revertir un módulo no borra sus datos ni rompe módulos previamente aceptados.
- Deshabilitar módulo no borra sus datos.
- Un permiso denegado se aplica de igual manera en UI, API y MCP.
- Cada operación de autenticación y dominio aplica límites definidos para intentos, petición/archivo, paginación, rango de fechas, lote, timeout y concurrencia antes de consumir recursos costosos.
- Login, OTP, recuperación y operaciones costosas tienen cuotas por cuenta/actor; el exceso produce un error controlado con reintento cuando corresponda, sin bloqueo automático indefinido inducible por terceros.
- Reportes y exportaciones respetan límites de tiempo y concurrencia y muestran el resultado controlado definido en su contrato cuando alcanzan el tope; los flujos asíncronos, si se acuerdan, protegen también la consulta/descarga con autorización vigente.
- UI, API y MCP verifican en servidor la autorización por operación y por objeto/tenant, junto con la allowlist de campos editables; las herramientas de IA no amplían permisos.
- La IA interactiva se autentica con OAuth Authorization Code + PKCE y usa los permisos vigentes del usuario que autorizó; el usuario puede listar/revocar la conexión desde `Ajustes > Mi cuenta > Aplicaciones conectadas`.
- Solo clientes aprobados previamente por un Administrador del sistema pueden iniciar OAuth; esa aprobación administrativa no otorga acceso a datos. El usuario puede solicitar habilitar un cliente no autorizado, pero la solicitud pendiente no crea conexión, grant ni token; queda en la cola de Ajustes y genera un correo para todos los Administradores activos. Después de la aprobación administrativa, la pantalla muestra el cliente y alcances y requiere aprobación explícita de cada usuario; repite la aprobación si se solicitan alcances adicionales.
- Si el Administrador aprueba una solicitud, el usuario ve Aprobada en Ajustes y recibe correo para completar OAuth y autorizar sus alcances; no se crean grants ni tokens hasta su consentimiento.
- Si el Administrador rechaza una solicitud, el usuario ve Rechazada en Ajustes, recibe correo y puede volver a solicitar más adelante.
- El Administrador configura el máximo de alcances permitido por cliente; OAuth impide consentir alcances fuera de ese límite o de los permisos vigentes del usuario. El usuario puede solicitar ampliar el máximo; si se aprueba, debe consentir los nuevos alcances antes de usarlos. Si se elimina un alcance del máximo, se revocan inmediatamente los grants y tokens afectados y el usuario debe volver a consentir para conectar de nuevo.
- Los access tokens de IA duran 15 minutos; los refresh tokens rotan y vencen como máximo a los 15 días, cuando se exige nueva autorización con TOTP. Cambio/recuperación de contraseña, desactivación, revocación manual o retirada de la aprobación administrativa del cliente invalidan los grants; retirar la aprobación revoca de inmediato todos los grants y tokens de usuario asociados.
- El Core permite registrar clientes MCP compatibles con el perfil definido; cada cliente necesita aprobación administrativa y el usuario conserva consentimiento individual de alcances. CIMD es el mecanismo principal; DCR solo se habilita para integraciones validadas que lo requieran. Las URI de redirección se comparan exactamente. Un cambio de la ficha CIMD aprobada bloquea nuevas conexiones hasta que el Administrador lo apruebe.
- Las mutaciones MCP de alto impacto requieren confirmación explícita del usuario ligada a la acción y argumentos.
- Las sesiones usan cookies con `Secure`, `HttpOnly` y `SameSite` explícito; las operaciones mutables autenticadas por cookie validan token CSRF y origen exacto, y `GET` no modifica datos.
- La cookie de sesión es host-only para `api.bastion.castilloit.net`; CORS acepta credenciales solo desde `https://bastion.castilloit.net`. Clientes externos/IA usan credenciales distintas de la cookie web; la IA interactiva actúa con los permisos del usuario conectado y cada acción se audita bajo esa identidad.
- La conexión AWS↔MySQL solo acepta la IP de salida autorizada, verifica TLS/certificado, usa una cuenta de privilegio mínimo, pool/timeouts acotados y consultas parametrizadas.
- El esquema, las migraciones y la persistencia JSON funcionan en MySQL 5.7, incluida la versión de mantenimiento confirmada para Bluehost; las validaciones de settings se ejecutan en Rust.
- Cambios de settings y operaciones sensibles quedan auditados.
- Las solicitudes para habilitar clientes IA quedan en una cola visible en Ajustes y notifican por correo a todos los Administradores activos mediante el servidor SMTP global y las credenciales SMTP personales del Administrador activo designado en Ajustes; no se configura una credencial transaccional compartida. Si el envío falla, el aviso permanece visible, expone el error y permite reintentarlo.
- La salida SMTP acepta solo host DNS y los puertos aprobados 465 (TLS implícito) o 587 (STARTTLS obligatorio), valida la identidad TLS, rechaza direcciones no públicas/reservadas incluyendo redes internas y metadatos de nube, fija la IP pública validada para cada conexión y aplica restricciones de egreso en AWS.

## Criterio de despliegue

- Los registros web `app` y `api` están proxied por Cloudflare Free; sus reglas WAF/rate limit se verifican desde el exterior.
- Los orígenes AWS y Bluehost usan TLS `Full (strict)` y el acceso directo al API origen queda limitado a Cloudflare si la infraestructura lo permite; cualquier limitación de Bluehost queda registrada.
- Los registros de correo permanecen DNS-only y el tráfico MySQL AWS↔Bluehost no se enruta por Cloudflare.
- Las limitaciones del plan gratuito (una regla de rate limit que usa la ruta como criterio y la IP como contador, inspección de cuerpos hasta 1 MB y eventos muestreados) se compensan con validación/límites en el core y logs propios.
- La regla perimetral se ajusta revisando eventos/falsos positivos y no se considera sustituto de límites por cuenta/actor en el core.
- API/MCP funciona detrás del proxy dentro de sus límites de tiempo/streaming; solicitudes largas se procesan de forma asíncrona o mantienen el stream activo.
- El transporte MCP remoto usa Streamable HTTP sobre HTTPS y la autorización OAuth delegada del usuario.
- MCP opera stateless: no guarda sesión de protocolo; autentica y autoriza cada llamada. Los grants OAuth son persistentes, separados y revocables desde Ajustes.
- Frontend en Bluehost usa solo endpoint público de API y no contiene secretos.
- Backend AWS se conecta a MySQL desde la IP autorizada usando TLS con certificado/identidad verificados.
- MySQL no acepta conexiones generales desde Internet.
- API/MCP y archivos privados requieren identidad/permisos apropiados. PDFs y adjuntos se guardan como objetos privados en AWS; no hay acceso público y el Core autoriza cada lectura/descarga según el objeto.
- PDFs y adjuntos no caducan automáticamente mientras exista su registro asociado; eliminar el registro requiere permiso, confirmación y auditoría, e incluye eliminar los archivos relacionados.
- La integración Typst y los recursos de plantilla/fuentes necesarios están incluidos en el release del backend y los PDFs solo pueden descargarse con autorización para el documento.
- Backup y restauración de datos/archivos han sido ensayados.
- Hay evidencia de la secuencia core-only en prueba y releases probados módulo por módulo, con versiones y migraciones registradas.

## Evidencia requerida

Screenshots de cada pantalla/estado, ejemplos de datos anonimizados, esquema MySQL, ejemplos request/response y trazabilidad entre requisito y prueba. Datos productivos privados no se anexan.

## Pendientes actuales

Esquema SQL completo y límites MySQL para los campos; versión de mantenimiento exacta y capacidades TLS del plan Bluehost; validación de migraciones sobre esa versión; ruta de actualización de MySQL antes de producción; asignaciones de roles personalizados por módulo/área; contratos completos; cálculo fiscal verificado; criterios de aceptación detallados por módulo.

## Clientes: corte inicial

- Buscar/paginar, crear/completar coincidencia por nombre y después RTN, editar siete campos y catálogo paginado autorizado.
- No sustituir RTN existente durante merge; edición explícita admite corregirlo.
- Permisos ver/editar, activación/allowlist/migración, CSRF, idempotencia, versiones y auditoría transaccional verificados.
- Ajustes propios sin campos editables por confirmación del usuario del 2026-10-22; pendiente de futura definición.
- No afirmar migración Legacy, borrado, MCP ni paridad visual sin sus validaciones.

