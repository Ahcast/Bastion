# Seguridad y permisos

## Modelo

El core autentica usuarios y resuelve roles/permisos. Los módulos declaran permisos; todos los adaptadores (UI, API y MCP) consultan al mismo servicio de autorización. La autorización efectiva se aplica en servidor por operación y recurso.

## Controles requeridos

- TLS de navegador a frontend/API/MCP y TLS verificado entre backend AWS y MySQL remoto. Si Bluehost no permite verificar la identidad del certificado, la arquitectura debe resolverse antes de producción.
- Credenciales fuera del frontend y repositorio; rotación y acceso mínimo.
- Cuenta MySQL dedicada a la aplicación, solo desde IP de salida AWS permitida, sin permisos de administración.
- Limitar intentos de contraseña y OTP por cuenta, IP y desafío; usar expiración, consumo único y controles contra automatización. Para contraseña: después de 3 fallos consecutivos, aplicar pausas de 30, 60, 120 minutos y duplicar hasta un máximo de 24 horas; cada grupo de 3 fallos después de terminar la pausa sube el nivel, mientras un acceso correcto reinicia el contador. No revelar si falló el usuario, contraseña o estado de cuenta de forma que facilite enumeración.
- Rotar identificador de sesión tras completar autenticación y tras cambios de privilegio; expirar e invalidar sesiones en el servidor al cerrar sesión, revocarlas o vencer.
- La sesión del navegador usa cookie host-only de `api.bastion.castilloit.net`, con `Secure`, `HttpOnly` y `SameSite` explícito. En cada cambio de estado validar token CSRF impredecible y `Origin` exacto (`Referer` exacto como respaldo documentado). No depender solo de `SameSite` ni de CORS; ningún `GET` modifica datos.
- CORS permite solo el origen exacto `https://bastion.castilloit.net` y credenciales para ese origen. No almacenar sesión, refresh token ni OTP en `localStorage` o `sessionStorage`. Los clientes externos/IA usan credenciales distintas de la cookie web. Para IA interactiva, las acciones se delegan al usuario conectado y conservan sus permisos y auditoría; las cuentas de servicio se reservan para automatizaciones futuras con autorización propia y mínima.
- Proteger también login, desafío/verificación OTP, recuperación y logout contra CSRF, replay, fijación de sesión, abuso de rate y enumeración.
- Validar/normalizar entradas en backend y usar consultas parametrizadas.
- Validar toda entrada en servidor, también cuando ya fue validada en la interfaz: allowlist de campos por operación; tipos, formatos, longitudes y límites; enumeraciones/catálogos; referencias y relaciones; reglas cruzadas; y unicidad normalizada en alta y edición, respaldada por restricciones de base de datos. Rechazar asignación masiva y entradas inesperadas. La validación no sustituye a la autorización por acción, objeto y tenant.
- Aplicar autorización a nivel de objeto/recurso en servidor (BOLA/IDOR), escapar salida y sanitizar contenido para prevenir XSS; política CSP y cabeceras seguras como defensa adicional.
- Revisar CORS, SSRF, traversal, carga/descarga de archivos, secretos, dependencias y límites de API; no confiar en campos ni cabeceras proporcionados por el cliente.
- El destino SMTP es una configuración global que solo puede cambiar el Administrador del sistema en `Ajustes > Core > Correo saliente`. Las cuentas de usuario guardan únicamente su correo y contraseña de autenticación; la API rechaza cualquier host, puerto o modo TLS enviado por el usuario. Así se elimina el endpoint arbitrario controlado por usuarios como vector SSRF, pero se conservan controles frente a errores o compromiso administrativo.
- El correo de `Mi cuenta` se usa como identidad SMTP AUTH y dirección `From`; no se acepta un remitente/host alternativo enviado por la aplicación cliente. Validar los campos de correo y encabezados, rechazar CR/LF y construir el sobre SMTP solo a partir de destinatarios permitidos por la operación, sin aceptar comandos SMTP crudos.
- Para la conexión SMTP, aceptar un nombre DNS de host (no URL ni IP literal), usar solo puertos de envío aprobados (465 con TLS implícito o 587 con STARTTLS obligatorio) y validar certificados/hostname. Resolver DNS al conectar; si alguna respuesta apunta a loopback, redes privadas, link-local, multicast, reservadas, no especificadas o metadatos de nube, rechazar la resolución completa. Conectar a una IP pública validada y fijada para esa operación, sin volver a resolver durante la conexión, para impedir DNS rebinding. Restringir la regla de egreso SMTP de AWS a TCP 465/587 y, cuando el proveedor use rangos estables, también a sus CIDR publicados. No permitir downgrade a texto plano.
- Archivos privados y PDFs generados: control por objeto en cada descarga, límites de tamaño/formato, nombre interno aleatorio, almacenamiento privado de objetos en AWS, antivirus cuando corresponda y borrado/auditoría. No caducan automáticamente mientras exista el registro asociado; el borrado vinculado requiere permiso, confirmación y auditoría. La generación Typst aplica los límites de tiempo/concurrencia del core y no acepta plantillas o rutas arbitrarias desde API/MCP.
- El correo SMTP y su contraseña de autenticación se cifran y redactan en lecturas/respuestas/logs; la contraseña nunca se devuelve al cliente. Host, puerto y TLS son settings globales editables solo por el Administrador del sistema.
- Registro de auditoría para pagos, conversiones, borrados, cambios de permisos, settings e importaciones.
- Conservar las entradas de la bitácora de acciones administrativas/de negocio durante 1 año desde cada evento; eliminar las entradas al vencer, protegerlas contra lectura o modificación no autorizada y excluir contraseñas, tokens, secretos y contenido privado.
- Mantener por separado el historial de acceso/IP durante 30 días; esta expiración no elimina los registros de negocio originales.
- La consulta global de auditoría se ofrece al Administrador del sistema en `Ajustes > Core > Bitácora`, con filtros y sin operaciones de edición o borrado manual desde la interfaz.
- Registrar eventos relevantes de seguridad (login exitoso/fallido, OTP requerido/validado/fallido, enrolamiento/recuperación TOTP, emisión/revocación de dispositivo y revocación de sesiones), sin registrar códigos OTP, semillas TOTP, cookies, tokens ni credenciales.
- Backups protegidos, retención definida y restauraciones ensayadas.

### Almacenamiento de credenciales y tokens

- Las contraseñas tienen mínimo 8 caracteres, permiten frases y no tienen reglas de composición ni vencimiento periódico. Aceptar al menos 64 caracteres, espacios y Unicode; rechazar contraseñas comunes o conocidas como comprometidas. Guardarlas solo como hashes Argon2id con sal aleatoria por contraseña y parámetros incluidos en formato PHC; ajustar el costo en AWS y limitar concurrencia de verificación para evitar agotamiento de recursos. No cifrar contraseñas ni ofrecer su recuperación en texto.
- Los tokens de sesión, confianza de dispositivo, invitación, restablecimiento y OAuth son valores aleatorios opacos de al menos 256 bits. Guardar solo su hash y los metadatos de cuenta, propósito/cliente, vencimiento, consumo y revocación necesarios. Los enlaces de un solo uso se consumen atómicamente para impedir reutilización concurrente.
- Guardar cada código de recuperación TOTP como hash individual y marcarlo como consumido de forma atómica. Para evitar replay de TOTP, persistir el último intervalo temporal aceptado y actualizarlo atómicamente al verificar.
- Las semillas TOTP y contraseñas SMTP de cada usuario deben recuperarse para su uso, por lo que se cifran con cifrado autenticado mediante cifrado de sobre: clave de datos protegida por AWS KMS; persistir ciphertext, nonce/etiqueta de autenticación y versión/identificador de clave. Mantener claves fuera de MySQL, separar su acceso, permitir rotación y nunca incluir texto claro en respuestas, logs o errores. El host/puerto/TLS globales no contienen credenciales de usuario.
- Los refresh tokens OAuth se guardan como hashes, se rotan en cada uso y detectan la reutilización de un token ya consumido; ante replay se revoca la familia/grant afectada. Los access tokens son opacos, de vida corta, vinculados al usuario, cliente, recursos/audiencia y alcances; cada llamada también comprueba permisos vigentes.

## OTP, dispositivos confiables y sesiones

### Política confirmada

- Usar OTP de tipo TOTP mediante una aplicación autenticadora.
- Tras validar la contraseña, pedir TOTP si el equipo/dispositivo no se reconoce, si la IP es nueva/no reconocida o si vencieron los 15 días de confianza del dispositivo. La condición es **equipo nuevo o IP nueva**; no se requiere que ambas cambien a la vez.
- Al llegar a 15 días desde la última verificación TOTP, volver a pedir TOTP incluso si `Mantenerme loggeado` conserva abierta la sesión. Después de verificar, renovar la confianza por hasta 15 días.
- Registrar la IP asociada a inicios de sesión y eventos de seguridad. La IP es una señal de riesgo y un dato de auditoría, no un factor de autenticación por sí mismo.
- Conservar las IP asociadas a autenticación y eventos de seguridad por 30 días; restringir su acceso y eliminarlas al vencer el plazo.
- Permitir al usuario ver los eventos de acceso propios de los últimos 30 días, con fecha, IP, dispositivo y resultado; excluir secretos y datos de otras cuentas.
- Tras validar con éxito TOTP desde un dispositivo nuevo, notificar a la dirección registrada; no enviar aviso por un cambio aislado de IP. La entrega es de mejor esfuerzo y no bloquea la autenticación; registrar el fallo sin revelar credenciales.
- Permitir al usuario listar y revocar individualmente sus propios dispositivos confiables desde `Ajustes > Mi cuenta`.
- En `Ajustes > Mi cuenta > Sesiones`, permitir listar dispositivo/navegador, IP, fecha de inicio, última actividad e indicar la sesión actual; permitir revocar la sesión actual. Cada revocación invalida el token en servidor. Solo existe una sesión activa por usuario.
- Limitar a una sesión activa por usuario. Un nuevo inicio solo revoca la anterior después de validar contraseña y el TOTP requerido; mostrar aviso en la sesión anterior al siguiente intento de uso.
- Permitir **Mantenerme loggeado**. Esta opción no extiende el plazo de confianza del dispositivo y no evita nuevos desafíos OTP.

### Diseño recomendado para la confianza del dispositivo

Usar un identificador aleatorio opaco emitido por el servidor para ese navegador, en vez de basar la confianza únicamente en una huella probabilística de atributos del navegador (por ejemplo, canvas, tamaño de pantalla o user-agent). El navegador presenta ese identificador en una cookie protegida; el servidor guarda solo su hash, la cuenta asociada, fecha de creación/último uso, vencimiento, metadatos mínimos para reconocerlo y el estado de revocación. Tras validar TOTP, el dispositivo se registra automáticamente por hasta 15 días; se informa al usuario y se puede revocar desde `Ajustes > Mi cuenta > Dispositivos confiables`.

La cookie de dispositivo y la cookie de sesión son credenciales y requieren HTTPS, `Secure`, `HttpOnly`, `SameSite` explícito, alcance host-only y protección contra CSRF. Cualquier cambio o recuperación de contraseña revoca todas las sesiones y dispositivos confiables; el usuario debe volver a autenticarse con contraseña y TOTP. El cambio/recuperación de TOTP, actividad sospechosa y cierre global también revocan los dispositivos confiables según la política del core. Después de TOTP válido, el servidor registra automáticamente el dispositivo y notifica al usuario; puede revisarlo y revocarlo desde Ajustes.

La IP se obtiene del peer de conexión o de cabeceras de proxy únicamente cuando procedan de proxies confiables configurados; ignorar `X-Forwarded-For` arbitrario enviado directamente por el cliente. Debido a IP dinámicas, VPN y redes compartidas, comparar la dirección con criterio explícito, restringir su acceso y conservarla por 30 días.

La implementación acepta únicamente `X-Forwarded-For` desde los CIDR de `BASTION_TRUSTED_PROXY_CIDRS`. Recorre la cadena de derecha a izquierda, omite solo proxies explícitamente confiables y usa el primer salto no confiable; ignora `CF-Connecting-IP` y otras cabeceras alternativas. Rechaza cabeceras duplicadas, valores no IP, más de 16 saltos o más de 1024 bytes y nunca inventa una IP si falta el peer de conexión. En el ingreso directo con Caddy, sobrescribir la cabecera con la IP de su conexión y mantener el backend en loopback, como en `deploy/Caddyfile.example`. Si se agrega otro proxy delante de Caddy, configurar y verificar primero su cadena de confianza; no confiar en rangos generales de clientes. La IP registrada es la salida pública observada (incluida una VPN o NAT), no la dirección privada del teléfono o computadora.

### TOTP y recuperación

- El enrolamiento debe requerir autenticación reciente, mostrar el secreto de enrolamiento una sola vez y confirmarlo con un TOTP válido antes de activarlo. Para cambiar el factor desde Mi cuenta, exigir contraseña y TOTP vigente, verificar el nuevo enrolamiento, invalidar los códigos de recuperación anteriores y emitir nuevos. TOTP no se puede desactivar.
- Guardar la semilla TOTP cifrada con clave administrada fuera de la base. La verificación la requiere en su forma original, por lo que no puede almacenarse solo como hash. Separar claves, limitar acceso y excluir la semilla de logs/respuestas normales.
- Aceptar un TOTP una sola vez dentro de su ventana válida; aplicar ventana de tiempo acotada, límites de intentos y respuesta uniforme. Tras 3 códigos incorrectos, invalidar el desafío y volver al login. Contabilizar también entre desafíos: máximo 10 fallos por cuenta o 30 por IP en una ventana de 15 minutos; al alcanzarse cualquiera, suspender verificaciones TOTP del alcance que alcanzó el límite durante 15 minutos. Estos límites son independientes del bloqueo progresivo de contraseña. Nunca registrar el código. Notificar cambios de factor.
- Proveer recuperación con códigos de un solo uso almacenados en forma protegida; no usar preguntas de seguridad ni una vía de recuperación más débil que el acceso habitual.
- Si se pierden el autenticador y todos los códigos, un Administrador puede iniciar un restablecimiento de TOTP solo después de verificar la identidad del usuario presencialmente en la oficina; registrar quién verificó y cuándo, sin retener datos sensibles innecesarios. Auditar el restablecimiento, revocar sesiones/dispositivos y obligar a completar el nuevo enrolamiento y generar nuevos códigos antes de permitir acceso a módulos. Cualquier cambio o restablecimiento de TOTP revoca todas las sesiones y dispositivos confiables; el usuario debe autenticarse otra vez y completar TOTP antes de acceder.
- TOTP es requisito de autenticación para todos los usuarios. El enrolamiento y entrega de códigos de recuperación de un solo uso son obligatorios durante la activación de la cuenta, antes de permitir acceso a los módulos.

TOTP reduce ataques basados solo en contraseña, pero el código introducido manualmente puede ser robado por phishing. Una opción futura es passkey/WebAuthn, que es resistente al phishing; no sustituye el requisito TOTP confirmado en esta etapa.

### Sesión persistente y “Mantenerme loggeado”

Separar cuatro estados: desafío preautenticado, sesión autenticada, confianza del dispositivo/IP de hasta 15 días y reautenticación de una sesión vigente. El desafío preautenticado solo permite verificar el OTP y vence a los 5 minutos. La sesión normal expira tras 24 horas de inactividad. Si `Mantenerme loggeado` se selecciona, emitir credencial persistente rotatoria/revocable sin vencimiento por inactividad ni límite absoluto; exigir TOTP cada 15 días y revocar al cerrar sesión o por una acción de seguridad. Permitir solo una sesión activa por usuario: al comenzar un nuevo acceso, conservar la sesión anterior hasta completar contraseña y el TOTP requerido; luego reemplazarla de forma atómica y avisar a la sesión revocada al siguiente intento de uso. Al cumplirse 15 días, exigir TOTP aunque la sesión siga abierta; tras validarlo, rotar la sesión, actualizar el registro de dispositivo/IP y continuar. `Mantenerme loggeado` no renueva por sí solo la confianza ni anula OTP requerido por equipo/IP nuevos.

### Alta y recuperación de cuentas

- Un cambio del correo de acceso queda pendiente hasta confirmar la nueva dirección con un enlace de un solo uso; mientras tanto, la dirección actual sigue siendo la de acceso y recuperación. Si el cambio lo inicia el usuario desde Mi cuenta, exigir revalidación de contraseña y TOTP antes de solicitar la confirmación nueva. Si lo inicia el Administrador, exigir revalidación de la contraseña y TOTP del Administrador además del permiso requerido; el usuario destinatario debe confirmar el correo nuevo. Tras confirmarlo, enviar al correo anterior un aviso de seguridad sin enlaces de acción.
- El Administrador del sistema crea usuarios mediante invitación a su correo registrado; el enlace es de un solo uso, vence a las 24 horas desde el envío exitoso y permite activar credenciales seguras. Si SMTP falla, conservar el usuario como `Invitación pendiente` y permitir reintentar; no iniciar la expiración hasta lograr enviar. No se envían contraseñas temporales.
- La solicitud de restablecimiento desde el login responde de forma genérica y envía un enlace de un solo uso al correo registrado, con vencimiento de una hora. Tras completar el cambio, revocar todas las sesiones y dispositivos confiables.
- El correo de invitación y recuperación sale por el servidor SMTP global configurado por el Administrador, autenticándose con el correo/contraseña SMTP del administrador que creó la cuenta; no hay credencial transaccional compartida ni fallback a otro SMTP. Si falla el envío, mostrar el error al administrador y permitir reintento tras corregir sus credenciales o la configuración global. Si falla la invitación, conservar la cuenta como `Invitación pendiente` y empezar las 24 horas solo después del envío exitoso.
- El aviso de acceso desde un dispositivo nuevo se envía al correo registrado mediante las credenciales SMTP personales de ese usuario. Si faltan credenciales SMTP o el envío falla, registrar el error sin bloquear el acceso válido ni exponer secretos.
- Contraseña con mínimo 8 caracteres, sin reglas obligatorias de composición; no vence periódicamente y se cambia a petición del usuario o ante alerta de compromiso.
- Las notificaciones de fallos de inicio deben limitarse para evitar spam y enviarse solo a administradores autorizados.
- Una vez activado el primer bloqueo de un incidente, enviar una alerta a todos los usuarios activos con rol `Administrador del sistema`. No repetirla durante ese incidente; se permite una nueva alerta tras 24 horas sin incidentes.
- Un Administrador del sistema puede liberar manualmente una pausa de inicio antes de que venza; auditar actor, cuenta, fecha y motivo, sin registrar secretos.
- Mantener siempre al menos un Administrador del sistema activo; el backend rechaza eliminarlo, desactivarlo o quitarle el rol al último.
- Al desactivar un usuario, impedir nuevos accesos y revocar inmediatamente todas sus sesiones, desafíos preautenticados y dispositivos confiables; registrar la acción en auditoría. La observación del administrador es opcional. Al reactivarlo, exigir inicio de sesión y TOTP conforme a las reglas de confianza vigentes; auditar también la reactivación y su observación opcional.
- Si la cuenta desactivada tiene una invitación pendiente, invalidar inmediatamente el enlace; la reactivación requiere emitir una invitación nueva de un solo uso.
- Las cuentas de usuario no se eliminan definitivamente desde la aplicación; se desactivan y conservan sus relaciones e historial de auditoría.
- El asistente de primera configuración solo opera mientras no exista un Administrador del sistema. Usa un código temporal de alta entropía inyectado desde configuración protegida del backend, lo limita por intentos y se desactiva atómicamente al crear la primera cuenta; el primer administrador debe completar TOTP antes de usar el Core.

## Revisión de vulnerabilidades

Antes de producción, hacer revisión de amenazas y verificación de controles sobre login/OTP, sesiones, CSRF/XSS, permisos por objeto, API/MCP, archivos, importaciones, conexiones a MySQL y despliegue. OWASP recomienda tokens CSRF en aplicaciones con estado y capas adicionales como `SameSite` y validación de origen; NIST especifica que TOTP tenga duración definida y el OTP no se acepte más de una vez mientras sea válido.

## Lineamientos de seguridad del core: prioridad alta

Estas cinco medidas quedan acordadas como requisitos del core. Se aplican en los servicios comunes y en cada operación de módulo. Los valores numéricos concretos se fijarán por tipo de operación antes de habilitarla y se ajustarán en el ambiente de prueba para conservar los flujos normales.

| Prioridad | Lineamiento del core | Regla que debe cumplir la aplicación |
|---|---|---|
| Alta | Límites por operación | Limitar intentos de login, OTP y recuperación; tamaño de peticiones y archivos; filas por página; rangos de fechas; lotes; tiempo de ejecución; y concurrencia de reportes/exportaciones. Validar antes de reservar recursos o consultar la base. Cada operación define una respuesta controlada cuando exceda sus topes; un flujo asíncrono solo se agrega si se especifica y conserva los permisos de consulta/descarga. |
| Alta | Límites en el borde y en la aplicación | Poner `app` y `api` detrás de Cloudflare Free por presupuesto. Usar la regla de rate limit disponible para rutas sensibles, revisar eventos y falsos positivos antes de aplicar una acción que pueda afectar usuarios. La regla de Cloudflare cuenta por IP/ruta; el backend aplica límites separados por cuenta/actor. Ninguna capa sustituye a la otra. |
| Alta | Permisos por operación y objeto | El servidor autoriza cada lectura o cambio para UI, API y MCP, incluyendo acceso al registro concreto, empresa/tenant y campos editables. Denegar por defecto. Las herramientas de IA reciben solo permisos, objetos y campos concedidos al usuario/conexión; las acciones sensibles requieren confirmación explícita. |
| Alta | Conexión AWS–MySQL Bluehost | Autorizar únicamente la IP fija de salida de AWS; verificar TLS y certificado; usar una cuenta exclusiva de privilegio mínimo; limitar el pool; fijar timeouts de conexión/consulta; parametrizar consultas; y medir índices. MySQL no se abre a redes generales. |
| Alta | Sesiones y CSRF | Las cookies de sesión usan `Secure`, `HttpOnly` y `SameSite` explícito, con alcance host-only. Para cambios de estado, validar token CSRF y `Origin` exacto; validar `Referer` exacto como respaldo si corresponde. `SameSite` y CORS no sustituyen estas verificaciones; `GET` no modifica datos. |

Cloudflare Free es la elección de borde acordada por no haber presupuesto para AWS WAF. La regla disponible no permite cuotas por cuenta y puede tardar en surtir efecto, por lo que los topes por actor, cuerpo y operación siempre se aplican también en el backend. Si el plan permite una acción de observación para una regla concreta, usarla durante el ajuste; no asumir que existe un modo `Count` equivalente al de AWS WAF.

Las siguientes medidas siguen como mejoras de prioridad media, sujetas a concretar por etapas:

| Prioridad | Medida | Impacto y costo esperado |
|---|---|---|
| Media | Métricas y auditoría compactas: resultado de autenticación, actor seudónimo, ruta, módulo, duración, código HTTP y correlation ID. Redactar secretos y limitar retención/acceso. | Sin fricción; reduce volumen al no registrar cuerpos ni datos completos. |
| Media | En CI, revisar dependencias, secretos y errores estáticos; fijar versiones de artefactos y mantener proceso de actualización. | Sin impacto durante el uso; concentra revisión en el pipeline. |
| Media | Para adjuntos privados, validar formato y contenido en backend, usar nombre interno y ejecutar análisis antimalware fuera de la ruta síncrona cuando corresponda. | Puede añadir estado de procesamiento a archivos grandes; evita consumir CPU de la ruta web. |

### Medidas que no conviene activar por defecto

- No ejecutar CAPTCHA en cada acceso; reservar desafíos adicionales para señales claras de abuso.
- No usar fingerprinting invasivo de navegador ni recolección continua de comportamiento como factor de autenticación. Mantener el token de dispositivo protegido acordado y usar los datos de IP/equipo solo para riesgo y auditoría.
- No ejecutar un modelo de IA, escaneo completo o consulta costosa en cada petición. Primero aplicar reglas, límites y métricas simples; escalar análisis solo ante eventos relevantes.
- No usar la base MySQL como contador global de cada petición ni bloquear una cuenta indefinidamente por fallos repetidos; ambas estrategias pueden crear carga o permitir denegación de servicio.

Estas medidas se afinan con pruebas de carga y observación del ambiente de prueba, usando límites que protejan AWS y MySQL Bluehost sin cortar los flujos normales.

### Límites de Cloudflare Free

- Requiere que `app` y `api` estén proxied; el WAF solo filtra el tráfico que pasa por ese proxy.
- La oferta actual incluye Free Managed Ruleset, reglas personalizadas y una regla de rate limit. En esta regla, los criterios disponibles incluyen la ruta y el contador se basa en IP; el control por cuenta se mantiene en el core.
- Los campos de inspección del cuerpo de reglas WAF tienen tope de 1 MB en Free; el cuerpo completo todavía puede llegar al origen, así que el backend aplica los propios topes.
- La protección del origen debe restringir accesos directos donde el hosting lo permita. Confirmar especialmente las capacidades de Bluehost; DNS proxied oculta la IP en respuestas DNS actuales, pero no basta si el origen acepta llamadas directas.
- Mantener correo y MySQL fuera del proxy HTTP de Cloudflare. Probar API/MCP contra los límites de timeout y el transporte elegido.

El alcance y los límites del plan se registran según la documentación del proveedor, pero pueden cambiar; volver a verificarlos al configurar el dominio.

## Permisos iniciales por área

La matriz acordada para Core y módulos es:

- `ver`: consultar/listar recursos autorizados.
- `editar`: crear, actualizar y ejecutar comandos de dominio no destructivos que cambian datos o estado. No permite eliminar ni purgar.
- `eliminar`: eliminar o purgar; exige además las confirmaciones y auditoría del caso de uso.

El backend valida estos permisos por acción y por objeto. UI, API y MCP usan la misma autorización y deniegan por defecto. El mapeo del Legacy es `consultar` → `ver`, `insertar`/`modificar` → `editar`, `eliminar` → `eliminar`. Los roles no se pueden asumir por nombre: las asignaciones se cargan desde la configuración explícita.

El rol integrado `Administrador del sistema` tiene los tres permisos en Core y todos los módulos habilitados. Los roles personalizados se crean sin permisos y solo obtienen los que el Administrador asigne explícitamente.

El rol integrado no puede eliminarse, renombrarse ni perder sus permisos completos desde la interfaz. El backend protege la existencia de al menos una cuenta activa con ese rol.

En Roles y Pantallas, el servidor valida cada campo, tipo permitido, referencia, unicidad normalizada y relación antes de guardar. Los límites de longitud se declaran en el contrato y deben concordar con el esquema MySQL; las restricciones únicas también se imponen en la base para evitar duplicados por concurrencia.

## IA/MCP

Aplicar permisos del usuario/conexión, alcance de recursos, límites de datos y auditoría. Nunca confiar en que el modelo mantenga instrucciones; validar cada solicitud. Requerir confirmación para acciones sensibles y registrar el resultado.

La IA interactiva usa OAuth Authorization Code + PKCE y actúa en nombre del usuario que autorizó la conexión. Solo los clientes previamente aprobados por un Administrador del sistema pueden iniciar OAuth; el Administrador fija el máximo de alcances del cliente y la aprobación no concede acceso a datos. Si cambia la ficha CIMD aprobada, bloquear nuevas conexiones hasta revisión y aprobación administrativa. Cada usuario solo puede autorizar alcances dentro de ese máximo y sus permisos vigentes. El usuario puede solicitar que el Administrador habilite un cliente no autorizado o amplíe el máximo de alcances permitido a un cliente aprobado; mientras se resuelve, la solicitud no inicia OAuth ni concede grants o tokens; se muestra en la cola de Ajustes y se notifica por correo a todos los Administradores activos. Después de la aprobación administrativa, se muestra Aprobada en Ajustes y se envía correo al solicitante para completar OAuth y autorizar sus propios alcances, incluidos los nuevos aprobados; la aprobación administrativa no crea grants ni tokens. Si el Administrador rechaza la solicitud, el estado Rechazada se muestra al usuario en Ajustes, se envía aviso por correo y puede volver a solicitarla más adelante. Mostrar nombre del cliente y alcances solicitados para que cada usuario apruebe explícitamente su conexión; cualquier ampliación requiere nueva aprobación. Cada llamada vuelve a verificar sus permisos vigentes; el token no amplía el rol ni comparte la cookie/contraseña web. Los access tokens duran 15 minutos; los refresh tokens se rotan, quedan ligados a usuario/cliente/alcances y vencen como máximo a los 15 días, cuando se vuelve a exigir autorización con TOTP. Revocar la aplicación, desactivar la cuenta, cambiar/recuperar la contraseña o retirar la aprobación administrativa del cliente invalida el grant y sus tokens; en este último caso se revocan de inmediato todas las conexiones de usuarios a ese cliente. Al quitar alcances del máximo del cliente, invalidar de inmediato solo los grants y tokens que incluyan esos alcances; para reconectar, el usuario debe volver a consentir dentro del límite vigente. El usuario consulta y revoca cada aplicación conectada desde `Ajustes > Mi cuenta`. Los avisos administrativos del Core usan el servidor SMTP global configurado por el Administrador, autenticándose con la cuenta SMTP personal de un Administrador activo designado en Ajustes, sin credencial transaccional separada; ante fallo, el aviso queda registrado en Ajustes, muestra el error y puede reintentarse. Las credenciales de servicio no se usan para interacción en el primer release.

## Revisión pendiente

La auditoría del informe fue estática. Hacer revisión de amenazas y verificación dinámica antes de producción; confirmar soporte TLS de MySQL Bluehost, conexiones remotas, IPs y políticas de hosting.

## Referencias para SMTP y SSRF

- [OWASP SSRF Prevention Cheat Sheet](https://cheatsheetseries.owasp.org/cheatsheets/Server_Side_Request_Forgery_Prevention_Cheat_Sheet.html): validar todas las respuestas DNS, conectar a la IP verificada y proteger contra DNS rebinding.
- [AWS VPC Security Group Rules](https://docs.aws.amazon.com/vpc/latest/userguide/security-group-rules.html): reglas de egreso con destino, protocolo y rango de puertos.
- [RFC 8314](https://www.rfc-editor.org/rfc/rfc8314.html): TLS implícito para SMTP submission en 465 y STARTTLS en 587; el cliente debe validar el certificado y exigir TLS.

## Fuentes técnicas consultadas

- [OWASP Multifactor Authentication Cheat Sheet](https://cheatsheetseries.owasp.org/cheatsheets/Multifactor_Authentication_Cheat_Sheet.html)
- [OWASP Authentication Cheat Sheet](https://cheatsheetseries.owasp.org/cheatsheets/Authentication_Cheat_Sheet.html)
- [OWASP Session Management Cheat Sheet](https://cheatsheetseries.owasp.org/cheatsheets/Session_Management_Cheat_Sheet.html)
- [OWASP Cross-Site Request Forgery Prevention Cheat Sheet](https://cheatsheetseries.owasp.org/cheatsheets/Cross-Site_Request_Forgery_Prevention_Cheat_Sheet.html)
- [OWASP REST Security Cheat Sheet](https://cheatsheetseries.owasp.org/cheatsheets/REST_Security_Cheat_Sheet.html)
- [OWASP API4:2023 Unrestricted Resource Consumption](https://api-security.owasp.org/editions/2023/en/0xa4-unrestricted-resource-consumption/)
- [OWASP SQL Injection Prevention Cheat Sheet](https://cheatsheetseries.owasp.org/cheatsheets/SQL_Injection_Prevention_Cheat_Sheet)
- [OWASP Logging Cheat Sheet](https://cheatsheetseries.owasp.org/cheatsheets/Logging_Cheat_Sheet.html)
- [Cloudflare WAF plan availability](https://developers.cloudflare.com/waf/)
- [Cloudflare rate limiting rules](https://developers.cloudflare.com/waf/rate-limiting-rules/)
- [Cloudflare origin protection](https://developers.cloudflare.com/fundamentals/security/protect-your-origin-server/)
- [NIST SP 800-63B-4, Authenticator Requirements](https://pages.nist.gov/800-63-4/sp800-63b/authenticators/)
- [NIST SP 800-63B-4, Authenticator Event Management](https://pages.nist.gov/800-63-4/sp800-63b/events/)
