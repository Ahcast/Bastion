# Core de Bastion

Especificación técnica vigente. [Arquitectura](README.md), [stack](../development/stack.md) y [plan](../development/plan.md). Core de plataforma implementado y verificado localmente; [estado real](../development/status.md) y [runbook](../development/core-runbook.md) distinguen integraciones pendientes.

## 2. Base que se conserva

1. La aplicación será un **monolito modular**: un backend Go y una SPA Angular, con límites internos claros.
2. Bluehost alojará Angular, MySQL y correo; AWS Lightsail ejecutará backend Go, API, MCP, crawler y workers.
3. MySQL será la fuente de verdad transaccional; S3 almacenará archivos, históricos y backups; GitHub conservará código y documentación.
4. El crawler permanecerá en AWS Lightsail. Su lógica de extracción no pertenece al Core.
5. Go controlará las reglas de negocio para todas las entradas: interfaz, API, automatizaciones y agentes.
6. La aplicación seguirá funcionando cuando la IA o el crawler estén temporalmente fuera de servicio.
7. La interfaz, sus mensajes, estados visibles, reportes y documentos para usuarios estarán en español. El código y los identificadores técnicos podrán permanecer en inglés.
8. Se conservarán los módulos existentes en el repositorio; este borrador no agrega módulos de negocio al menú aprobado.

### 2.1 Distribución de responsabilidades

| Componente | Responsabilidad | Límite |
| --- | --- | --- |
| Angular en Bluehost | Interfaz, navegación, formularios y presentación | No decide permisos ni reglas de negocio definitivas. |
| Go en AWS Lightsail | Core, módulos, API, validación, autorización y operaciones | Crawler Python en servicio separado; sin OCR ni inferencia. |
| MySQL en Bluehost | Estado operativo, relaciones, transacciones y metadatos | No funciona como almacén de archivos binarios masivos. |
| Lightsail | Extracción, reintentos técnicos y backup worker | No es el maestro de datos de negocio. |
| S3 | Archivos privados, evidencia, raw y recuperación | No reemplaza la base transaccional. |
| Agentes externos | Investigación, análisis y acciones autorizadas | No acceden directamente a MySQL ni obtienen privilegios de administrador por defecto. |

## 3. Qué es el Core

El término Core comprende dos partes coordinadas:

- **Core backend:** `backend/internal/core`, responsable de identidad y capacidades técnicas compartidas.
- **Core frontend:** `frontend/src/app/core`, responsable de sesión, cliente API, configuración y protección de navegación. Los componentes visuales reutilizables pertenecen a `shared`. Inicio y Configuración se ubican en `core/pages`; el shell común en `core/shell`.

Workflow y AI son módulos transversales separados en el repositorio. Aunque forman parte de la plataforma, no se fusionan dentro de Core.

### 3.1 Capacidades comunes y propietario

| Capacidad | Propietario propuesto | Qué reciben los módulos |
| --- | --- | --- |
| Usuarios, roles, equipos, sesiones y MFA | Core | Identidad y evaluación de acceso. |
| Configuración general de empresa | Core | Parámetros tipados y versionables. |
| Auditoría | Core | Registro uniforme de acciones. |
| Contexto de ejecución | Core | Actor, usuario delegado, origen y correlación. |
| Idempotencia y soporte de eventos durables | Core | Protección frente a duplicados y reintentos. |
| Transporte de archivos, correo y notificaciones | Core | Adaptadores comunes y seguimiento técnico. |
| Flujos, solicitudes de aprobación y decisiones | Workflow | Contrato común para transiciones y autorizaciones. |
| Carpetas, comentarios y organización documental | Collaboration | Experiencia de colaboración sobre archivos comunes. |
| Tareas, ejecuciones y proveedores de IA | AI | Integración mediante contratos autorizados. |
| Reglas, estados y datos comerciales | Cada módulo de negocio | Servicios del módulo, sin duplicarlos en Core. |

**Límite esencial:** Core aporta cómo identificar, autorizar, registrar y ejecutar una operación. El módulo propietario determina qué significa esa operación y cuáles son sus reglas.

Por ejemplo, Core no calcula el margen de una cotización, no decide si un proceso es interesante y no emite facturas. Quotes, Processes e Invoices conservan esas responsabilidades.

## 4. Organización del código y dependencias

Se mantienen estas ubicaciones existentes:

| Ruta | Uso |
| --- | --- |
| `backend/internal/core/` | Servicios transversales y sus contratos. |
| `backend/internal/modules/workflow/` | Flujos y aprobaciones. |
| `backend/internal/modules/ai/` | Orquestación de tareas de IA. |
| `backend/internal/modules/<module>/` | Lógica de cada módulo aprobado. |
| `backend/integrations/` | Documentación de integraciones; no asumir que ya es un namespace ejecutable. |
| `frontend/src/app/core/` | Autenticación, API, guards, interceptors y configuración. |
| `frontend/src/app/shared/` | UI, pipes, directivas y utilidades reutilizables. |
| `frontend/src/app/modules/` | Pantallas de los módulos. |
| `docs/contracts/` | Contratos públicos y de integración. |

La estructura documental vigente de Core separa `Identity`, `Access`, `Settings`, `Api`, `Mcp`, `Audit`, `Messaging`, `Files`, `Execution`, `Contracts` y `Modules`. Esta última área registra módulos; no contiene su lógica de negocio. Paquetes objetivo: `backend/internal/core`; módulos: `backend/internal/modules`.

Cada área incorpora paquetes y contratos Go cuando haya código que los justifique. La composición raíz registra módulos, rutas y herramientas MCP; Core no importa módulos concretos.

### 4.1 Reglas de dependencia

- Los módulos consumen contratos públicos de Core; Core no importa clases concretas de los módulos de negocio.
- Las operaciones entre módulos pasan por servicios de aplicación/Services públicos o eventos. Un módulo no modifica directamente las tablas de otro.
- Los controladores validan la entrada y delegan; la lógica reutilizable no vive dentro del controlador.
- Jobs, API REST y adaptadores MCP usan las mismas acciones de aplicación y las mismas políticas de autorización.
- Las consultas transversales de reportes pueden usar proyecciones o consultas de lectura documentadas; no permiten escrituras encubiertas.
- Los adaptadores externos encapsulan credenciales, protocolo y traducción de errores. No se dispersan llamadas al proveedor por los módulos.
- Una utilidad solo se incorpora a Core cuando es transversal o protege una regla común. No se traslada lógica comercial por comodidad.

## 5. Contrato para incorporar módulos

**Propuesta de implementación:** registrar módulos mediante configuración versionada y registro explícito de módulos Go. No se necesita un marketplace ni instalación de código desde la interfaz.

Cada módulo debe declarar:

1. Identificador estable y nombre visible en español.
2. Versión y dependencias obligatorias.
3. Tablas y datos que administra.
4. Rutas, acciones públicas y contratos de entrada/salida.
5. Permisos y políticas de autorización por acción y registro.
6. Eventos que publica y consume.
7. Jobs, horarios, límites y reglas de reintento.
8. Participación en auditoría, documentos, notificaciones y Workflow.
9. Migraciones, configuración, pruebas y procedimiento de desactivación.

Ejemplo conceptual, no configuración ejecutable:

```yaml
id: processes
label: Procesos
version: 0.1.0
depends_on: [core]
permissions:
  - process.read
  - process.assign
  - process.import
navigation:
  label: Procesos
  required_permission: process.read
```

La presencia de una carpeta no activa el módulo. Habilitarlo requiere dependencias satisfechas, migraciones aplicadas y configuración válida. Deshabilitarlo debe bloquear sus rutas y nuevos jobs, gestionar los trabajos pendientes y conservar sus datos. Ocultar el menú no equivale a desactivar el módulo.

## 6. Identidad, autenticación y sesiones

### 6.1 Usuarios de la SPA

Base de diseño: sesiones opacas del Core Go con cookies, CSRF y TOTP obligatorio. El alcance de usuarios, RBAC, MFA, sesiones y configuración SMTP dentro del Core fue confirmado expresamente en la sesión.

Lineamientos del borrador:

- Angular en `https://bastion.castilloit.net` y API en `https://api.bastion.castilloit.net` (propuesto) son orígenes distintos. Configurar CORS con origen exacto, credenciales y CSRF explícito; no usar wildcard.
- Cookie de sesión `__Host-bastion_session`, con `Secure`, `HttpOnly`, `SameSite=Lax`, `Path=/` y sin atributo `Domain`.
- La cookie de sesión no se comparte con WordPress. El aislamiento de cookies no sustituye el aislamiento de archivos y permisos dentro del hosting.
- El token CSRF se entrega mediante JSON y Angular lo envía explícitamente en X-CSRF-Token; la cookie de sesión permanece HttpOnly. Ver contrato API/MCP.
- No se almacenarán credenciales de sesión ni bearer tokens de la SPA en `localStorage`.
- Se exigirá completar el enrolamiento y confirmación TOTP antes de acceder a datos de negocio. Un login parcial solo habilitará rutas de enrolamiento, desafío y salida.
- Códigos de recuperación de un solo uso; secreto TOTP cifrado y nunca expuesto en logs.
- Recuperar contraseña no elimina MFA. El restablecimiento de MFA exige verificación administrativa de identidad, auditoría y revocación de sesiones.
- Renovar el identificador de sesión tras autenticación; invalidar sesión al salir y permitir revocación administrativa.
- Denegar registro público; el alta de empleados se realizará mediante administración o invitación controlada.

Valores iniciales propuestos: 30 minutos de inactividad, 12 horas de duración absoluta y 5 minutos de vigencia para reautenticación de operaciones sensibles. Deben ser configurables dentro de límites seguros y aplicarse en backend. La política de sesiones simultáneas sigue pendiente. El polling automático no debe mantener indefinidamente una sesión sin actividad humana.

Para acciones críticas se exigirá reautenticación reciente. La aprobación de negocio y la reautenticación son controles diferentes: una no sustituye a la otra. Passkeys para administradores y aprobadores quedan como fase posterior.

### 6.2 Identidades de servicio y agentes

- Cada integración tiene identidad propia, credencial revocable, alcance mínimo y fecha de expiración.
- `crawler-lightsail` conserva únicamente permisos relacionados con sus entregas.
- `backup-worker` utiliza permisos específicos de backup, separados de la aplicación y del crawler.
- Un asistente delegado opera con la intersección de permisos del usuario, alcance de la integración y política sobre el recurso.
- `requested_by` se deriva de la identidad autenticada/delegación validada; no se acepta como prueba de identidad un campo enviado libremente por el agente.
- La tecnología propuesta para delegación MCP es implementación OAuth por seleccionar con OAuth y PKCE. No se presupone un proveedor OIDC ni que sesiones del Core resuelva ese flujo. Deben probarse emisor, audiencia, revocación y compatibilidad del cliente.

## 7. Autorización uniforme

**Denegar por defecto.** Toda acción debe comprobar identidad activa, permiso, acceso al registro y restricciones de estado. Cuando corresponda, también debe comprobar aprobación y reautenticación.

Los permisos distinguen consultar, crear, modificar, enviar, aprobar, exportar y administrar. Los roles agrupan permisos; las políticas de autorización incorporan propietario, equipo, asignación u otras restricciones reales. No se crean todavía roles comerciales no aprobados.

Los guards de Angular mejoran la navegación, pero el backend decide. Listados, búsquedas, adjuntos, exportaciones y endpoints de detalle deben aplicar las mismas restricciones.


Cambiar roles o desactivar una cuenta debe invalidar permisos cacheados y bloquear operaciones posteriores, incluidos jobs delegados todavía pendientes.

## 8. Configuración y datos compartidos

### 8.1 Configuración

Separar configuración de infraestructura, secretos y parámetros de negocio:

| Tipo | Ubicación propuesta | Gestión |
| --- | --- | --- |
| Infraestructura no secreta | Configuración de despliegue y código | Versionada, por ambiente. |
| Credenciales y claves | Entorno protegido, fuera del directorio público | Sin Git ni respuestas de API. |
| Empresa y parámetros editables | MySQL | Validación, permisos y auditoría. |
| Preferencias personales | MySQL por usuario | Sin capacidad de alterar controles de seguridad. |

La pantalla Configuración reúne las opciones existentes en el documento maestro, pero cada módulo conserva la propiedad de sus reglas: impuestos, series, plantillas y workflows no deben convertirse en lógica genérica indiscriminada.

Para la primera versión se propone una sola empresa operadora, Castillo IT. Multiempresa/SaaS no se incorpora como requisito implícito.

### 8.2 Convenciones de datos

- Migrations versionadas y cambios compatibles con el proceso de despliegue.
- InnoDB, transacciones, claves foráneas e índices; validar motor y versión realmente disponibles.
- Fechas de eventos en UTC y presentación en `America/Tegucigalpa`. Fechas sin hora se conservan como fecha, sin conversiones artificiales de zona.
- Importes en decimal, con moneda explícita; nunca cálculos financieros mediante punto flotante binario. La precisión y el redondeo serán definidos por los módulos pertinentes.
- Identificadores estables; no generar consecutivos con `MAX(id) + 1`.
- Restricciones únicas de base de datos para prevenir duplicados incluso bajo concurrencia.
- Una operación con múltiples escrituras relacionadas debe usar transacción.
- Evitar sobrescritura silenciosa mediante versión del registro o mecanismo equivalente; un conflicto debe ser visible para el usuario.
- Desactivación o anulación cuando se requiera trazabilidad. No aplicar borrado lógico indiscriminadamente a todas las tablas.

Entidades técnicas propuestas: sesiones, identidades de servicio, configuración, `audit_events`, `idempotency_records`, `outbox_events`, `jobs`, `failed_jobs`, notificaciones y metadatos/versiones de archivos. El esquema físico final se definirá al implementar; no todas requieren una tabla nueva si el framework ya proporciona una adecuada.

## 9. API y contexto de ejecución

Se conservan `/api/v1/` y `/api/internal/v1/`, REST JSON sobre HTTPS y documentación OpenAPI 3.1. El prefijo `internal` no hace privado un endpoint: requiere autenticación, permisos y límites como cualquier otra entrada.

Cada ejecución debe propagar un contexto uniforme:

```json
{
  "actor_type": "user|service|agent",
  "actor_id": "identidad autenticada",
  "requested_by": null,
  "source": "web|api|job|mcp|crawler",
  "request_id": "identificador de esta petición",
  "correlation_id": "identificador del flujo completo"
}
```

Los listados tendrán paginación y límites; filtros y ordenamientos se validarán con listas permitidas. Los errores expondrán código estable, mensaje en español, errores por campo cuando proceda y `request_id`, sin stack traces ni secretos.

Usar respuestas coherentes: autenticación fallida, acceso denegado, validación, conflicto y limitación de frecuencia deben distinguirse. Un `202 Accepted` indica recepción durable para procesamiento posterior; no significa operación completada.

Los cambios incompatibles requieren nueva versión o migración explícita. La API no expondrá modelos completos por conveniencia; cada respuesta tendrá campos definidos.

## 10. Idempotencia y consistencia

**Propuesta de implementación:** servicio común de idempotencia para importaciones, solicitudes reintentables, envíos y ejecución de acciones sensibles.

- La clave se delimita por identidad y operación, junto con un hash del contenido relevante.
- La misma clave con el mismo contenido devuelve el resultado ya registrado o el estado en curso.
- La misma clave con contenido diferente produce conflicto.
- La reserva es atómica, respaldada por restricción única. Dos solicitudes simultáneas no deben ejecutar dos veces la operación.
- Se documentarán retención, recuperación de operaciones interrumpidas y condiciones para reintentar fallos.
- Antes de devolver una respuesta guardada se valida nuevamente el acceso vigente del solicitante.

La idempotencia no sustituye las claves únicas del negocio. Aunque caduque un registro técnico, reenviar una observación o documento ya importado no debe crear otra entidad.

No se promete «exactamente una vez» entre servicios externos. La estrategia será entrega al menos una vez con consumidores idempotentes y conciliación cuando el resultado sea incierto.

### 10.1 Eventos y outbox

Los cambios que requieran efectos externos deben guardar estado de negocio, auditoría relevante y evento pendiente en la misma transacción de MySQL. Un job posterior publica o procesa el evento de outbox; los consumidores deduplican por `event_id`.

El evento tendrá identificador, tipo, versión de esquema, entidad, fecha y correlación. Su contenido será mínimo. No incluirá documentos completos ni secretos.

Si la transacción falla, no se envía el evento. Si el proceso cae después de confirmar la transacción, el evento pendiente permanece recuperable. Una simple llamada «después del commit» no garantiza por sí sola esa recuperación.

## 11. Trabajos en segundo plano en Lightsail

Cola durable MySQL y worker Go implementados; ejecución acotada mediante timer systemd en Lightsail. Disponibilidad y consumo reales del hosting por validar.

**Propuesta inicial:** scheduler y workers acotados mediante cron, con bloqueo contra solapamientos, límite de duración, memoria, cantidad de jobs e intentos. Los workers Go deben detenerse al agotar su presupuesto incluso si siguen entrando tareas.


Todo job debe:

- Procesar una unidad pequeña o un lote acotado.
- Guardar progreso cuando pueda interrumpirse.
- Ser idempotente y llevar correlación.
- Tener timeouts de red y reintentos limitados con espera creciente para fallos transitorios.
- Registrar fallo definitivo y permitir reintento controlado.
- Reevaluar permisos y vigencia de aprobaciones antes de producir efectos sensibles.

No mantener requests HTTP abiertos durante importaciones grandes, generación masiva de documentos, investigación o llamadas extensas de IA. Tampoco dormir un worker durante largos periodos de espera: programar el siguiente intento.

No asumir Redis, Supervisor, Docker, WebSockets propios ni Node persistente en Bluehost. Angular se compila fuera del hosting y se despliegan sus archivos estáticos.

OCR, procesamiento documental intensivo y cargas que excedan el presupuesto se delegarán a un ejecutor externo mediante un contrato específico. El Lightsail no se considera automáticamente capaz de recibir todas esas cargas ni debe exponer MySQL para ejecutar workers remotos.

## 12. Auditoría y aprobaciones

### 12.1 Auditoría

Core ofrecerá un único mecanismo para registrar actor, usuario delegado, acción, entidad, fecha, origen, resultado y correlación. Guardará cambios anteriores y posteriores solo en campos pertinentes y redactados.

La auditoría comercial exitosa se confirma junto con la transacción. Intentos denegados y fallos se registran aparte para no desaparecer con un rollback. Los logs técnicos no sustituyen el historial de negocio.

No registrar contraseñas, tokens, cookies, secretos TOTP ni cuerpos completos sensibles. Definir permisos de consulta, retención y exportación. El historial no será editable desde la aplicación; una tabla append-only no equivale a evidencia criptográficamente inalterable frente a un administrador de base de datos.

### 12.2 Workflow y aprobación

Workflow administra definiciones, instancias, transiciones y aprobaciones. Core aporta identidad, acceso, auditoría y mecanismos de ejecución.

Una aprobación debe quedar ligada a acción, entidad, versión o hash del contenido, solicitante, aprobador, decisión y vigencia. Si cambia el importe, destinatario u otro dato relevante, la aprobación anterior deja de autorizar esa versión.

La ejecución verifica nuevamente la aprobación y la consume de forma atómica cuando corresponda, evitando dobles ejecuciones. Los estados de aprobación y ejecución se registran por separado.

Se mantienen como acciones sujetas a aprobación las que comprometen dinero, autorizan pagos, presentan ofertas o emiten órdenes a proveedores. La matriz concreta de aprobadores y umbrales sigue pendiente. Para la primera versión se propone impedir la autoaprobación en acciones críticas; cualquier excepción tendrá que quedar definida explícitamente.

## 13. Archivos, documentos y correo

### 13.1 Servicio común de archivos

S3 privado conserva los bytes; MySQL conserva identificador, nombre original, tipo, tamaño, checksum, versión, ubicación, propietario y vínculos con entidades. La organización de carpetas y colaboración permanece en Collaboration.

- Las descargas requieren permiso sobre el documento y la entidad vinculada; una S3 key no constituye autorización.
- Entregar acceso temporal después de autorizar, con vencimiento corto y sin persistir URLs firmadas como ubicación definitiva.
- Validar tamaño, extensión y tipo real; no permitir ejecución de archivos subidos.
- Los archivos no confiables pasan por cuarentena y el control de seguridad definido antes de quedar disponibles.
- Un módulo vincula documentos mediante identificadores; no construye rutas arbitrarias ni manipula credenciales de S3.
- Cambiar contenido crea versión cuando se necesite historial; no sobrescribir evidencia silenciosamente.

Como MySQL y S3 no comparten transacción, se usa el flujo `pending` → `quarantined` → `available` o `rejected`, con verificación del objeto. La limpieza de huérfanos requiere política operativa pendiente. Una carga incompleta no se muestra como documento disponible. DocumentPolicies y BindDocument conectan políticas de módulos implementados sin importar negocio en Core; el acceso requiere tanto el permiso del archivo como el de la entidad.

El bucket actual del crawler está confirmado en el diseño. Los buckets adicionales de documentos y backups son propuestas del maestro, no recursos cuya creación se haya verificado.

### 13.2 Correo y notificaciones

El correo permanece en Bluehost. Core administra también configuración SMTP —servidor, puerto, cifrado, credenciales, remitente y pruebas controladas— además de transporte y registro técnico. El módulo propietario determina finalidad, destinatarios autorizados, plantilla y necesidad de aprobación.

Los envíos se encolan, tienen identificador estable, límites y prevención de duplicados. Diferenciar pendiente, aceptado por el servidor de correo, fallido y resultado incierto. La aceptación SMTP no demuestra entrega final al destinatario.

Ante timeout después de un posible envío, no reenviar a ciegas. Conciliar el estado o escalar la incertidumbre. Los agentes usarán herramientas acotadas como `send_supplier_rfq`, no una capacidad irrestricta de enviar correos.

## 14. Integración del crawler

Processes conserva el importador y las reglas de consolidación de procesos. Core aporta autenticación del servicio, validación transversal, idempotencia, cola, auditoría y observabilidad.

Se conserva el contrato conceptual existente:

```http
POST /api/internal/v1/crawler/runs
POST /api/internal/v1/crawler/runs/{run_id}/processes
POST /api/internal/v1/crawler/runs/{run_id}/complete
```

Reglas obligatorias al implementarlo:

1. Distinguir ejecución del crawler, recepción durable y aplicación a datos de negocio.
2. Cada lote tiene identificador, versión de esquema y checksum verificable.
3. Un ACK de recepción solo se emite después de guardar durablemente el lote. El resultado de importación se consulta o devuelve por separado.
4. Reenviar el mismo lote no duplica procesos, versiones, eventos ni análisis de IA.
5. Un resultado parcial no elimina registros ausentes ni reemplaza campos conocidos por vacíos provenientes de fallos.
6. Datos externos y datos internos —responsable, notas, prioridad o estado comercial— se mantienen separados.
7. Una entrega atrasada no revierte automáticamente una observación más reciente; se define orden y procedencia de versiones.
8. Fallar Bluehost no obliga a repetir el scraping: se reentrega desde artefactos existentes.
9. La importación permite replay sin consultar HonduCompras.

El cliente API y su ACK están documentados en IMPORT_CONTRACT del crawler. Processes implementa los tres endpoints según [contrato](../contracts/processes.md); se conserva el código Python. El ACK de esta versión confirma aplicación durable, no solo recepción. Pruebas/CI y límites externos en [status](../development/status.md).

## 15. Integración de IA y MCP

AI administra tareas durables, ejecutores y resultados; MCP es un adaptador Core hacia los mismos servicios de aplicación de Go. El ejecutor inicial prioritario es ChatGPT Work con skills mediante el plan del usuario; Go no consume Plus como API de inferencia. Ver [integración IA](ai-integration.md) y [contrato](../contracts/ai-pipeline.md). El Core no contendrá lógica exclusiva para una marca de modelo.

Registrar tarea, actor, delegación, herramientas usadas, resultado, errores y costos cuando estén disponibles. Establecer presupuestos, límites y cancelación. Un proveedor fuera de servicio deja trabajo pendiente o fallido, sin bloquear la operación manual.

Documentos, páginas y correos son datos no confiables: su contenido no puede otorgar permisos, cambiar políticas ni autorizar acciones. Las herramientas validan argumentos y permisos independientemente de lo que produzca el modelo.

MCP pertenece expresamente al Core; AI administra tareas y resultados sin duplicar servicios de aplicación/políticas de autorización. MCP se habilita por fases, comenzando por consulta y escritura acotada de resultados. El RAG inicial apoya pricing con fuentes S3 e índices/metadatos MySQL; búsqueda vectorial opcional posterior. No se requiere RAG para operar Core. Compatibilidad Work/Plus, ejecución diaria desatendida, Grokbot y corpus requieren validación antes de activar.

## 16. Core de la interfaz

La estructura visual común incluirá navegación, encabezado, sesión, notificaciones y tratamiento uniforme de carga, vacío, error y acceso denegado. Inicio y Configuración se ubican en `core/pages/dashboard` y `core/pages/settings`; la composición raíz registra los módulos y sus rutas, sin importar módulos concretos desde servicios de Core.

Menú aprobado: **Inicio, Clientes, Proveedores, Órdenes de Compra, Cotizaciones, Facturas, Gastos, Reportes, Procesos, Colaboración y Configuración.** Workflow y AI no agregan automáticamente entradas superiores al menú.

- Carga diferida por feature y componentes visuales reutilizables.
- Navegación filtrada por permisos efectivos y módulos habilitados.
- Mensajes de validación, errores y estados de negocio en español.
- Manejo central de sesión vencida, CSRF, falta de permiso, conflictos y límites de frecuencia.
- No reintentar automáticamente escrituras si no tienen contrato de idempotencia.
- Polling inicial de 15–30 segundos conforme al maestro, solo donde se necesite; pausar en pestañas ocultas y aplicar espera creciente ante errores.
- Diseño usable por teclado, con estados de foco y formularios accesibles.

La biblioteca de componentes, identidad visual y diseño detallado de pantallas quedan fuera de este borrador. No se consideran aprobados por el hecho de definir el Core.

## 17. Despliegue, seguridad operativa y recuperación

### 17.1 Validación previa de Bluehost

Antes de desplegar validar runtime Go/Linux en Lightsail, acceso remoto cifrado MySQL, límites de conexiones, SMTP, HTTPS, S3, memoria y despliegue estático Angular en Bluehost.

Objetivo: Go en Lightsail, Angular en Bluehost y Percona/MySQL 5.7 en Bluehost. Fijar versiones y dependencias tras validar compatibilidad; no se requiere runtime backend para Bastion.

### 17.2 Despliegue controlado

- Separar desarrollo, pruebas y producción, con credenciales distintas y datos de prueba adecuados.
- En Bluehost exponer únicamente activos compilados de Angular; en Lightsail publicar HTTPS del proxy hacia Go. Mantener secretos, backups y logs fuera del document root.
- Desactivar debug en producción; HTTPS y protección WAF con reglas compatibles con API, autenticación y cargas autorizadas. Proveedor/configuración final por validar.
- Construir y comprobar dependencias fuera del hosting cuando sea posible; desplegar un artefacto reproducible asociado a un commit.
- Usar exclusivamente main y commits directos conforme a la política Git; sin ramas ni PR. CI revisa cada publicación. La restricción preventiva de creación requiere GitHub Pro en este repositorio privado.
- Aplicar migraciones compatibles, comprobar sesión, API y procesamiento de jobs, y disponer de rollback del código.
- Un rollback de código no implica revertir automáticamente la base de datos. Cambios destructivos requieren respaldo y plan específico.

### 17.3 Observabilidad y backups

Registrar `request_id`, `correlation_id`, `job_id` y `run_id` donde corresponda. Monitorizar latencia, errores, edad del trabajo pendiente, último cron, fallos de importación y último backup verificado.

La respuesta pública de salud será mínima; los diagnósticos internos requerirán permiso. Mostrar en Configuración los estados operativos sin revelar secretos.

Se conserva la arquitectura de backup Bluehost → backup worker Lightsail → S3. No habilitar acceso MySQL irrestricto para facilitarla; usar acceso restringido y cifrado o extracción por SSH. El mecanismo de extracción y credenciales se validará en el hosting.

Un backup no está probado hasta restaurarlo. El plan debe incluir MySQL, archivos y recuperación segura de claves necesarias para descifrar información. Definir RPO, RTO y responsables. La retención sugerida por el maestro —30 diarios, 12 semanales y 12 mensuales— permanece pendiente de validación operativa y de costos.


## Controles y aceptación

### 24.2 Parámetros propuestos de identidad

| Control | Valor o política inicial |
| --- | --- |
| Contraseña | Mínimo 15 caracteres; permitir gestores y pegado; bloquear contraseñas comunes/comprometidas. |
| MFA | TOTP obligatorio para todos; códigos de recuperación de un solo uso. |
| Sesión inactiva | 30 minutos. |
| Duración absoluta | 12 horas. |
| Reautenticación | Antigüedad máxima de 5 minutos para operaciones sensibles. |
| Alta | Administración o invitación; sin registro público. |
| Desactivación | Revocar sesiones/tokens y reevaluar trabajos delegados pendientes. |
| Cambios administrativos | Permiso específico, reautenticación y auditoría. |

Son parámetros propuestos en esta sesión, no configuración desplegada. Separar administración de usuarios de aprobación financiera. Administrar roles no debe conceder automáticamente capacidad de aprobar pagos. Una aprobación de negocio no se sustituye con un desafío MFA.

### 24.3 API y acceso a datos

- Denegar por defecto; comprobar permisos en todas las peticiones, incluidos listados, búsquedas, archivos y exportaciones.
- Evitar acceso a registros ajenos por modificación de identificadores. No confiar en IDs difíciles de adivinar como control de acceso.
- Permitir explícitamente campos editables; no aceptar roles, propietario o estados protegidos mediante asignación masiva.
- Consultas parametrizadas y listas permitidas de filtros/ordenamientos.
- Escape de contenido, CSP y CSRF para autenticación mediante cookies.
- CORS limitado a orígenes necesarios, sin comodín con credenciales. CORS no sustituye autenticación ni protege de clientes no navegador.
- Límites de frecuencia, concurrencia, paginación y tamaño por operación; valores tras medir cargas reales.
- Errores controlados, sin stack traces, credenciales ni estructuras internas.
- Transacciones, idempotencia y restricciones únicas para operaciones reintentables.

### 24.4 MCP y agentes

- Efectuar autorización por identidad, conexión, herramienta, registro y estado en cada invocación.
- Consentimiento explícito y redirects OAuth exactos; validar emisor, destinatario y expiración del token según su formato y flujo.
- Rechazar tokens destinados a otro recurso. No reenviar tokens recibidos a proveedores externos.
- Verificar `Origin` cuando corresponda al transporte, sin confundirlo con autenticación.
- Herramientas limitadas y argumentos validados; no exponer SQL genérico, shell, acceso arbitrario a archivos ni envío irrestricto de correo.
- Contenido de páginas, PDFs, correos o respuestas de modelos se trata como dato no confiable. No puede otorgar permisos ni alterar políticas.
- Registrar agente y usuario delegado; este último procede de la delegación validada, no de argumentos libres.
- Permitir desactivar MCP completo, una conexión o una herramienta mediante control administrativo auditado.
- Restringir volúmenes de resultados, exportaciones y trabajos simultáneos. La reanudación de un job vuelve a comprobar acceso.

### 24.5 Archivos, SMTP y salida de red

S3 privado, autorización por entidad y enlaces temporales. Validar tipos y tamaños, poner en cuarentena y analizar archivos no confiables antes de habilitarlos. La herramienta de análisis y su ejecutor quedan pendientes; no se asume disponible en Bluehost.

Las credenciales SMTP permanecen cifradas o en entorno protegido, nunca visibles en respuestas o logs. Las pruebas de envío son acciones explícitas con destinatario autorizado. Correo operativo con límites, control de adjuntos y prevención de duplicados.

Evitar SSRF: validar URLs de descargas e integraciones, restringir protocolos y destinos, bloquear loopback, redes privadas y endpoints de metadatos, y reevaluar redirecciones y resolución DNS. Las URLs encontradas en documentos no se descargan sin estos controles.

### 24.6 Secretos, hosting y recuperación

Credenciales separadas para aplicación, migraciones, crawler y backup. Usar permisos mínimos; no conceder administración de base de datos al usuario de ejecución habitual. Guardar claves fuera del repositorio y directorio público. La recuperación incluye las claves necesarias para descifrar información y su custodia separada.

Un subdominio dedicado y cookies separadas no aíslan sitios que comparten usuario de sistema. Evaluar exposición frente a WordPress u otros sitios de la misma cuenta. Confirmar parches de Percona antes de producción.

### 24.7 Operación e incidentes

Auditar accesos, fallos MFA, cambios de permisos/SMTP, conexiones MCP, exportaciones y acciones sensibles sin guardar secretos. Alertar sobre intentos repetidos y patrones anómalos con umbrales configurables.

Respuesta mínima: revocar sesiones/tokens, suspender conexiones o jobs afectados, preservar evidencia, corregir el origen del incidente, rotar credenciales comprometidas y restaurar solo cuando corresponda. Documentar responsable y procedimiento. Revisar dependencias y cambios de permisos antes de cada despliegue.

## 25. Verificación y condiciones de salida

Además de los criterios de la sección 19, comprobar:

1. Sesiones parciales sin MFA no acceden a datos y un usuario revocado pierde acceso por UI, REST y MCP.
2. Cambiar identificadores no permite consultar registros ajenos ni descargar sus documentos.
3. Herramientas no autorizadas siguen bloqueadas aunque se invoquen directamente.
4. Tokens de audiencia incorrecta o delegaciones revocadas son rechazados.
5. Datos externos maliciosos no permiten eludir políticas de autorización, aprobaciones o límites.
6. Cambiar contenido relevante invalida una aprobación anterior y evita ejecución duplicada.
7. SMTP/S3/IA/crawler pueden fallar sin perder estados ni producir reintentos ciegos.
8. API/MCP operan a través del WAF sin cachear datos privados ni depender de desafíos interactivos.
9. Dependencias y migraciones funcionan en Go/Percona realmente disponibles.
10. Backups, claves y rollback permiten una recuperación ensayada.

La estructura y el Core ya tienen implementación local y pruebas; las integraciones/despliegue reales están sujetos a validación. La validación de este documento no acredita controles instalados ni una auditoría de seguridad del hosting.

