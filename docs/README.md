# Documentación del nuevo CastilloIT

Este paquete convierte el análisis de la aplicación actual y las decisiones acordadas en una especificación para desarrollar la nueva aplicación en Rust.

## Documentos

| Archivo | Propósito |
|---|---|
| `00-vision-y-alcance.md` | Objetivo, límites y resultados esperados. |
| `01-aplicacion-actual-y-paridad.md` | Informe fuente: inventario de módulos, pantallas, campos, botones, flujos, API y hallazgos del sistema actual. |
| `02-core-y-ajustes.md` | Responsabilidades del core y ajustes centrales con aportaciones de cada módulo. |
| `03-arquitectura-rust.md` | Principios y estructura técnica objetivo. |
| `04-contrato-de-modulos.md` | Contrato para conectar módulos al core. |
| `05-api.md` | Reglas del contrato HTTP del core y módulos. |
| `06-mcp.md` | Registro y ejecución segura de herramientas MCP. |
| `07-datos-y-reglas.md` | Modelo de dominio, persistencia y reglas conocidas. |
| `08-seguridad-y-permisos.md` | Identidad, permisos, datos, archivos y controles para IA. |
| `09-despliegue.md` | Bluehost, AWS, MySQL, ambientes y conectividad. |
| `10-criterios-de-aceptacion.md` | Cómo revisar paridad y completar la implementación. |
| `12-direccion-visual.md` | Referencia visual elegida y sistema de diseño a derivar de ella. |
| `13-panel-inicio.md` | Inicio del Core, cinco áreas, navegación plegable y adaptación móvil. |
| `14-permisos-por-rol.md` | Matriz visual de permisos por rol, compatibilidad, conservación y autorización. |
| `adr/ADR-0002-frontend-react.md` | React + TypeScript + Vite y build estático para Bluehost. |
| `adr/ADR-0001-stack-inicial.md` | Decisión técnica del primer corte del Core. |
| `modulos/README.md` | Catálogo de módulos derivado del informe actual. |
| `modulos/_plantilla-modulo.md` | Plantilla común de especificación por módulo. |

Cada módulo del catálogo tendrá una ficha individual dentro de `modulos/`.

## Criterio de certeza

- **Confirmado en informe:** aparece en rutas, scripts o controladores analizados.
- **Por verificar:** el código compilado o estático no aclara la presentación o regla completa.
- **Pendiente de decisión:** requisito nuevo que debe definirse antes de implementarlo.

El informe se obtuvo de una copia de frontend Angular compilado y backend Laravel. No se validaron pantallas en una sesión activa ni se dispuso del esquema completo de producción. No interpretar los pendientes como ausencia de funcionalidad.

## Registro de decisiones

- 2026-10-23: Facturas desplegado en AWS/Bluehost desde `f6c5a1d` tras CI exitoso, respaldo privado y migración propia 1. Se verifican salud HTTPS, 35 archivos públicos y preservación de 43 tablas existentes. Disponible sin activar; ítems sin tracking ni comentarios. Ver `modulos/facturas-implementacion.md`.

- 2026-10-23: corrección solicitada de Procesos → Cotización: una línea por ítem analizado de Scout con su cantidad, en lugar de filas originales que agrupan varios productos. No usar descripción ni items_fuente como línea genérica. Se conserva el aviso no bloqueante. El usuario indica continuar la publicación; ver `modulos/procesos-cotizacion.md`.

- 2026-10-23: Facturas adopta los lineamientos de frontend de Cotizaciones: listado compacto, buscador único de cliente manual/catálogo, generales, ítems en tabla adaptable y notas/totales separados. El servidor valida la selección y conserva la instantánea del cliente. Se mantiene el menú de nueve acciones y el alcance de borradores sin emisión fiscal. El usuario autoriza subir este corte al repositorio; ver `modulos/facturas-implementacion.md`.

- 2026-10-23: el usuario confirma para Facturas un botón desplegable por fila con Ver, Editar, Marcar como enviado, Ingresar pago, Clonar, PDF, Nota de entrega, Emitir Recibo y Eliminar, en ese orden. El corte actual conecta Ver/Editar y muestra las demás opciones deshabilitadas hasta implementar sus casos de uso; no se amplían permisos ni se dan por definidas reglas pendientes. Ver `modulos/facturas-implementacion.md`.

- 2026-10-22: a solicitud del usuario comienza Facturas con un corte local de borradores sin emisión ni numeración fiscal. Listado/editor, cálculo comercial puro reutilizado de Cotizaciones, inversión/gastos, permisos y auditoría del Core, idempotencia y control de versión. Las reglas aún pendientes de pagos, numeración, envío y corrección no se presumen resueltas. Se incorpora desactivado y sin ajustes editables; no se declara completo ni desplegado. Ver `modulos/facturas-implementacion.md` y `modulos/facturas.openapi.yaml`.

- 2026-10-23: el usuario solicita Convertir a cotización desde Procesos: entidad como cliente manual, expediente como referencia e ítems con cantidades originales de Scout. Abre un borrador en el editor y usa el guardado normal. Recordatorio informativo inline, sin confirmación ni bloqueo, con enlace al original. Cantidades no verificables quedan vacías y no se inventan. Ver `modulos/procesos-cotizacion.md`.

- 2026-10-23: el usuario solicita adjuntar varios archivos en Cotizaciones y abrir PDF en el navegador sin descarga automática. Se implementa almacenamiento privado mediante el Core, control de revisiones y adjuntos incluidos en correo según Legacy; límites técnicos documentados en modulos/cotizaciones-acciones.md. No cambia la separación de despliegue.

- 2026-10-23: el usuario aporta el logo de Castillo IT para cotizaciones, facturas y notas de entrega. Firma/sello se aplican a cotizaciones en la parte inferior izquierda, a la altura de subtotales/totales. Los recursos originales son privados y compartidos por el Core; no se publican en GitHub ni en Bluehost. Ver `15-identidad-documental.md`; las plantillas PDF de los módulos pendientes conservan su estado de implementación.

- 2026-10-22: el usuario confirma incorporación incremental de Scout aunque el rastreo o análisis del ciclo queden parciales. Se valida una publicación atómica finalizada con SHA-256 y total completo; solo se incorporan filas con extracción completa, análisis confirmado y hash vigente. Pendientes, errores y extracciones incompletas se difieren sin borrar ni sobrescribir históricos, marcas o notas. La interfaz informa filas diferidas; un fallo de autenticación de IA sigue necesitando corrección y no se interpreta como análisis exitoso.

- 2026-10-22: Ajustes > Procesos muestra como información de solo lectura los reportes main/scout/combiner/clasificador de Scout, última ejecución, estado por etapa, contadores y análisis IA. Usa consulta/ver del módulo activo, consulta cada 60 segundos y no ejecuta scripts ni importaciones. Archivos y campos acotados, sin JSON crudo ni excepciones/configuración. Sustituye la ficha estática dentro del Core; ver `modulos/procesos-implementacion.md`.

- 2026-10-22: el usuario confirma menú por cotización con Ver, Editar, Marcar como enviado, conversiones, Aprobar, Clonar, PDF, correo y Eliminar. Ambas conversiones quedan visibles y deshabilitadas hasta desarrollar sus módulos. PDF usa el diseño Legacy como referencia para Typst y correo conserva destinatarios/copias/mensaje y programación. Empresa, compilación PDF, almacenamiento privado y SMTP siguen como servicios del Core. Implementación y condiciones de habilitación: `modulos/cotizaciones-acciones.md`.

- 2026-10-22: por autorización del usuario se publica el primer corte técnico de Cotizaciones en GitHub y luego AWS/Bluehost. El código `78743be` pasó CI con MySQL 5.7, se respaldaron base/frontend y se aplicaron dos migraciones. Cotizaciones queda disponible pero sin activar; Procesos y sus 835 registros se conservaron. Ver la evidencia y pendientes en `modulos/cotizaciones-implementacion.md`.

- 2026-10-22: corrección de Procesos: mostrar solo recepción estrictamente futura por fecha y hora exactas; conservar vencidos en el histórico. Estadísticas globales de la importación independientes de filtros y vencimiento. Nuevos por semana usa la primera incorporación en Bastion mediante inserted/created_at de recibos confirmados, no primera detección de Scout. Ver `modulos/procesos-implementacion.md`.

- 2026-10-22: se coteja la copia Legacy indicada por el usuario para Cotizaciones. Se corrigen defaults a 30 días/ISV15, vigencia 0..3650, longitudes y contacto del usuario creador. Se implementan contador COT, lista de estados y cálculo decimal con la secuencia de redondeo del backend. La vista previa invoca Rust para coincidir con el guardado; no se redistribuyen residuos ni se incorporan controles de exoneración ausentes en la UI activa. Ver `modulos/cotizaciones-paridad.md`; validación contable y despliegue siguen pendientes.

- 2026-10-22: comienza el corte local de borradores. Registro `cotizaciones`, ledger de migraciones propio, DTO decimales y escritura transaccional con auditoría, idempotencia y concurrencia. Se amplía la allowlist a IDs separados por comas. La paridad comercial y settings siguen pendientes; ver `modulos/cotizaciones-implementacion.md`. No se despliega ni migra la base remota.

- 2026-10-22: Procesos adopta detalle expandido dentro de la fila según la referencia HTML del usuario, estadísticas de nuevos por semana y entregas de tres días en hora de Honduras, y filtros compactos con entidad no municipal, búsqueda de contenido fuente/expediente, orden de recepción/puntuación y checks. Las estadísticas usan todos los resultados filtrados; se deriva primera detección en UTC para agregación. No se añaden permisos ni settings. Ver `modulos/procesos-implementacion.md` y su OpenAPI.

- 2026-10-22: el usuario confirma que el botón Actualizar de Procesos incorpora el maestro local de Scout y después recarga la consulta. Se elimina Importar CSV y la recepción de archivos por HTTP; Exportar CSV conserva su comportamiento. La nueva operación POST de actualización no recibe rutas ni CSV, requiere sesión, CSRF y `procesos/importacion/editar`, verifica el manifiesto y audita al usuario que la solicita. La recepción periódica opcional conserva su identidad técnica independiente.

- 2026-10-22: se confirma entrega local de Scout y Bastion en el mismo servidor, desde `/var/lib/bastion-scout/maestro.csv` y su `main.json`. Un trabajador del backend verifica finalización, SHA-256 y cantidad de filas, y reutiliza el importador transaccional con identidad interna del Core limitada a `procesos/importacion/editar`. No se expone un endpoint nuevo ni se usan cuentas humanas o credenciales de red. Se admite el formato actual `resumen`/ítems numerados conservando compatibilidad con la entrega anterior. Ver `modulos/procesos-entrega-local.md`.

- 2026-10-21: el usuario confirma que Personas, Usuarios, Roles y permisos y Mi cuenta pertenecen a Ajustes. El menú principal conserva Inicio, los módulos habilitados y un único acceso Ajustes. Mi cuenta está disponible para cada usuario autenticado; Personas conserva sus permisos y Usuarios/Roles/Core siguen reservados al Administrador. La ubicación no modifica la autorización del API.

- 2026-10-21: el usuario confirma conservar todos los procesos y dejar la depuración pendiente. Ajustes > Procesos presenta la fuente Scout, la conservación total y el alcance compartido de marcas/notas como política informativa del primer corte; no se agregan controles editables ni borrado automático. Una política configurable o una acción de purga requiere definición posterior.

- 2026-10-21: el usuario confirma marcas De interés y notas compartidas por todos los usuarios del módulo Procesos. Se persisten separadas del CSV de Scout, se conservan al importar y se editan con control de versión para evitar sobrescrituras entre usuarios.

- 2026-10-21: primer corte de Procesos separado en `crates/modules/procesos`, con migraciones y ledger `_procesos_migrations` propios. `migrate` aplica primero Core y después Procesos; `serve` solo sincroniza disponibilidad y no crea tablas. La allowlist `BASTION_ALLOWED_MODULES` queda vacía por defecto; permite `procesos` después de sus migraciones, y el administrador controla su activación. Los endpoints vuelven a comprobar disponibilidad y permisos y se bloquean al desactivar sin borrar datos. La entrada manual usa el cliente CSRF común; MCP y recepción automática esperan sus contratos de identidad.

- 2026-10-21: comienza Procesos como primer módulo de negocio, con publicación y validación primero en GitHub. El usuario confirma como entrada el CSV maestro de `bastion-scout` después de su análisis IA; el contrato se verifica en su repositorio y se documenta en `modulos/procesos-implementacion.md`. El CSV del crawler anterior no define el nuevo importador. Settings, alcance de marcas/notas y purga siguen pendientes de definición explícita.

- 2026-10-21: frontend y API instalados en el ambiente de pruebas con HTTPS y MySQL TLS verificado. Las notas de operación, IPs, cuentas y rutas reales se conservan localmente fuera del repositorio público.

- 2026-10-21: el usuario confirma React + TypeScript estricto y Vite para el frontend del Core y módulos. Se compila como archivos estáticos para Bluehost, con Node únicamente en desarrollo/CI. Se mantiene Rust en AWS y MySQL en Bluehost. Se migra con paridad funcional y visual, carga por pantallas y cliente API/CSRF común; primero documentación, después GitHub validado y finalmente Bluehost. La decisión y sus límites se registran en `adr/ADR-0002-frontend-react.md`.

- 2026-10-21: se corrige el enrolamiento TOTP del frontend para mostrar QR SVG generado localmente, además de la clave manual. Se incluye `qrcode-generator` 2.0.4 bajo MIT en `web/vendor/`; la URI y la clave no se envían a servicios QR externos. El mismo componente visual se reutiliza en el cambio de autenticador, cuyo endpoint sigue pendiente de implementación. Se verifica la decodificación exacta de tres URI de prueba con un lector independiente y la alternativa manual ante fallo del generador. La corrección de archivos estáticos se publica en Bluehost con respaldo previo.


- 2026-10-21: se confirma por SSH de solo lectura que Lightsail usa Ubuntu 24.04.4, arquitectura x86_64 y glibc 2.39. El pipeline genera el ejecutable de release en Ubuntu 24.04 para `x86_64-unknown-linux-gnu`, después de formato y pruebas, con dependencias fijadas por `Cargo.lock`. Publica un artifact con procedencia y SHA-256; esta etapa prepara el paquete sin instalar ni iniciar servicios en Lightsail.

- 2026-10-21: el usuario confirma `bastion.castilloit.net` y una instancia AWS Lightsail para las pruebas; MySQL permanece en Bluehost. Solicita por ahora únicamente publicar el código existente en `https://github.com/Ahcast/Bastion/`, directamente en `main`, sin crear ramas de desarrollo ni desplegar servicios. El workspace Rust se publica en la raíz y el frontend en `web/`. La compilación y validación de conectividad/despliegue siguen pendientes.

- La aplicación se organiza en un core y módulos conectables.
- El core conserva sus funciones y aloja una sección central `Ajustes`.
- `Ajustes` incluye `Mi cuenta`, con perfil, credenciales SMTP personales, seguridad, dispositivos confiables y sesiones del usuario autenticado; el acceso actual al perfil se conserva.
- El Core inicial no agrega preferencias de apariencia por usuario; `Mi cuenta` cubre perfil, seguridad, sesiones y credenciales para enviar correo.
- En `Ajustes`, Empresa/SAR y el endpoint/puerto/TLS del servidor SMTP son configuración global del Core y exclusivos del Administrador. Cada usuario configura solo su correo y contraseña SMTP para autenticarse en ese servidor. Los envíos programados usan las credenciales del usuario que los programa.
- Cada módulo registra allí su propia sección de settings; declara el alcance de sus valores (global, empresa o usuario) y los permisos `ver`/`editar`.
- Todos los ajustes del Core son exclusivos del Administrador del sistema; los módulos definen permisos para sus propias secciones de settings.
- Los ajustes de módulo usan permisos `ver`/`editar` por rol, módulo y alcance; los roles nuevos comienzan sin acceso y el Administrador del sistema conserva acceso total.
- Los settings de Core y módulos se almacenan en una tabla común por módulo/alcance/clave/versión, con JSON validado por Rust y secretos cifrados.
- El core aloja la infraestructura común de API y MCP; cada módulo aporta sus operaciones/capacidades.
- La IA interactiva del primer release usa identidad delegada del usuario conectado, respeta sus permisos y queda auditada bajo ese usuario; las cuentas de servicio quedan para automatizaciones futuras.
- La IA usa OAuth Authorization Code + PKCE, alcances mínimos y revocación de aplicaciones conectadas desde `Ajustes > Mi cuenta`.
- El Administrador del sistema aprueba qué clientes IA pueden iniciar conexiones OAuth y define el máximo de alcances por cliente; después cada usuario consiente alcances dentro de ese máximo y sus propios permisos. Un usuario puede solicitar ampliarlo; si se aprueba, debe volver a consentir los nuevos alcances. El usuario puede solicitar habilitar un cliente no autorizado, pero no obtiene conexión hasta la aprobación del Administrador y el consentimiento individual. La solicitud queda en la cola de Ajustes y genera un correo para todos los Administradores activos, enviado al servidor SMTP global con las credenciales del Administrador activo designado en Ajustes; no hay credencial transaccional compartida. Si un aviso falla, se conserva en Ajustes, muestra el error y permite reintentar. Si una solicitud se aprueba, el usuario ve Aprobada en Ajustes y recibe correo para completar OAuth y autorizar sus alcances; no obtiene acceso hasta dar su consentimiento. Si se rechaza, ve el estado, recibe correo y puede solicitarlo de nuevo más adelante. La aprobación administrativa no concede acceso a datos y cualquier ampliación de alcances requiere nuevo consentimiento. Si el Administrador retira la aprobación de un cliente, se revocan inmediatamente todas sus conexiones, grants y tokens de usuario. Si quita un alcance de su máximo, revoca solo los grants y tokens afectados y exige que sus usuarios vuelvan a consentir para reconectar.
- El primer release busca admitir cualquier cliente que cumpla el perfil MCP/OAuth definido, con autorización previa del Administrador y validación de interoperabilidad; no se fija una lista de marcas. CIMD es el registro principal; DCR solo se usa si una integración validada necesita compatibilidad heredada. Los cambios de ficha CIMD bloquean nuevas conexiones hasta revisión y aprobación administrativa.
- MCP remoto se publica desde AWS mediante Streamable HTTP sobre HTTPS.
- MCP opera sin estado de protocolo; cada llamada se autentica y autoriza, mientras que los grants OAuth se administran y revocan por separado.
- Sus access tokens duran 15 minutos; los refresh tokens son rotatorios, duran hasta 15 días y exigen nueva autorización con TOTP al vencer; desactivación, cambio de contraseña y revocación manual invalidan la conexión.
- La API se especifica primero en OpenAPI; `/api/v1` es el prefijo propuesto. Adopta errores RFC 9457, paginación acotada, autorización por objeto/campo, idempotencia para operaciones con efectos repetibles y `ETag`/`If-Match` para evitar sobrescrituras. La interfaz usa sesión por cookie segura en el host del API; los clientes externos/IA usan identidades separadas con alcance mínimo. Rutas y cuotas concretas se completan por operación en `05-api.md`.
- Los PDFs se generarán con Typst mediante un servicio compartido del core; los módulos aportan los datos y documentos de su dominio. La integración y gestión de plantillas se definirán antes de implementar.
- Despliegue acordado: frontend Bluehost, backend/API/MCP AWS, base MySQL Bluehost.
- Decisión técnica del primer corte Core: workspace Cargo con `crates/core`, `crates/api`, `crates/mcp` y `apps/server`; `tokio` + `axum` + `tower-http` para el servidor HTTP y `sqlx` MySQL con TLS `VerifyIdentity`. CORS acepta un único origen exacto configurado, el cuerpo HTTP general se limita a 1 MiB y el pool MySQL se mantiene acotado. MCP y los flujos de autenticación no se consideran implementados en este corte; se añadirán según sus contratos antes de exponer operaciones de usuario.
- Bluehost usa MySQL 5.7; el Core inicial y sus migraciones deben ser compatibles con esta versión. Se puede usar el tipo nativo `JSON` para settings con validación de contenido en Rust. Confirmar la versión de mantenimiento exacta, TLS y acceso remoto del plan. MySQL 5.7 está en Sustaining Support de Oracle desde octubre de 2023; revisar con Bluehost la ruta de actualización antes de producción.
- PDFs generados con Typst y adjuntos se guardan en almacenamiento privado de objetos en AWS; acceso público deshabilitado y autorización comprobada por el Core. No caducan mientras exista el registro asociado; al borrar el registro con permiso, confirmación y auditoría, se eliminan sus archivos. Servicio y respaldo quedan por definir técnicamente.
- Protección perimetral propuesta: Cloudflare Free como DNS/proxy/WAF para `bastion.castilloit.net` y `api.bastion.castilloit.net`; MySQL sigue directo entre AWS y Bluehost y requiere su propio control de IP/TLS. Las limitaciones del plan se documentan en `09-despliegue.md`.
- Salida SMTP: el Administrador configura un servidor global; el usuario no controla el host/puerto/TLS ni puede enviar overrides. El backend conecta únicamente al host global configurado, valida/pinea su IP pública y TLS, y bloquea redes internas y metadatos; AWS restringe el egreso SMTP a los puertos aprobados y a CIDR estables cuando existan. Esto elimina la selección de destino desde una cuenta de usuario y reduce el riesgo residual por error o compromiso administrativo.
- Lineamientos de seguridad de prioridad alta acordados para el core: límites por operación y cuenta, límites perimetrales con Cloudflare Free más cuotas del backend, permisos por operación/objeto para UI/API/MCP, conexión AWS–MySQL restringida y verificada, y controles de sesión/CSRF. El detalle normativo está en `08-seguridad-y-permisos.md` y se refleja en arquitectura, contratos y criterios de aceptación.
- Estrategia de lanzamiento: desplegar y probar el Core solo en servidores; después incorporar y validar un módulo por release. Se confirma un backend Rust común y habilitación de módulos por ambiente; el mismo artefacto se ejecuta con Core solo o con los módulos habilitados. Artefactos independientes o carga dinámica quedan fuera del primer release.
- Habilitación de módulos: despliegue limita qué módulos están disponibles en cada ambiente; el Administrador del sistema activa o desactiva los permitidos desde `Ajustes > Core > Módulos`. Se valida inclusión, allowlist y migraciones; los cambios quedan auditados y desactivar conserva datos.
- Seguridad de acceso acordada: TOTP en app autenticadora al iniciar sesión desde equipo nuevo/no reconocido o IP nueva/no reconocida, y cada 15 días aunque `Mantenerme loggeado` conserve abierta la sesión. Tras validar TOTP se renueva la confianza por hasta otros 15 días. La IP se registra como señal de riesgo/auditoría.
- El usuario puede cambiar su TOTP desde Mi cuenta con contraseña y TOTP vigentes; se verifica el nuevo enrolamiento, se regeneran los códigos de recuperación y se revocan sesiones/dispositivos. No puede desactivar TOTP.
- Sesiones y cuenta: sesión normal expira tras 24 horas de inactividad; `Mantenerme loggeado` no tiene expiración por inactividad ni absoluta, pero exige TOTP cada 15 días. El usuario puede ver y revocar dispositivos de confianza; las IP se conservan 30 días. La recuperación de TOTP usa códigos de recuperación de un solo uso.
- Usuarios y roles del Core: un usuario tiene un rol; solo el Administrador del sistema gestiona usuarios, roles y permisos. Core inicia con ese rol y los roles personalizados empiezan sin permisos. Usuarios nuevos reciben una invitación de un solo uso por correo que vence en 24 horas.
- El backend impide eliminar, desactivar o quitar el rol al último Administrador del sistema activo.
- Historial de acceso/IP: 30 días. Bitácora de acciones administrativas y de negocio: 1 año; su vencimiento no elimina los registros de negocio.
- El Administrador del sistema consulta eventos de auditoría en `Ajustes > Core > Bitácora`, de solo lectura y con filtros por fecha, actor, módulo, acción y resultado.
- Zona horaria del sistema: `America/Tegucigalpa` para presentación y tareas programadas; las marcas de tiempo se guardan en UTC y no hay selección por usuario.
- El rol Administrador del sistema tiene permisos completos sobre Core y todos los módulos habilitados.
- El rol Administrador del sistema no puede eliminarse, renombrarse ni perder permisos desde la interfaz.
- El Administrador debe asignar un rol antes de enviar la invitación a un usuario.
- Cada usuario debe vincularse a una Persona existente y cada Persona puede vincularse a una sola cuenta; el backend y MySQL garantizan la relación uno a uno.
- El Administrador puede reasignar la Persona de una cuenta en cualquier estado solo a una Persona libre; cada cambio queda auditado.
- Desactivar una cuenta bloquea nuevos accesos y revoca inmediatamente sesiones y dispositivos confiables; la reactivación requiere autenticación normal con TOTP según corresponda.
- La observación al desactivar o reactivar es opcional; ambas acciones se auditan.
- Las cuentas se conservan y se desactivan; no se eliminan definitivamente desde la aplicación.
- La Persona permanece vinculada a la cuenta desactivada y solo se libera mediante una reasignación auditada a una Persona libre.
- Desactivar una cuenta con invitación pendiente invalida el enlace; al reactivarla se emite una invitación nueva.
- Contraseñas: mínimo 8 caracteres, sin reglas obligatorias de combinación ni vencimiento periódico; cambio por solicitud o alerta. La recuperación usa enlace de un solo uso al correo registrado. Invitaciones y recuperación usan el servidor SMTP global con las credenciales personales del administrador que creó la cuenta.
- El correo nuevo de acceso debe confirmarse antes de sustituir al actual; hasta entonces se conserva el correo vigente para acceso y recuperación.
- El cambio de correo propio desde Mi cuenta requiere revalidar contraseña y TOTP antes de iniciar la confirmación.
- Cambiar el correo de otra cuenta requiere que el Administrador revalide contraseña y TOTP y que el usuario destinatario confirme la dirección nueva.
- Al completar el cambio, se avisa al correo anterior mediante un mensaje de seguridad sin enlaces de acción.
- Protección de acceso por fallos: tras 3 fallos consecutivos se notifica al administrador y se aplican pausas progresivas que duplican su duración (30, 60, 120 minutos...) hasta 24 horas; un inicio correcto reinicia el contador. El bloqueo automático no es indefinido.
- Las alertas de bloqueo se envían a todos los Administradores del sistema activos: una por incidente al activarse el primer bloqueo, con una nueva alerta si transcurren 24 horas sin incidentes.
- Si se introducen 3 códigos TOTP incorrectos, se invalida ese desafío y se vuelve al login; los límites TOTP por cuenta/IP son independientes del bloqueo progresivo de contraseña.
- El desafío TOTP posterior a la contraseña vence a los 5 minutos.
- Los intentos TOTP se limitan a 10 fallos por cuenta o 30 por IP en 15 minutos; alcanzar cualquiera activa pausa de 15 minutos en ese alcance, aparte del bloqueo de contraseña.
- La activación de cada cuenta exige configurar TOTP y guardar los códigos de recuperación antes de acceder a módulos.
- Si se pierden el autenticador y todos los códigos de recuperación, un Administrador puede restablecer TOTP tras verificar identidad; el usuario debe enrolarse de nuevo antes de acceder.
- Esa verificación administrativa se realiza presencialmente en la oficina y se registra quién la hizo y cuándo.
- La primera cuenta se crea con un asistente web de configuración protegido por código secreto temporal; el asistente se desactiva después de crear el primer Administrador del sistema.
- El enlace de recuperación de contraseña es de un solo uso y vence en una hora.
- Cualquier cambio o recuperación de contraseña revoca todas las sesiones y dispositivos confiables; se requiere iniciar sesión de nuevo con TOTP.
- En `Ajustes > Mi cuenta > Sesiones`, el usuario ve los datos de su sesión activa y puede revocarla; solo se permite una sesión activa por usuario.
- Cada sesión muestra dispositivo/navegador, IP, fecha de inicio, última actividad y si es la sesión actual.
- Se permite una sesión activa por usuario; un nuevo acceso autenticado con contraseña y el TOTP requerido reemplaza la anterior y la notifica.
- El usuario puede consultar su historial de accesos de los últimos 30 días, con fecha, IP, dispositivo y resultado.
- Tras un acceso exitoso desde un dispositivo nuevo se intenta enviar aviso al correo registrado con las credenciales SMTP del usuario; si no están configuradas o falla el envío, se registra y no bloquea el acceso.
- Si falla SMTP al enviar invitaciones o recuperaciones, el Core muestra el error y permite reintentar cuando se corrijan las credenciales del administrador creador o la configuración SMTP global; no utiliza otra cuenta como respaldo.
- Si falla una invitación inicial, se conserva el usuario en estado `Invitación pendiente`; sus 24 horas de validez empiezan cuando un reenvío logra entregarse.
- Diseño recomendado de confianza del dispositivo: token aleatorio revocable en cookie protegida, almacenando su hash en el servidor; una huella de navegador convencional no se usa como único mecanismo de confianza.
- Tras completar TOTP, el dispositivo se registra automáticamente como confiable por hasta 15 días; el usuario recibe aviso y puede revocar la confianza desde Ajustes.
- Personas: únicamente el nombre completo es obligatorio; DNI, país y género son opcionales y se validan si se proporcionan. El DNI es único cuando se informa.
- Matriz de permisos confirmada: `ver`, `editar` (incluye crear, actualizar y comandos de dominio no destructivos) y `eliminar`. El backend aplica el mismo permiso por operación y objeto en UI, API y MCP.
- 2026-10-22: se corrige la lectura del identificador opcional de roles personalizados al cargar el catálogo tras iniciar sesión y al administrar usuarios. Un valor SQL nulo identifica un rol personalizado válido; no concede administración. Se comprueban contraseña, TOTP, dispositivo confiable, catálogo sin permisos, asignación explícita y desactivación en MySQL 5.7 de pruebas.
- Facturas: estados `Borrador`, `Enviado` y `Pagado`; un pago parcial conserva `Enviado` y saldo cero determina `Pagado`. La condición exacta de envío y la corrección posterior siguen pendientes.
- Credenciales: contraseñas con Argon2id; tokens aleatorios opacos guardados solo como hashes; semillas TOTP y contraseñas SMTP personales cifradas con cifrado autenticado y claves protegidas/rotables mediante AWS KMS. Códigos de recuperación se consumen una sola vez; refresh tokens OAuth rotan y detectan replay. Los parámetros completos están en `08-seguridad-y-permisos.md`.
- Roles y Pantallas: el backend valida todas las entradas y relaciones, con nombres normalizados/únicos, límites explícitos y restricciones en MySQL para evitar duplicados concurrentes; no se confía en validación del frontend.
- El catálogo de módulos de primera etapa se deriva de `01-aplicacion-actual-y-paridad.md`.

- 2026-10-22: el usuario confirma sustituir el editor JSON de permisos por una matriz visual con columnas Ver, Crear y editar (una sola casilla del permiso existente editar) y Eliminar independiente; se conserva el contrato, los permisos adicionales y la administración exclusiva del Administrador del sistema. Ver 14-permisos-por-rol.md.

- 2026-10-22: primera versión de Clientes cotejada con formulario/controlador Legacy: siete campos, búsqueda, catálogo paginado, alta con merge y edición con versión. Permisos y auditoría del Core. El usuario autoriza publicar sin ajustes editables y con su sección en Ajustes indicando el pendiente. Ver modulos/clientes-implementacion.md. No se migra el catálogo Legacy ni se habilita borrado/MCP.

- 2026-10-22: Clientes publicado en GitHub y desplegado en AWS/Bluehost desde `c86e8142763bea59adcad7dfb442daa4e96a3526`, tras CI exitoso (46 pruebas Rust y 42 frontend), artifacts verificados y respaldos privados. Migración propia aplicada; datos y módulos existentes conservados. Clientes queda disponible, desactivado hasta su activación administrativa. Salud HTTPS y hashes públicos comprobados; ver evidencia y límites en `modulos/clientes-implementacion.md`.

- 2026-10-23: Inicio consume resúmenes reales de Procesos y Cotizaciones por sus permisos existentes. Mes/Año/Todo, Actualizar, recepciones vigentes, estados e importes registrados; calendario de Honduras, errores parciales y presentación móvil. Core comparte contrato/calendario y cada módulo conserva sus consultas. No requiere migración. Ver [16-dashboard-modulos.md](16-dashboard-modulos.md).

- 2026-10-23: continúa Órdenes de compra con listado/filtros, editor y cálculo cotejado del Legacy. El usuario confirma agregar el catálogo de destinos en Ajustes > Órdenes de compra; alta/renombrado como ampliación autorizada, propiedad del dominio de Órdenes y permisos destinos/ver/editar. Registro ordenes_compra desactivado, migraciones propias, idempotencia, versiones y auditoría; cliente manual como instantánea. Archivos, proveedores, clonación, eliminación, conversiones y MCP siguen pendientes. Publicado en GitHub y desplegado en AWS/Bluehost desde 59a8057 con CI exitoso, respaldo y migración propia verificados; disponible sin activar. No se migran datos Legacy. Ver modulos/ordenes-de-compra-implementacion.md y su OpenAPI.
- 2026-10-23: el usuario confirma que los ítems de Facturas no llevan tracking ni comentarios. Se retiran del editor y del contrato; los borradores anteriores siguen siendo legibles y estos campos se omiten al responder y guardar. Ver modulos/facturas-implementacion.md.
