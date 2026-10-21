# Core y ajustes

## Propósito del core

El core contiene las capacidades transversales de la aplicación. Mantiene sus funciones actuales y proporciona un entorno consistente donde los módulos se registran y se conectan.

## Capacidades del core

- Inicio/cierre de sesión, perfil, gestión de usuarios y sesión.
- Roles, permisos y acceso por módulo/acción.
- Navegación y registro/estado de módulos.
- Dashboard y servicios compartidos que se confirmen para la nueva versión.
- Servicio compartido para generar PDFs con Typst; cada módulo aporta los datos y documentos de su dominio.
- Personas, configuración de empresa y configuración de correo identificadas en la aplicación actual.
- Área central de Ajustes.
- Ajustes personales del usuario autenticado, separados de las opciones administrativas globales.
- Infraestructura común para API, MCP, auditoría y configuración.

La delimitación funcional del Core y sus pantallas parte del inventario de `01-aplicacion-actual-y-paridad.md` y de las decisiones de este documento; no depende de disponer de una sesión activa de la aplicación Legacy.

## Inventario funcional del Core heredado

Este inventario toma los campos y flujos del informe estático del Legacy (`01-aplicacion-actual-y-paridad.md`) y los contrasta con el frontend y backend incluidos en la copia. El HTML de Bastion guía el estilo visual. Las decisiones de la columna final definen la nueva aplicación y prevalecen sobre las diferencias o carencias del Legacy.

| Pantalla/área | Campos, botones y funciones | Regla acordada o por completar |
|---|---|---|
| Inicio de sesión y navegación | Correo y contraseña; iniciar sesión; menú de módulos permitidos; Mi perfil; cerrar sesión. | La nueva autenticación agrega TOTP, dispositivo/IP confiables por 15 días y reautenticación al vencer esa confianza, según la política aprobada. |
| Dashboard | Rango Mes (predeterminado), Año o Todo; Actualizar. Indicadores de facturas, total facturado, pagado, saldo pendiente e ISV; facturas y órdenes por estado; resumen financiero de órdenes; listas recientes de órdenes/facturas y resumen de procesos. Enlace a los listados y reportes correspondientes. | El frontend muestra mensajes de carga parcial y de error total. |
| Mi Perfil | Consultar y guardar correo, nombre completo, DNI, género y país; contraseña y confirmación opcionales para cambiarla. Contraseña nueva: mínimo 8 caracteres; la confirmación debe coincidir. | Estos requisitos corresponden al perfil propio. No se deben confundir con la regla de Personas, donde solo el nombre es obligatorio. Para cambiar su correo de acceso, el usuario valida de nuevo contraseña y TOTP; el correo nuevo se usa para acceso solo después de confirmarlo, hasta entonces permanece vigente el actual. |
| Usuarios | Buscar por correo y listar 15 por página; invitar por correo con Persona y rol asignados; editar correo, Persona, rol o estado con observación opcional; recuperar acceso mediante enlace de un solo uso. Contraseña: mínimo 8 caracteres, sin reglas obligatorias de composición ni vencimiento periódico. | Solo el Administrador del sistema administra usuarios y roles. Cada usuario debe vincularse a una Persona existente. Para cambiar el correo de otra cuenta, el Administrador revalida su contraseña y TOTP, y el usuario destinatario confirma la dirección nueva antes de sustituir la actual. La Persona permanece vinculada mientras la cuenta esté desactivada; puede cambiarse en cualquier estado solo por otra que no esté vinculada a una cuenta, y cada reasignación queda auditada. Las cuentas no se eliminan definitivamente: se desactivan y se conserva el historial. Al desactivar una cuenta, se bloquean nuevos accesos y se revocan de inmediato su sesión y dispositivos confiables. Si hay una invitación pendiente, se invalida; al reactivar la cuenta debe enviarse una nueva invitación. La invitación vence a las 24 horas desde el envío exitoso y usa el servidor SMTP global con las credenciales del administrador que la creó. |
| Personas | Crear, listar y editar nombre completo, DNI, país y género. | **Solo el nombre es obligatorio.** Los demás campos son opcionales; si se envían, se validan en backend. Los valores permitidos de país/género y su representación se alinean con los catálogos/esquema, sin hacerlos obligatorios. DNI debe ser único cuando se proporcione. |
| Roles | Crear, listar, consultar y editar nombre y descripción. | El backend valida todos los campos y relaciones en alta y edición; aplica nombre normalizado no vacío y unicidad también al editar. Longitudes y restricciones se fijan explícitamente contra el esquema MySQL antes de cerrar el contrato. |
| Bitácora de auditoría | Consultar eventos con fecha/hora, actor, módulo, acción, registro afectado y resultado; filtros por rango de fechas, usuario, módulo, acción y resultado; limpiar filtros y ver detalle del evento. | `Ajustes > Core > Bitácora`, solo lectura y exclusiva del Administrador del sistema. Conserva eventos por 1 año. No muestra ni permite modificar secretos o el contenido de los registros auditados. |
| Pantallas y módulos | Pantallas: crear, listar/consultar y editar identificador/nombre, descripción y tipo. Módulos: listar nombre, versión incluida, estado y ambiente actual; botones Activar/Desactivar para cada módulo disponible. | El backend valida catálogo, campos y unicidad. Solo el Administrador del sistema puede cambiar el estado. Se puede activar únicamente un módulo incluido en el release, permitido por la configuración del ambiente y con migraciones compatibles ya ejecutadas. Activar publica navegación, pantallas, settings, rutas API y herramientas MCP; desactivar los bloquea y conserva datos. Cada cambio se audita. |
| Permisos | Asignar permisos a un rol por módulo/recurso; interfaz y servidor usan la misma decisión. | Acciones acordadas: **ver, editar y eliminar**. **Editar incluye crear, actualizar y comandos de dominio no destructivos.** `consultar` del Legacy se mapea a `ver`; `insertar` y `modificar`, a `editar`; `eliminar`, a `eliminar`. Se comprueba autorización por registro en UI, API y MCP. |
| Empresa/SAR | Nombre, RTN, dirección, teléfono, correo, logo, rango autorizado, fecha límite, factura actual y texto SAR; guardar/cancelar y subir logo. Calcula días/facturas restantes y alertas. | Se conserva la validación de tipo/formato/tamaño en backend; logo JPG/JPEG/PNG/WEBP de hasta 2 MB. El cálculo y alertas se documentan en reglas de datos, sin depender de cálculos del navegador. |
| Correo saliente | En `Ajustes > Core > Correo saliente`, el Administrador configura el host SMTP, puerto y modo TLS globales. En `Ajustes > Mi cuenta > Correo saliente`, cada usuario configura solo su correo y contraseña; una contraseña vacía conserva la almacenada. El correo es la identidad SMTP AUTH y la dirección remitente. | El usuario no puede elegir host, puerto ni transporte. Los envíos se autentican en el servidor global con las credenciales del usuario/administrador emisor según el flujo. Contraseñas cifradas en reposo y nunca devueltas en claro. La salida SMTP aplica las restricciones anti-SSRF de `08-seguridad-y-permisos.md`. |
| Página no encontrada | Estado para rutas inexistentes; separación entre no encontrado y falta de permiso. | El permiso denegado se responde como tal según la política de API; la pantalla no revela si existe un recurso privado. |
| Mantenimiento de procesos | En Ajustes, dentro de Procesos: antigüedad y acción “Depurar procesos antiguos”. | La purga requiere permiso `eliminar`, confirmación explícita, límites, auditoría y resultado controlado. |

En la ficha de Persona, solo el nombre completo es obligatorio; por separado, cada cuenta de usuario debe vincularse a una Persona existente y una Persona no puede vincularse a más de una cuenta. Los campos, botones y funciones restantes se basan en el informe y el código; el HTML de Bastion se usa para apariencia.

## Validación obligatoria del backend

La validación del Core es obligatoria aunque la pantalla ya valide. Aplica a UI, API y MCP antes de ejecutar el caso de uso:

- Validar campos permitidos por operación (allowlist); rechazar campos inesperados y evitar asignación masiva.
- Validar presencia según el flujo, tipo, formato, normalización, tamaño y longitud máxima; imponer límites a listas, archivos, rangos y cantidades antes de consultar o reservar recursos.
- Validar enumeraciones y valores de catálogos en servidor. Los identificadores deben tener formato válido, existir y estar relacionados correctamente.
- Validar reglas cruzadas y de negocio; nombres únicos tras normalizar espacios y mayúsculas/minúsculas en altas y ediciones. Respaldar unicidad con restricciones/índices en MySQL para evitar carreras.
- Validar por separado la autorización de la acción, el objeto concreto, el tenant y los campos modificables. Conocer un ID válido no da permiso para accederlo.
- Devolver errores estables y legibles sin SQL, trazas, rutas internas ni valores secretos. Registrar el detalle técnico protegido con correlation ID.

Para Roles y Pantallas, el contrato declara cada campo y regla antes de implementar sus endpoints. No se acepta una mutación que dependa de validación exclusiva del frontend o que termine en error SQL sin controlar.

## Área Ajustes

La navegación muestra una entrada única `Ajustes`. Dentro de ella el core presenta:

1. **Ajustes del core:** opciones transversales, empresa/SAR, servidor de correo saliente, usuarios, roles, pantallas y permisos. Todos son exclusivos del Administrador del sistema.
2. **Mi cuenta:** ajustes propios del usuario autenticado, incluidos seguridad de acceso, dispositivos confiables, sesiones y sus credenciales personales para autenticarse en el servidor SMTP global.
3. **Ajustes de módulos:** una sección por cada módulo registrado que aporte configuración.

Ejemplo conceptual:

```text
Ajustes
├── Mi cuenta
│   ├── Perfil
│   ├── Seguridad y OTP
│   ├── Dispositivos confiables
│   ├── Sesiones
│   ├── Aplicaciones conectadas
│   └── Correo saliente (personal)
├── Core
│   ├── Empresa
│   ├── Correo saliente (servidor SMTP)
│   ├── Usuarios y acceso
│   ├── Personas y roles
│   ├── Módulos
│   ├── Bitácora
│   ├── Clientes IA y solicitudes
│   └── Notificaciones administrativas
├── Órdenes de compra
├── Cotizaciones
└── Facturas
```

La ubicación respeta el alcance de cada dato: Empresa/SAR y el servidor SMTP son configuración global del Core, exclusiva del Administrador del sistema. Correo saliente de `Mi cuenta` guarda solamente las credenciales SMTP personales del usuario, no el destino de red. Los envíos se autentican en el servidor global configurado por el Administrador con el correo/contraseña del emisor del flujo: el usuario que programa un envío, el Administrador que crea una invitación/recuperación o el Administrador designado para avisos administrativos. No hay un usuario SMTP transaccional compartido. Los demás ajustes del Core, incluidos los valores globales, la lista/cola de clientes IA y notificaciones administrativas, también son exclusivos del Administrador. No habrá una lista cerrada por marca: el Core apunta a admitir clientes compatibles con el perfil MCP/OAuth definido, siempre sujetos a aprobación administrativa. Para registrar clientes se prioriza CIMD; DCR solo se admite si una integración probada requiere compatibilidad heredada. Si cambia la ficha CIMD de un cliente aprobado, el Core bloquea nuevas conexiones hasta que un Administrador revise y apruebe el cambio. El Administrador aprueba qué clientes IA pueden iniciar la conexión y fija para cada cliente su máximo de alcances; cada usuario solo puede consentir alcances dentro de ese máximo y de sus permisos vigentes; un usuario puede solicitar desde el flujo OAuth que se habilite un cliente no autorizado o ampliar el máximo de alcances de un cliente aprobado. La solicitud queda pendiente para revisión en Ajustes y se notifica por correo a todos los Administradores activos; no inicia una conexión ni concede acceso. Si se aprueba una ampliación, el usuario debe consentir individualmente los nuevos alcances antes de usarlos. Si se aprueba, el solicitante ve el estado Aprobada en Ajustes y recibe un correo que indica que debe completar OAuth y autorizar sus alcances; la aprobación administrativa no crea grants ni tokens. Si se rechaza, el solicitante ve el estado en Mi cuenta > Aplicaciones conectadas, recibe un aviso por correo y puede enviar otra solicitud más adelante. La aprobación administrativa del cliente no concede acceso a datos de usuarios. Si el Administrador retira esa autorización, el Core revoca de inmediato todos los grants y tokens de usuario asociados al cliente. Si reduce el máximo de alcances, revoca de inmediato solo los grants y tokens que incluyan los alcances retirados; el usuario debe volver a completar OAuth y consentir dentro del máximo actualizado. Si falla un envío, el aviso permanece en Ajustes, muestra el error y permite reintentar. Los envíos programados usan las credenciales SMTP del usuario que los programó. Los módulos pueden organizar sus propios grupos dentro de su sección y declarar permisos para ellos según el contrato de módulos.

El Core usa `America/Tegucigalpa` como zona horaria compartida para mostrar fechas/horas y ejecutar envíos programados; no hay una zona horaria distinta por usuario. Las marcas de tiempo se guardan en UTC y se convierten al presentar o programar.

Para conservar el flujo actual, Mi Perfil permanece accesible desde el acceso de usuario existente y también se ofrece como `Ajustes > Mi cuenta > Perfil`.

`Mi cuenta` contiene ajustes del usuario conectado, no políticas globales de seguridad. Debe permitir consultar el estado de TOTP; cambiar el autenticador si conserva el actual, exigiendo contraseña y TOTP vigente, verificando el nuevo enrolamiento, regenerando los códigos de recuperación y revocando todas las sesiones y dispositivos confiables; TOTP no se puede desactivar. También puede revisar/revocar dispositivos propios y ver la sesión activa, revocarla y cerrar la sesión. Cada sesión muestra dispositivo/navegador, IP, fecha de inicio, última actividad e identifica la sesión actual. Se permite una sola sesión activa por usuario; al autenticarse desde otro navegador/equipo, contraseña y el TOTP que corresponda deben completarse antes de reemplazar la sesión previa. La sesión anterior recibe aviso al intentar usarla después de ser revocada. El usuario también puede consultar su historial de accesos de 30 días con fecha, IP, dispositivo y resultado, y consultar/revocar autorizaciones de aplicaciones conectadas y solicitudes de clientes IA en estados pendiente, aprobada o rechazada. Si la solicitud es rechazada, se muestra el estado y se envía aviso por correo; el usuario puede volver a solicitar más adelante. También permite configurar su correo y contraseña de autenticación SMTP para el servidor global; no muestra ni modifica host, puerto o TLS. La contraseña se redacta al consultar y solo se reemplaza si se envía una nueva; secretos OTP y credenciales de sesión no se muestran. El Core inicial no añade preferencias de apariencia (como tema o densidad); la sección se limita al perfil, seguridad, sesiones, aplicaciones conectadas e SMTP personal.

La política global confirmada es pedir contraseña y TOTP al iniciar sesión cuando el dispositivo sea nuevo o la IP no se haya reconocido. En un dispositivo/IP reconocidos, se permite el inicio normal con contraseña durante los primeros 15 días; al cumplirse ese plazo se vuelve a pedir TOTP, incluso si **Mantenerme loggeado** mantiene abierta la sesión. Tras validar TOTP se registra automáticamente el dispositivo por hasta 15 días, se informa al usuario y se le permite revocar la confianza desde Ajustes. Tras un acceso exitoso desde un dispositivo nuevo, enviar aviso al correo registrado usando las credenciales SMTP del propio usuario; si no las configuró o falla el envío, registrar el error y no bloquear la sesión autenticada. Los cambios de IP sin dispositivo nuevo aparecen en el historial de seguridad, sin generar correo. Las sesiones normales expiran tras 24 horas de inactividad. `Mantenerme loggeado` conserva la sesión sin límite de inactividad ni duración absoluta, con TOTP obligatorio cada 15 días. Solo hay una sesión activa por usuario; un nuevo acceso autenticado reemplaza la anterior después de completar el TOTP requerido y notifica a la sesión revocada. El usuario puede ver y revocar individualmente sus dispositivos confiables; las IP de inicio de sesión/eventos se conservan por 30 días.

Si el usuario conserva el autenticador, puede cambiarlo en Mi cuenta con contraseña y TOTP vigentes; verifica el nuevo enrolamiento, recibe códigos de recuperación nuevos y los anteriores quedan invalidados. El cambio revoca todas las sesiones y dispositivos confiables y exige autenticarse de nuevo. La recuperación de TOTP usa códigos de recuperación de un solo uso. Si se pierden el autenticador y todos esos códigos, un Administrador puede restablecer TOTP después de verificar la identidad del usuario presencialmente en la oficina; se registra quién y cuándo hizo la verificación. La nueva configuración de TOTP y entrega de códigos de recuperación es obligatoria antes de devolver acceso a módulos. La configuración inicial de TOTP también es obligatoria durante la activación de la cuenta. El desafío de segundo factor vence a los 5 minutos. Tras 3 códigos TOTP incorrectos, se invalida el desafío actual y se solicita iniciar sesión de nuevo; se aplican límites independientes por cuenta/IP sin usar el bloqueo prolongado de contraseña. El alta de usuarios es mediante invitación de un solo uso enviada al correo registrado, con vencimiento de 24 horas desde el envío exitoso. Si el primer envío falla, el usuario permanece como `Invitación pendiente` y se puede reenviar; el plazo comienza cuando un envío se completa correctamente. El restablecimiento de contraseña se solicita desde el inicio de sesión y envía un enlace de un solo uso al correo registrado, con vencimiento de una hora. Para invitaciones y recuperación se usa el servidor SMTP global configurado por el Administrador, autenticándose con las credenciales SMTP del administrador que creó la cuenta; no hay credencial transaccional central. Si el envío falla, el Core muestra un error al administrador y permite reintentar tras corregir sus credenciales o la configuración global; no usa otro remitente como respaldo. Las pausas automáticas por fallos de contraseña son progresivas y finitas: después de 3 fallos consecutivos inician en 30 minutos; cada nuevo grupo de 3 fallos tras vencer la pausa duplica el tiempo (60, 120, etc.) hasta un máximo de 24 horas. Un acceso correcto reinicia el contador. Todos los Administradores del sistema activos reciben una alerta al activarse el primer bloqueo de un incidente; se repite cuando ha transcurrido un periodo de 24 horas sin incidentes.

El Core trae un único rol integrado `Administrador del sistema`, protegido e inmutable desde la interfaz, con permisos completos de `ver`, `editar` y `eliminar` sobre Core y todos los módulos habilitados. Cada usuario tiene un rol; solo ese rol puede administrar usuarios, crear roles y asignar permisos. Los roles personalizados comienzan sin permisos. Siempre debe quedar al menos un Administrador del sistema activo: el backend impide eliminarlo, desactivarlo o quitarle el rol si es el último. La configuración de permisos sigue usando `ver`, `editar` (incluye crear) y `eliminar`. La primera cuenta se crea con un asistente web protegido por código secreto temporal; el asistente solo está disponible antes de que exista el primer Administrador del sistema y se desactiva tras crearlo. El primer administrador también debe completar el enrolamiento TOTP antes de acceder al Core.

## Registro de settings por módulo

Un módulo declara un identificador estable, etiqueta, grupos, campos tipados, valores predeterminados, reglas de validación, secretos, alcance (`global`, `empresa` o `usuario`), permisos `ver`/`editar` y versión de esquema. El core verifica el registro, comprueba permisos por rol, módulo y alcance, muestra los formularios, guarda valores en una tabla común por módulo/alcance/clave/versión como JSON validado por Rust, y registra cambios. Los secretos se cifran antes de persistirlos. Los roles personalizados solo acceden a los settings de módulo si el Administrador les asigna `ver` o `editar`; los roles nuevos empiezan sin acceso. El usuario solo puede leer o cambiar sus propios settings de alcance usuario; el Administrador del sistema conserva acceso total.

El core no interpreta la semántica de negocio de cada setting. El módulo valida/consume su configuración y puede exponer una pantalla especializada cuando un formulario genérico no alcanza; esa interfaz sigue presentada bajo Ajustes. Los ajustes del Core son exclusivos del Administrador del sistema; los settings propios del usuario y los aportados por módulos conservan sus alcances y permisos específicos.

## Ciclo de vida

- **Instalar/registrar:** el módulo aparece en el catálogo del Core cuando viene incluido en el release y la configuración del ambiente lo permite.
- **Habilitar/deshabilitar:** la configuración del ambiente limita qué módulos pueden usarse; el Administrador del sistema puede activar o desactivar los disponibles desde `Ajustes > Core > Módulos`. El servidor comprueba ambas condiciones. Un módulo desactivado deja de exponer pantallas, rutas API y herramientas MCP, y conserva sus datos.
- **Actualizar:** el módulo declara versión de settings y migraciones compatibles.
- **Retirar:** se oculta y bloquea el módulo; borrar configuración/datos requiere política de retención y acción administrativa explícita.

## Despliegue progresivo

El Core debe poder desplegarse solo en el ambiente de prueba, operar sin módulos de negocio y ser validado en los servidores antes de añadir módulos. Después, se incorpora y prueba un módulo por release. La configuración del ambiente establece el conjunto máximo disponible y el Administrador cambia el estado activo desde `Ajustes > Core > Módulos`; la interfaz no puede habilitar un módulo que no esté en el release, no esté permitido por el ambiente o no tenga migraciones compatibles aplicadas. Los módulos desactivados no aparecen ni exponen API/MCP, y conservan sus datos. En el monolito modular inicial, los releases actualizan el backend común.

## Requisitos de seguridad

- El control de acceso se aplica en servidor; ocultar una pantalla no sustituye autorización.
- El core ofrece políticas comunes para límites por operación, permisos por objeto y operación, sesión/CSRF, y autorización/auditoría compartida por UI, API y MCP.
- Las operaciones declaran y aplican límites de intentos, tamaño, paginación, rango de fechas, timeout y concurrencia antes de consumir recursos costosos. Los topes por cuenta/actor se aplican en el backend además de los límites perimetrales de Cloudflare Free.
- Los módulos declaran permisos, alcance de recursos y campos que cada operación puede modificar. El core verifica el objeto concreto en cada solicitud; las herramientas de IA respetan los mismos permisos.
- La sesión del navegador usa cookie host-only del API con `Secure`, `HttpOnly` y `SameSite` explícito; cambios de estado requieren token CSRF y origen exacto. Integraciones externas usan identidad/permisos propios. Los detalles se definen en `08-seguridad-y-permisos.md` y `05-api.md`.
- El backend limita el pool y los timeouts de MySQL remoto, usa TLS verificado y consultas parametrizadas con credenciales mínimas.
- Los secretos se almacenan cifrados y nunca se devuelven al cliente en claro.
- Cada cambio registra actor, módulo, fecha y resultado sin incluir valores secretos.
- Los cambios destructivos o de alto impacto requieren confirmación y permiso específico.

## Por decidir

- MySQL 5.7 está confirmado para Bluehost; quedan por verificar la versión de mantenimiento exacta, capacidades de TLS/remote MySQL del plan y compatibilidad de migraciones sobre esa versión. 5.7 admite el tipo nativo `JSON`, con validación de settings en Rust.
- Revisar con Bluehost la ruta de actualización de MySQL antes de producción, dado el estado de Sustaining Support de MySQL 5.7.
- Cada ficha de módulo especificará qué roles reciben permisos `ver` o `editar` para cada setting; el mecanismo común de autorización por rol, módulo y alcance ya está definido.
